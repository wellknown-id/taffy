# Clay vs Taffy: Flexbox Feature Comparison

> **Clay** v0.14 — Immediate-mode UI layout library (C, single-header, ~5000 LOC)
> **Taffy** — Retained-mode UI layout library (Rust, ~15K+ LOC across multiple modules)

## Executive Summary

Clay implements a **simplified subset** of CSS Flexbox. It covers the most common layout patterns (single-direction stacking, grow-to-fill, center alignment, padding, gaps) but omits many CSS Flexbox features. Taffy implements the **full W3C CSS Flexbox specification** (with a few noted TODOs) and additionally supports CSS Grid, Block layout, and Float layout.

**Rough estimate: Clay covers ~30-35% of the CSS Flexbox spec surface area.**

---

## 1. Architecture Comparison

| Aspect | Clay | Taffy |
|---|---|---|
| Language | C (single-header) | Rust (modular crate) |
| Paradigm | Immediate-mode (rebuild tree every frame) | Retained-mode (persistent tree, dirty-marking) |
| Memory | User-provided arena, no malloc | Owns node storage, optional caching |
| Layout algorithms | Simplified flexbox only | Flexbox + Grid + Block + Float + Leaf |
| Output | Flat render command array | Per-node `Layout` struct (position, size, order) |
| Text | Built-in text layout with user measurement callback | No text layout (leaf nodes only) |
| Scrolling | Built-in scroll containers with momentum | No scroll handling (content-size tracking only) |
| Input | Pointer state, hover, click callbacks | None (pure layout) |
| Debug | Built-in visual debug view | No built-in debug view |

---

## 2. Flexbox Property Comparison

### 2.1 Flex Container Properties

| CSS Property | Clay | Taffy | Notes |
|---|---|---|---|
| **flex-direction** | Partial | Full | |
| `row` | `CLAY_LEFT_TO_RIGHT` | `Row` | |
| `column` | `CLAY_TOP_TO_BOTTOM` | `Column` | |
| `row-reverse` | — | `RowReverse` | Clay: missing |
| `column-reverse` | — | `ColumnReverse` | Clay: missing |
| **flex-wrap** | — | Full | Clay: completely missing |
| `nowrap` | (always) | `NoWrap` | |
| `wrap` | — | `Wrap` | |
| `wrap-reverse` | — | `WrapReverse` | |
| **justify-content** | Partial | Full | |
| `flex-start` / `start` | `CLAY_ALIGN_X_LEFT` / `CLAY_ALIGN_Y_TOP` | `Start` / `FlexStart` | |
| `center` | `CLAY_ALIGN_X_CENTER` / `CLAY_ALIGN_Y_CENTER` | `Center` | |
| `flex-end` / `end` | `CLAY_ALIGN_X_RIGHT` / `CLAY_ALIGN_Y_BOTTOM` | `End` / `FlexEnd` | |
| `space-between` | — | `SpaceBetween` | Clay: missing |
| `space-around` | — | `SpaceAround` | Clay: missing |
| `space-evenly` | — | `SpaceEvenly` | Clay: missing |
| `stretch` | — | `Stretch` | Clay: missing |
| **align-items** | Partial | Full | |
| `start` / `flex-start` | Via `childAlignment` | `Start` / `FlexStart` | |
| `center` | Via `childAlignment` | `Center` | |
| `end` / `flex-end` | Via `childAlignment` | `End` / `FlexEnd` | |
| `stretch` | — | `Stretch` | Clay: missing (GROW on cross-axis is similar) |
| `baseline` | — | `Baseline` | Clay: missing |
| **align-content** | — | Full | Clay: completely missing (no multi-line) |
| **gap** | Partial | Full | |
| `gap` (main axis) | `childGap` | `Size<LengthPercentage>` | |
| `row-gap` | — | `Size<LengthPercentage>.width` | Clay: missing |
| `column-gap` | — | `Size<LengthPercentage>.height` | Clay: missing |
| Percentage gaps | — | Yes | Clay: integer gap only |

### 2.2 Flex Item Properties

| CSS Property | Clay | Taffy | Notes |
|---|---|---|---|
| **flex-grow** | Partial | Full | Clay: all GROW elements share equally (no variable factor) |
| **flex-shrink** | Implicit | Full | Clay: largest-first compression, no per-item factor |
| **flex-basis** | — | Full | Clay: missing (sizing types replace this concept) |
| **align-self** | — | Full (7 values) | Clay: completely missing |
| **order** | — | Full | Clay: completely missing |

### 2.3 Box Model Properties

| CSS Property | Clay | Taffy | Notes |
|---|---|---|---|
| **width / height** | Full | Full | |
| Fixed size | `CLAY_SIZING_FIXED(n)` | `Dimension::Length(n)` | |
| Percentage | `CLAY_SIZING_PERCENT(p)` | `Dimension::Percent(p)` | |
| Fit-content | `CLAY_SIZING_FIT(min, max)` | `Dimension::Auto` + min/max | |
| Grow (fill) | `CLAY_SIZING_GROW(min, max)` | `flex-grow: 1` + `Dimension::Auto` | |
| `auto` | `FIT` is closest | `Dimension::Auto` | Semantic difference |
| **min-width / min-height** | Partial | Full | Clay: via `Clay_SizingMinMax` on sizing types only |
| **max-width / max-height** | Partial | Full | Clay: via `Clay_SizingMinMax` on sizing types only |
| **padding** | Full | Full | Clay: uint16 per side; Taffy: LengthPercentage per side |
| **margin** | — | Full (LengthPercentageAuto) | Clay: completely missing |
| **border** | — | Full (LengthPercentage) | Clay: visual borders only (no layout effect) |
| **box-sizing** | — (content-box implicit) | Full | Clay: always content-box behavior |
| **aspect-ratio** | Full | Full | Both support it |
| **position** | Partial | Full | |
| `relative` | (always, normal flow) | `Relative` | |
| `absolute` | Via `Clay_FloatingElementConfig` | `Absolute` | Clay: floating elements, not true CSS absolute |
| **inset (top/right/bottom/left)** | Partial | Full | Clay: floating offset + attach points |
| **overflow** | Partial | Full | |
| `visible` | (default, unclipped) | `Visible` | |
| `hidden` | `Clay_ClipElementConfig` | `Hidden` | |
| `scroll` | Clip + built-in scrolling | `Scroll` | Taffy: reserves scrollbar gutter only |
| `clip` | Same as hidden | `Clip` | |
| **display** | Partial | Full | |
| `flex` | (always) | `Flex` | |
| `grid` | — | `Grid` | |
| `block` | — | `Block` | |
| `none` | — | `None` | |
| **direction (RTL)** | — | Full | Clay: no RTL support |

---

## 3. Algorithm Completeness

### Clay's Layout Algorithm

Clay uses a custom layout algorithm that is **inspired by flexbox but not spec-compliant**:

1. **Element tree construction** — sizes are accumulated bottom-up as elements are declared
2. **X-axis sizing** (BFS) — expand PERCENT, compress/expand GROW, handle min/max
3. **Text wrapping** — word-based wrapping with measurement cache
4. **Aspect ratio** — apply height from width
5. **Height propagation** (DFS) — update parent heights from children
6. **Y-axis sizing** (BFS) — same as X-axis but for the vertical axis
7. **Final positioning** — DFS with alignment, scroll offsets, floating element placement

### Taffy's Layout Algorithm

Taffy implements the **W3C CSS Flexbox specification** (Section 9) with all defined steps:

1. **Generate anonymous flex items** — filter display:none/absolute, resolve styles
2. **Line length determination** — compute available space, flex base size (5 sub-rules A-E)
3. **Main size determination** — collect flex lines (wrapping), resolve flexible lengths (freeze loop)
4. **Cross size determination** — hypothetical cross sizes, baseline calculation, cross sizes per line, align-content stretch
5. **Main-axis alignment** — auto margins, justify-content (all 9 modes), reverse handling
6. **Cross-axis alignment** — per-item alignment (align-self/align-items), align-content for multi-line
7. **Final layout** — absolute positioning, RTL handling, content-size tracking

### Key Algorithmic Differences

| Aspect | Clay | Taffy |
|---|---|---|
| Wrapping | Not supported | Full multi-line flex-wrap with line collection |
| Flex grow/shrink | Equal distribution | Proportional (flex-grow/flex-shrink factors) |
| Baseline alignment | Not supported | Full baseline calculation |
| Absolute positioning | Floating elements (separate tree roots) | Inline with flex algorithm (spec-compliant) |
| RTL | Not supported | Full direction handling |
| Content-based auto min-size | Not implemented | Per CSS Flexbox spec section 4.5 |
| Flex-basis resolution | Not applicable | All 5 spec rules (A-E) |
| Freeze loop (flex resolving) | Not applicable | Full spec freeze-loop with violation handling |
| Min/max violations | Handled per sizing type | Explicit violation detection and correction |

---

## 4. Features Clay Has That Taffy Does Not

Clay is a **full UI framework** rather than a pure layout library. It provides many features outside the scope of Taffy:

| Feature | Clay | Taffy |
|---|---|---|
| Text layout | Built-in word wrapping, measurement cache, alignment | None (leaf nodes only) |
| Scrolling | Built-in momentum scrolling, touch support | None (content-size tracking only) |
| Input handling | Pointer state, hover detection, click callbacks | None |
| Rendering | Render command generation (rect, text, border, image, custom) | None (outputs `Layout` structs only) |
| Debug view | Built-in visual debug overlay | None |
| Floating/anchoring | Rich 9-point anchor system with attach-to-element | No equivalent (only absolute positioning) |
| Transitions/animation | Enter/exit transitions with easing | None |
| Borders (visual) | Per-side border rendering with between-children borders | None (border only affects layout) |
| Corner radius | Per-corner radius | None |
| Background colors | Per-element | None |
| Image rendering | Image element config | None |
| Custom elements | User-data pointer for custom rendering | None (measure function only) |
| Overlay tinting | Overlay color applied to subtrees | None |
| Visibility culling | Offscreen element skipping | None |
| SIMD hashing | SSE2/NEON for string hashing | N/A |

---

## 5. Property Value Support Matrix

### Alignment Values

| Value | Clay justify | Clay align-items | Taffy justify-content | Taffy align-items / align-self |
|---|---|---|---|---|
| `start` | Yes | Yes | Yes | Yes |
| `end` | Yes | Yes | Yes | Yes |
| `flex-start` | (same as start) | (same as start) | Yes | Yes |
| `flex-end` | (same as end) | (same as end) | Yes | Yes |
| `center` | Yes | Yes | Yes | Yes |
| `stretch` | — | — | Yes | Yes |
| `baseline` | — | — | — | Yes |
| `space-between` | — | N/A | Yes | N/A |
| `space-around` | — | N/A | Yes | N/A |
| `space-evenly` | — | N/A | Yes | N/A |

### Sizing Values

| Value | Clay | Taffy |
|---|---|---|
| Fixed (px) | Yes | Yes |
| Percentage | Yes | Yes |
| Auto / Fit-content | Yes (FIT) | Yes |
| Grow / Fill | Yes (GROW) | Yes (via flex-grow + auto) |
| Min-content | — | Yes |
| Max-content | — | Yes |
| `calc()` | — | Yes (feature-gated) |
| Min/max constraints | Yes (on sizing type) | Yes (independent properties) |

---

## 6. Summary Scorecard

| Category | Clay | Taffy | Coverage |
|---|---|---|---|
| Flex container props | 4/7 | 7/7 | 57% |
| Flex item props | 1/5 | 5/5 | 20% |
| Box model props | 6/14 | 14/14 | 43% |
| Alignment values | 3/10 | 10/10 | 30% |
| Sizing values | 4/7 | 7/7 | 57% |
| Algorithm phases | 3/7 | 7/7 | 43% |
| **Overall Flexbox** | **~21/50** | **50/50** | **~42%** |
| Layout modes (flexbox only) | 1 | 5 | — |
| Non-layout features (text, scroll, input, etc.) | 13 | 0 | — |

### Interpretation

- **Clay's flexbox is approximately 30-35% spec-complete** when measured against the full CSS Flexbox specification.
- Clay covers the **"80/20" of common UI layout**: single-direction flex, grow-to-fill, centering, padding, and gaps. Many real-world UIs can be built with just these features.
- Clay compensates for limited flexbox support with a **rich set of non-layout features** (text, scrolling, input, rendering, debug) that Taffy deliberately does not provide.
- Taffy is a **pure layout engine** focused on spec compliance and correctness, while Clay is a **complete immediate-mode UI framework** with a simplified layout model.

---

## 7. Recommendations for Clay

If Clay wants to improve flexbox compatibility, the highest-impact additions would be:

1. **Flex-wrap** — Enables responsive layouts, multi-row toolbars, tag clouds, etc. This is the single biggest missing feature.
2. **Margins** — Essential for spacing individual items differently from their siblings.
3. **Variable flex-grow/flex-shrink** — Allows fine-grained control over how space is distributed.
4. **align-self** — Per-item cross-axis alignment is very commonly needed.
5. **space-between / space-around / space-evenly** — Common distribution patterns for navigation bars, card grids, etc.
6. **Reverse directions** — row-reverse and column-reverse are important for RTL-aware layouts.
7. **Separate row-gap / column-gap** — Essential when flex-wrap is added.
