//! CSS Grid Lanes: a grid whose tracks in one axis are lanes that items stack along.
//! <https://drafts.csswg.org/css-grid-3/>
//!
//! The lanes are sized by the grid track sizing algorithm with every item in a single track of
//! the stacking axis, an auto-placed item counting in each of the lanes it could take (as
//! "virtual" copies of it). Items are then stacked in order, each in whichever lanes leave it
//! closest to the start, and the stacking axis' single track is as long as the longest lane.
use super::types::{CellOccupancyMatrix, CellOccupancyState, GridItem, GridTrack};
use super::{NamedLineResolver, OriginZeroLine};
use crate::geometry::{AbsoluteAxis, AbstractAxis, Line, Size};
use crate::style::{AlignItems, AvailableSpace};
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt, NodeId, SizingMode};
use crate::util::sys::Vec;
use crate::{CoreStyle, GridItemStyle};

/// The abstract axis that the lanes' tracks are sized in.
pub(super) fn lanes_abstract_axis(lanes_axis: AbsoluteAxis) -> AbstractAxis {
    match lanes_axis {
        AbsoluteAxis::Horizontal => AbstractAxis::Inline,
        AbsoluteAxis::Vertical => AbstractAxis::Block,
    }
}

/// Place a grid lanes container's items: those with a definite lane where they say, the rest in
/// the first lanes (to be chosen when they're stacked) and, virtually, in every other lane they
/// could take. Every item is in the stacking axis' one track.
pub(super) fn place_grid_lanes_items<'a, S, ChildIter>(
    cell_occupancy_matrix: &mut CellOccupancyMatrix,
    items: &mut Vec<GridItem>,
    children_iter: impl Fn() -> ChildIter,
    lanes_axis: AbsoluteAxis,
    align_items: AlignItems,
    justify_items: AlignItems,
    named_line_resolver: &NamedLineResolver<<S as CoreStyle>::CustomIdent>,
) where
    S: GridItemStyle + 'a,
    ChildIter: Iterator<Item = (usize, NodeId, S)>,
{
    let explicit_lane_count = cell_occupancy_matrix.track_counts(lanes_axis).explicit;
    let stack_span = Line { start: OriginZeroLine(0), end: OriginZeroLine(1) };
    let lane_placement = |style: &S| match lanes_axis {
        AbsoluteAxis::Horizontal => named_line_resolver
            .resolve_column_names(&style.grid_column())
            .map(|placement| placement.into_origin_zero_placement(explicit_lane_count)),
        AbsoluteAxis::Vertical => named_line_resolver
            .resolve_row_names(&style.grid_row())
            .map(|placement| placement.into_origin_zero_placement(explicit_lane_count)),
    };
    let push_item = |items: &mut Vec<GridItem>, node: NodeId, index: usize, style: S, lanes: Line<OriginZeroLine>| {
        let (column, row) = match lanes_axis {
            AbsoluteAxis::Horizontal => (lanes, stack_span),
            AbsoluteAxis::Vertical => (stack_span, lanes),
        };
        let mut item = GridItem::new_with_placement_style_and_order(
            node,
            column,
            row,
            style,
            align_items,
            justify_items,
            index as u16,
        );
        // In the stacking axis an item is as big as its content: nothing stretches it.
        let stack_alignment = match lanes_axis {
            AbsoluteAxis::Horizontal => &mut item.align_self,
            AbsoluteAxis::Vertical => &mut item.justify_self,
        };
        if *stack_alignment == AlignItems::Stretch {
            *stack_alignment = AlignItems::Start;
        }
        items.push(item);
    };

    // 1. Items with a definite lane (which can add implicit lanes)
    for (index, node, style) in children_iter() {
        let placement = lane_placement(&style);
        if !placement.is_definite() {
            continue;
        }
        let lanes = placement.resolve_definite_grid_lines();
        cell_occupancy_matrix.mark_area_as(lanes_axis, lanes, stack_span, CellOccupancyState::DefinitelyPlaced);
        push_item(items, node, index, style, lanes);
    }

    // 2. Enough lanes for the widest auto-placed item
    let mut max_auto_span = 0;
    for (_, _, style) in children_iter() {
        let placement = lane_placement(&style);
        if !placement.is_definite() {
            max_auto_span = max_auto_span.max(placement.indefinite_span());
        }
    }
    if max_auto_span == 0 {
        return;
    }
    let start_line = cell_occupancy_matrix.track_counts(lanes_axis).implicit_start_line();
    cell_occupancy_matrix.mark_area_as(
        lanes_axis,
        Line { start: start_line, end: start_line + max_auto_span },
        stack_span,
        CellOccupancyState::Unoccupied,
    );
    let start_line = cell_occupancy_matrix.track_counts(lanes_axis).implicit_start_line();
    let end_line = cell_occupancy_matrix.track_counts(lanes_axis).implicit_end_line();

    // 3. Auto-placed items, in every lane they could take
    for (index, node, style) in children_iter() {
        let placement = lane_placement(&style);
        if placement.is_definite() {
            continue;
        }
        let span = placement.indefinite_span();
        let first = Line { start: start_line, end: start_line + span };
        cell_occupancy_matrix.mark_area_as(lanes_axis, first, stack_span, CellOccupancyState::AutoPlaced);
        push_item(items, node, index, style, first);
        let item = items.last_mut().unwrap();
        item.lanes_auto = true;
        let mut copy = item.clone();
        copy.lanes_virtual = true;
        let mut start = start_line + 1;
        while start + span <= end_line {
            let lanes = Line { start, end: start + span };
            cell_occupancy_matrix.mark_area_as(lanes_axis, lanes, stack_span, CellOccupancyState::AutoPlaced);
            match lanes_axis {
                AbsoluteAxis::Horizontal => copy.column = lanes,
                AbsoluteAxis::Vertical => copy.row = lanes,
            }
            items.push(copy.clone());
            start += 1;
        }
    }
}

/// Stack a grid lanes container's items (in order) along the stacking axis, choosing auto-placed
/// items' lanes, and return the length of the longest lane.
#[allow(clippy::too_many_arguments)]
pub(super) fn stack_grid_lanes_items(
    tree: &mut impl LayoutPartialTree,
    items: &mut [GridItem],
    lanes: &[GridTrack],
    lanes_axis: AbsoluteAxis,
    stack_gap: f32,
    tolerance: f32,
    track_reverse: bool,
    inner_node_size: Size<Option<f32>>,
) -> f32 {
    let lanes_axis = lanes_abstract_axis(lanes_axis);
    let stack_axis = lanes_axis.other();
    // (Tracks alternate with the lines between them, which start and end the list.)
    let lane_count = lanes.len() / 2;
    let mut running = Vec::new();
    running.resize(lane_count, 0.0f32);
    let mut order: Vec<usize> = (0..items.len()).filter(|&index| !items[index].lanes_virtual).collect();
    order.sort_by_key(|&index| items[index].source_order);
    let mut stacked_any = false;
    for index in order {
        let item = &mut items[index];
        let indexes = item.placement_indexes(lanes_axis);
        let span = ((indexes.end - indexes.start) / 2) as usize;
        let mut first_lane = (indexes.start / 2) as usize;
        let reach = |running: &[f32], first: usize| running[first..first + span].iter().copied().fold(0.0f32, f32::max);
        if item.lanes_auto && span <= lane_count {
            let positions = 0..=(lane_count - span);
            let shortest = positions.clone().map(|first| reach(&running, first)).fold(f32::INFINITY, f32::min);
            let ties = |first: &usize| reach(&running, *first) <= shortest + tolerance;
            first_lane = if track_reverse {
                positions.rev().find(ties).unwrap_or(0)
            } else {
                positions.clone().find(ties).unwrap_or(0)
            };
            let new_indexes = Line { start: (first_lane * 2) as u16, end: ((first_lane + span) * 2) as u16 };
            match lanes_axis {
                AbstractAxis::Inline => item.column_indexes = new_indexes,
                AbstractAxis::Block => item.row_indexes = new_indexes,
            }
        }
        let last_lane = (first_lane + span).min(lane_count);
        let offset = reach(&running, first_lane.min(last_lane)).max(0.0);
        let offset = if span == 0 { 0.0 } else { offset };
        let lane_size: f32 =
            lanes[item.track_range_excluding_lines(lanes_axis)].iter().map(|track| track.base_size).sum();
        let mut available_space = Size::NONE;
        available_space.set(lanes_axis, Some(lane_size));
        let margins = item.margins_axis_sums_with_baseline_shims(inner_node_size.width, tree).get(stack_axis);
        // (The item's percentages are of its lanes.)
        let known_dimensions = item.known_dimensions(tree, inner_node_size, available_space);
        let size = tree.measure_child_size(
            item.node,
            known_dimensions,
            available_space,
            available_space.map(|space| match space {
                Some(size) => AvailableSpace::Definite(size),
                None => AvailableSpace::MaxContent,
            }),
            SizingMode::InherentSize,
            stack_axis.as_abs_naive(),
            Line::FALSE,
        ) + margins;
        item.lanes_offset = offset;
        item.lanes_size = size;
        for reach in &mut running[first_lane.min(last_lane)..last_lane] {
            *reach = offset + size + stack_gap;
        }
        stacked_any = true;
    }
    if !stacked_any {
        return 0.0;
    }
    (running.iter().copied().fold(0.0f32, f32::max) - stack_gap).max(0.0)
}
