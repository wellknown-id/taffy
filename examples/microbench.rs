use std::time::Instant;
use taffy::prelude::*;
use taffy::style::Style as TaffyStyle;
use taffy::TaffyTree;

fn main() {
    println!("=== Taffy Microbenchmark Suite ===\n");

    bench_deep_fixed_tree();
    bench_wide_auto_tree();
    bench_wide_fixed_tree();
    bench_nested_wide_auto_tree();
    bench_grid_uniform();
}

fn bench_grid_uniform() {
    let mut tree: TaffyTree = TaffyTree::new();
    let cell_style = Style { size: Size::from_lengths(10.0, 10.0), ..Default::default() };
    let mut children = Vec::new();
    // 10x10 grid = 100 cells
    for _ in 0..100 {
        children.push(tree.new_leaf(cell_style.clone()).unwrap());
    }
    let grid_style = Style {
        display: Display::Grid,
        grid_template_columns: vec![length(100.0); 10],
        grid_template_rows: vec![length(100.0); 10],
        ..Default::default()
    };
    let root = tree.new_with_children(grid_style, &children).unwrap();

    let per_iter = measure("Grid 10x10 uniform (100 cells)", &mut tree, root);
    println!("  Per iteration: {:.2}us\n", per_iter.as_secs_f64() * 1e6);
}

fn bench_deep_fixed_tree() {
    let style = Style { flex_grow: 1.0, size: Size::from_lengths(10.0, 10.0), ..Default::default() };
    let mut tree: TaffyTree = TaffyTree::new();
    let root = build_deep_tree(&mut tree, 10_000, 10, &style);

    let per_iter = measure("Deep fixed tree (10K nodes)", &mut tree, root);
    println!("  Per iteration: {:.2}us\n", per_iter.as_secs_f64() * 1e6);
}

fn bench_wide_auto_tree() {
    let container_style = Style { display: Display::Flex, ..Default::default() };
    let leaf_style = Style { flex_grow: 1.0, ..Default::default() };

    let mut tree: TaffyTree = TaffyTree::new();
    let children: Vec<_> = (0..1000).map(|_| tree.new_leaf(leaf_style.clone()).unwrap()).collect();
    let root = tree.new_with_children(container_style, &children).unwrap();

    let per_iter = measure("Wide auto tree (1K auto children)", &mut tree, root);
    println!("  Per iteration: {:.2}us\n", per_iter.as_secs_f64() * 1e6);
}

fn bench_wide_fixed_tree() {
    let container_style = Style { display: Display::Flex, ..Default::default() };
    let leaf_style = Style { size: Size::from_lengths(10.0, 10.0), ..Default::default() };

    let mut tree: TaffyTree = TaffyTree::new();
    let children: Vec<_> = (0..1000).map(|_| tree.new_leaf(leaf_style.clone()).unwrap()).collect();
    let root = tree.new_with_children(container_style, &children).unwrap();

    let per_iter = measure("Wide fixed tree (1K fixed children)", &mut tree, root);
    println!("  Per iteration: {:.2}us\n", per_iter.as_secs_f64() * 1e6);
}

fn bench_nested_wide_auto_tree() {
    let container_style = Style { display: Display::Flex, flex_wrap: FlexWrap::Wrap, ..Default::default() };
    let leaf_style = Style { flex_grow: 1.0, ..Default::default() };

    let mut tree: TaffyTree = TaffyTree::new();
    let mut all_children = Vec::new();
    for _ in 0..100 {
        let sub_children: Vec<_> = (0..100).map(|_| tree.new_leaf(leaf_style.clone()).unwrap()).collect();
        let container = tree.new_with_children(container_style.clone(), &sub_children).unwrap();
        all_children.push(container);
    }
    let root = tree.new_with_children(container_style, &all_children).unwrap();

    let per_iter = measure("Nested wide auto tree (10K nodes, wrapping)", &mut tree, root);
    println!("  Per iteration: {:.2}us\n", per_iter.as_secs_f64() * 1e6);
}

fn measure(label: &str, tree: &mut TaffyTree, root: NodeId) -> std::time::Duration {
    let avail = Size { width: AvailableSpace::Definite(1000.0), height: AvailableSpace::Definite(1000.0) };

    for _ in 0..100 {
        tree.compute_layout(root, avail).unwrap();
    }

    let iter = 1000;
    let start = Instant::now();
    for _ in 0..iter {
        tree.compute_layout(root, avail).unwrap();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iter;
    println!("{}:", label);
    println!("  Total: {:.2}ms for {} iterations", elapsed.as_secs_f64() * 1e3, iter);
    per_iter
}

fn build_deep_tree(tree: &mut TaffyTree, max_nodes: u32, branching_factor: u32, style: &TaffyStyle) -> NodeId {
    let children = build_deep_recursive(tree, max_nodes, branching_factor, style);
    tree.new_with_children(style.clone(), &children).unwrap()
}

fn build_deep_recursive(
    tree: &mut TaffyTree,
    max_nodes: u32,
    branching_factor: u32,
    style: &TaffyStyle,
) -> Vec<NodeId> {
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
