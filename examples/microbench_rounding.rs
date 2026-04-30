use std::time::Instant;
use taffy::prelude::*;
use taffy::TaffyTree;

fn main() {
    println!("=== Rounding-Focused Microbenchmark ===\n");

    let style = Style { flex_grow: 1.0, size: Size::from_lengths(10.0, 10.0), ..Default::default() };
    let mut tree: TaffyTree = TaffyTree::new();
    let root = build_deep_tree(&mut tree, 10_000, 10, &style);

    let avail = Size { width: AvailableSpace::Definite(1000.0), height: AvailableSpace::Definite(1000.0) };

    // Warmup
    for _ in 0..200 {
        tree.compute_layout(root, avail).unwrap();
    }

    let iter = 5000;

    // Benchmark: layout + rounding (default)
    tree.enable_rounding();
    let start = Instant::now();
    for _ in 0..iter {
        tree.compute_layout(root, avail).unwrap();
    }
    let with_rounding = start.elapsed();
    let per_iter_rounded = with_rounding / iter;

    // Benchmark: layout only (no rounding)
    tree.disable_rounding();
    let start = Instant::now();
    for _ in 0..iter {
        tree.compute_layout(root, avail).unwrap();
    }
    let without_rounding = start.elapsed();
    let per_iter_unrounded = without_rounding / iter;

    let rounding_overhead = per_iter_rounded - per_iter_unrounded;
    let rounding_pct = rounding_overhead.as_secs_f64() / per_iter_rounded.as_secs_f64() * 100.0;

    println!("Deep fixed tree (10K nodes, branching=10):");
    println!("  Layout + Rounding: {:.2}us/iter", per_iter_rounded.as_secs_f64() * 1e6);
    println!("  Layout only:       {:.2}us/iter", per_iter_unrounded.as_secs_f64() * 1e6);
    println!("  Rounding overhead: {:.2}us/iter ({:.1}%)", rounding_overhead.as_secs_f64() * 1e6, rounding_pct);
    println!();

    // Also bench wide tree
    let container_style = Style { display: Display::Flex, ..Default::default() };
    let leaf_style = Style { flex_grow: 1.0, ..Default::default() };
    let mut tree2: TaffyTree = TaffyTree::new();
    let children: Vec<_> = (0..1000).map(|_| tree2.new_leaf(leaf_style.clone()).unwrap()).collect();
    let root2 = tree2.new_with_children(container_style, &children).unwrap();

    for _ in 0..200 {
        tree2.compute_layout(root2, avail).unwrap();
    }

    tree2.enable_rounding();
    let start = Instant::now();
    for _ in 0..iter {
        tree2.compute_layout(root2, avail).unwrap();
    }
    let wr = start.elapsed() / iter;

    tree2.disable_rounding();
    let start = Instant::now();
    for _ in 0..iter {
        tree2.compute_layout(root2, avail).unwrap();
    }
    let wo = start.elapsed() / iter;

    let ro = wr - wo;
    let rp = ro.as_secs_f64() / wr.as_secs_f64() * 100.0;

    println!("Wide auto tree (1K auto children):");
    println!("  Layout + Rounding: {:.2}us/iter", wr.as_secs_f64() * 1e6);
    println!("  Layout only:       {:.2}us/iter", wo.as_secs_f64() * 1e6);
    println!("  Rounding overhead: {:.2}us/iter ({:.1}%)", ro.as_secs_f64() * 1e6, rp);
}

fn build_deep_tree(tree: &mut TaffyTree, max_nodes: u32, branching_factor: u32, style: &Style) -> NodeId {
    let children = build_deep_recursive(tree, max_nodes, branching_factor, style);
    tree.new_with_children(style.clone(), &children).unwrap()
}

fn build_deep_recursive(tree: &mut TaffyTree, max_nodes: u32, branching_factor: u32, style: &Style) -> Vec<NodeId> {
    if max_nodes <= branching_factor {
        return (0..max_nodes).map(|_| tree.new_leaf(style.clone()).unwrap()).collect();
    }
    (0..branching_factor)
        .map(|_| {
            let max_nodes = (max_nodes - branching_factor) / branching_factor;
            let children = build_deep_recursive(tree, max_nodes, branching_factor, style);
            tree.new_with_children(style.clone(), &children).unwrap()
        })
        .collect()
}
