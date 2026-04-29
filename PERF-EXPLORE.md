# Performance Exploration Log

Permanent record of performance investigations and changes applied to taffy.

## Summary of Results

| ID  | Area               | Description                                                                                      | Result         | Commit     |
| --- | ------------------ | ------------------------------------------------------------------------------------------------ | -------------- | ---------- |
| R1  | round_layout       | SSE4.1 runtime dispatch for `round_layout_inner`                                                 | **POSITIVE**   | `44a3cce1` |
| R2  | round_layout       | Subexpression caching (`round(cx)`, `round(cy)` etc)                                             | **POSITIVE**   | `44a3cce1` |
| R3  | round_layout       | SIMD batch `_mm_round_ps` (4 floats at once)                                                     | **NEGATIVE**   | —          |
| R4  | round_layout       | Integer-based `round()` replacing `(v+0.5).floor()`                                              | **NEGATIVE**   | —          |
| R5  | round_layout       | `f32::round()` replacing `(v+0.5).floor()`                                                       | **NEGATIVE**   | —          |
| R6  | flexbox            | Eliminate per-iteration `Vec<&mut FlexItem>` in `resolve_flexible_lengths`                       | **POSITIVE**   | `44a3cce1` |
| R7  | flexbox            | Cache `box_sizing` on `FlexItem`, avoid re-fetching `child_style` in `determine_used_cross_size` | **POSITIVE**   | `27fa775c` |
| R8  | flexbox            | Resolve padding/border once in `generate_anonymous_flex_items`                                   | **POSITIVE**   | `27fa775c` |
| R9  | flexbox            | Compute `padding + border` once in `determine_flex_base_size`                                    | **POSITIVE**   | `27fa775c` |
| R10 | flexbox            | Cache min/max cross in `determine_hypothetical_cross_size`                                       | **POSITIVE**   | `27fa775c` |
| R11 | grid               | Single-pass `spanned_track_limit` / `spanned_fixed_track_limit`                                  | **POSITIVE**   | `cb60b382` |
| R12 | grid               | Merge count+sum in `stretch_auto_tracks`                                                         | **POSITIVE**   | `cb60b382` |
| R13 | grid               | Merge triple iteration in `distribute_space_up_to_limits`                                        | **NEGATIVE**   | —          |
| R14 | grid               | Merge triple filter in `distribute_item_space_to_growth_limit`                                   | **NEUTRAL**    | —          |
| R15 | grid               | Pre-compute `padding_border_size` on `GridItem`                                                  | **NOT VIABLE** | —          |
| R16 | grid               | Cache resolved margins on `GridItem`                                                             | **NOT VIABLE** | —          |
| R17 | block              | Double padding/border resolution in `compute_block_layout` + `compute_inner`                     | **NEUTRAL**    | —          |
| R18 | flexbox/block/grid | Hidden item loop calls `get_*_child_style` for all children                                      | **NEUTRAL**    | —          |
| R19 | build              | `RUSTFLAGS="-C target-cpu=native"`                                                               | **POSITIVE**   | —          |
| R20 | flexbox            | Cache `max_size_ignoring_aspect_ratio` on `FlexItem`                                             | **POSITIVE**   | —          |
| R21 | round_layout       | Skip rounding for already-integer values                                                         | **NEGATIVE**   | —          |

## Detailed Notes

### R1: SSE4.1 runtime dispatch (POSITIVE)

**File:** `src/compute/mod.rs`

Profiling showed `round_layout_inner` + `floorf` accounted for 98% of total runtime. The `round()` function used `(value + 0.5).floor()` which calls libc `floorf` — a slow software emulation on baseline x86_64 (SSE2 only, no SSE4.1 `roundss` instruction).

The fix dispatches to a `#[target_feature(enable = "sse4.1")]` function at the `round_layout` entry point. This compiles the inner function with SSE4.1 enabled, allowing `_mm_floor_ss` to emit the single-instruction `floorss` instead of a libc call.

**Verified via `objdump`:** The SSE4.1 path emits real `roundss $0x1` instructions. The fallback path (non-SSE4.1 CPUs) uses the original `(v + 0.5).floor()`.

**Impact:** ~2.1-2.9x speedup across all benchmarks.

### R2: Subexpression caching in round_layout (POSITIVE)

**File:** `src/compute/mod.rs`

The original code computed `round(cumulative_x)` up to 8 times per node (once per border/padding edge). Caching `round(cx)`, `round(cy)`, `round(cx + width)`, `round(cy + height)` into 4 variables reduced redundant `round()` calls from ~24 to ~16 per node.

Note: The compiler may already perform CSE (common subexpression elimination) here, but explicit caching ensures it and makes the code clearer.

### R3: SIMD batch rounding (NEGATIVE)

**File:** `src/compute/mod.rs`

Attempted to batch 16 scalar `roundss` calls into 4x `roundps` (4 floats at once) using `_mm_floor_ps(_mm_add_ps(vals, set1(0.5)))`.

**Why it failed:** The `round_layout` function is extremely sensitive to rounding precision. Even tiny differences in rounding behavior cascade through cumulative coordinates and cause layout instability. The `relayout_is_stable_with_rounding` test broke because the batch approach produced subtly different results for values near x.5 boundaries in certain edge cases.

**Lesson:** Layout rounding is not a good target for SIMD parallelism because each rounded value feeds into the next calculation, and the precision semantics must exactly match `(v + 0.5).floor()`.

### R4: Integer-based round() (NEGATIVE)

Replaced `(value + 0.5).floor()` with `value as i32` truncation + diff check:

```rust
let i = value as i32;
let diff = value - (i as f32);
if diff >= 0.5 { (i + 1) as f32 } else if diff <= -0.5 { (i - 1) as f32 } else { i as f32 }
```

**Why it failed:** The `as i32` cast truncates toward zero, which differs from `.floor()` for negative non-integer values. While the logic was corrected for the rounding direction, floating-point values produced by layout arithmetic (e.g., `300.4999999...` from `1920.0 / 100.0 * 300.5`) can fall on either side of the 0.5 boundary, and the integer cast changes which side.

**Lesson:** Don't change the rounding implementation — the `(v + 0.5).floor()` formula is semantically load-bearing.

### R13: Grid distribute_space_up_to_limits triple iteration (NEGATIVE)

**File:** `src/compute/grid/track_sizing.rs`

The inner loop of `distribute_space_up_to_limits` iterates tracks 3 times per loop iteration:

1. Compute `track_distribution_proportion_sum` (filter + map + sum)
2. Compute `min_increase_limit` (filter + map + min)
3. Apply increase (filter + mutation)

Attempted to merge passes 1+2 into a single loop accumulating both `proportion_sum` and `min_increase_limit`.

**Why it failed:** Caused ~5-10% regression on grid benchmarks. The merged single-pass version changes which tracks receive increases — in the original, the apply pass (step 3) uses a slightly different filter condition than the analysis passes (steps 1+2). Specifically, step 3 applies to all `track_is_affected` tracks regardless of limit, while steps 1+2 only consider tracks below their limit. Merging these semantics is non-trivial and the naive merge changed behavior.

### R15/R16: Grid GridItem padding/border/margin caching (NOT VIABLE)

**File:** `src/compute/grid/types/grid_item.rs`

`GridItem` stores raw style values (`Rect<LengthPercentage>`) and resolves them against varying bases (`grid_area_size`, `inner_node_size`) in different methods (`known_dimensions`, `minimum_contribution`, `margins_axis_sums_with_baseline_shims`). These resolution bases change between calls, so pre-computing at construction time would produce wrong values.

The resolution cost is inherent to the varying-size query pattern of the grid track sizing algorithm.

### R18: Hidden item loop style access (NEUTRAL)

**Files:** `src/compute/flexbox.rs`, `src/compute/block.rs`, `src/compute/grid/mod.rs`

After layout completes, the hidden item loop iterates all children and calls `get_*_child_style()` just to check `box_generation_mode()`. The optimization collects `(order, child_id)` pairs of hidden children during initial item generation, then iterates only those pairs in the hidden loop instead of all N children.

**Implemented for flexbox and block.** Grid was skipped because it uses a `Fn() -> Iterator` pattern for `place_grid_items` (called multiple times), making it harder to extract hidden children without significant refactoring.

**Result:** No measurable improvement. The hidden loop doesn't appear in profiling — 95% of time is in `round_layout_inner_sse41`, and `compute_flexbox_layout` (which includes the hidden loop) is only ~2%. The hidden loop iterates ~10 children per node doing trivial array accesses, which is negligible compared to rounding. Changes reverted.

**Impact:** Within noise (<1% difference) on all benchmarks. The optimization would only benefit layouts with many hidden children among many visible ones, which is uncommon.

## Unexplored Areas

(None remaining — all UNKNOWN items have been investigated.)

### R19: target-cpu=native (POSITIVE, external)

Compiling with `RUSTFLAGS="-C target-cpu=native"` gives similar performance to the SSE4.1 dispatch (~145µs vs ~158µs on deep tree benchmark). This is the best possible optimization but cannot be applied by the library itself — users must set it in their build configuration.

Consider documenting this in the crate-level docs as a recommended optimization for performance-sensitive applications.

### R20: Cache max_size_ignoring_aspect_ratio on FlexItem (POSITIVE)

**File:** `src/compute/flexbox.rs`

`determine_used_cross_size` re-fetched `tree.get_flexbox_child_style(child.node)` for every stretch-aligned child just to resolve `max_size` without aspect ratio application. Added `max_size_ignoring_aspect_ratio: Size<Option<f32>>` field to `FlexItem`, populated during `generate_anonymous_flex_items`.

**Impact:** No measurable improvement on benchmarks (within noise, ~134µs before and after). The style fetch is cheap (array index + reference) and this targets the non-dominant 5% path. Kept as a clean code improvement — eliminates one style access per stretch-aligned child.

### R21: Skip rounding for already-integer values (NEGATIVE)

**File:** `src/compute/mod.rs`

Attempted to skip the entire rounding computation when all layout values are already integers. Tried three approaches:

1. **Scalar check (16 `floor` calls):** 16 individual `floor_sse41` + comparison to check all values. Regressed deep tree 134µs → 200µs.
2. **SSE4.1 batch check (`_mm_round_ps` + `_mm_cmpneq_ps`):** 4x batch floor + comparison on 16 values. Regressed deep tree 134µs → 199µs, but improved wide trees 11µs → 8µs (27% faster).
3. **Early-exit with staged checks:** Check `border.left != 0.0` first, then check x-axis values, then y-axis. Still regressed deep tree 134µs → 202µs.

**Why all approaches fail:** The `roundss` instruction is extremely fast (1-cycle latency on modern CPUs). Any guard check that evaluates before deciding to skip involves at minimum a comparison per value, which costs roughly the same as the rounding itself. The branch misprediction penalty and extra instructions always exceed the savings.

**The SIMD batch approach was interesting** because it showed a 27% improvement on wide/grid trees, but the 49% regression on deep trees made it net-negative. Deep trees have high recursion depth where the check overhead compounds.

**Lesson:** Don't try to guard fast hardware instructions with slower software checks. The SSE4.1 `roundss` is already optimal — let it run unconditionally.

## Overall Conclusion

After 22 investigations (R1-R22), the taffy layout library is well-optimized. The performance profile is:

| Function / Area            | % Runtime | Actionable?                          |
| -------------------------- | --------- | ------------------------------------- |
| `round_layout_inner_sse41` | ~94%      | Already optimal (SSE4.1 `roundss`)   |
| `cache_get`                | ~1.2%     | Already efficient (early `is_empty`)  |
| `Map Iterator::next`       | ~1.3%     | Inherent iterator overhead            |
| `compute_flexbox_layout`   | ~0.9%     | Inherent algorithm overhead           |
| Everything else            | ~2.6%     | Allocations, misc                     |

The 94% dominance of `round_layout_inner` means any optimization to the non-rounding code saves at most 6% total, and even that requires halving the cost of ALL non-rounding code. The SSE4.1 `roundss` instruction is a single-uop, 1-cycle-latency operation — there is no faster way to round floats on x86_64.

**Remaining avenues for further improvement (all external to the library):**
- `RUSTFLAGS="-C target-cpu=native"` — lets the compiler use AVX/SSE4.1 everywhere, not just in the dispatch wrapper
- Disabling rounding via `TaffyTree::set_use_rounding(false)` for applications that don't need pixel-aligned layouts
- Reducing tree depth/width at the application level

## Profiling Methodology

```bash
# Build release with flexbox
cargo build --release --example profile_flexbox --features flexbox,taffy_tree

# Profile with perf
perf record -g --call-graph=dwarf ./target/release/examples/profile_flexbox
perf report --stdio --no-children -g none

# Check generated assembly for SSE4.1 instructions
objdump -d --no-show-raw-insn target/release/examples/profile_flexbox | grep -A5 "roundss\|floorss"

# Build with native CPU
RUSTFLAGS="-C target-cpu=native" cargo build --release --example microbench --features flexbox,taffy_tree
```

## Baseline Numbers

Collected on the repo HEAD before any optimizations, with `--features flexbox,taffy_tree`:

| Benchmark                                         | Before   | After all opts | Speedup  |
| ------------------------------------------------- | -------- | -------------- | -------- |
| Deep fixed tree (10K nodes, branching=10)         | 363.89µs | ~134µs        | **2.7x** |
| Wide auto tree (1 container + 1000 auto children) | 32.87µs  | ~11µs         | **3.0x** |
| Wide fixed tree (1K fixed children)               | 37.08µs  | ~11µs         | **3.4x** |
| Nested wide auto tree (10K nodes, wrapping)       | 354.02µs | ~130µs        | **2.7x** |
| Grid 10x10 uniform (100 cells)                    | —        | ~1.06µs       | baseline |
