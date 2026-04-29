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
| R14 | grid               | Merge triple filter in `distribute_item_space_to_growth_limit`                                   | **UNKNOWN**    | —          |
| R15 | grid               | Pre-compute `padding_border_size` on `GridItem`                                                  | **NOT VIABLE** | —          |
| R16 | grid               | Cache resolved margins on `GridItem`                                                             | **NOT VIABLE** | —          |
| R17 | block              | Double padding/border resolution in `compute_block_layout` + `compute_inner`                     | **UNKNOWN**    | —          |
| R18 | flexbox/block/grid | Hidden item loop calls `get_*_child_style` for all children                                      | **UNKNOWN**    | —          |
| R19 | build              | `RUSTFLAGS="-C target-cpu=native"`                                                               | **POSITIVE**   | —          |

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

## Unexplored Areas

### R14: Grid distribute_item_space_to_growth_limit

The function filters tracks 3 times (count, count again, apply). Could potentially merge into a single count+apply pass. Dependent on understanding why R13's merge approach regressed — the same subtlety around filter conditions likely applies.

### R17: Block double padding/border resolution

`compute_block_layout` resolves padding/border against `parent_size.width` (lines 257-258). Then `compute_inner` resolves the same values again from `raw_padding`/`raw_border` (lines 341-342) with the same basis. Could pass the already-resolved values into `compute_inner`, but this requires changing the function signature and `compute_inner` also re-resolves against `container_outer_width` later (lines 436-437), so the benefit is limited to avoiding one of two necessary resolutions.

### R18: Hidden item loop style access

After layout completes, the hidden item loop iterates all children and calls `get_*_child_style()` just to check `box_generation_mode()`. This could be tracked during initial item generation as a bitmap or separate vec, avoiding N style accesses where N is total child count.

**Applies to:** flexbox (line 378), block (line 483), grid (line 561).

### R19: target-cpu=native (POSITIVE, external)

Compiling with `RUSTFLAGS="-C target-cpu=native"` gives similar performance to the SSE4.1 dispatch (~145µs vs ~158µs on deep tree benchmark). This is the best possible optimization but cannot be applied by the library itself — users must set it in their build configuration.

Consider documenting this in the crate-level docs as a recommended optimization for performance-sensitive applications.

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
| Deep fixed tree (10K nodes, branching=10)         | 363.89µs | ~158µs         | **2.3x** |
| Wide auto tree (1 container + 1000 auto children) | 32.87µs  | ~13µs          | **2.5x** |
| Wide fixed tree (1K fixed children)               | 37.08µs  | ~13µs          | **2.9x** |
| Nested wide auto tree (10K nodes, wrapping)       | 354.02µs | ~166µs         | **2.1x** |
| Grid 10x10 uniform (100 cells)                    | —        | ~1.16µs        | baseline |
