use taffy::prelude::*;
use taffy::style::Style as TaffyStyle;
use taffy::TaffyTree;

fn main() {
    let style = Style { flex_grow: 1.0, size: length(10.0), ..Default::default() };

    let mut tree: TaffyTree = TaffyTree::new();
    let root = build_deep_tree(&mut tree, 10_000, 10, &style);

    for _ in 0..10 {
        tree.compute_layout(
            root,
            Size { width: AvailableSpace::Definite(1000.0), height: AvailableSpace::Definite(1000.0) },
        )
        .unwrap();
    }

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        tree.compute_layout(
            root,
            Size { width: AvailableSpace::Definite(1000.0), height: AvailableSpace::Definite(1000.0) },
        )
        .unwrap();
    }
    let elapsed = start.elapsed();
    eprintln!("1000 iterations: {:?}", elapsed);
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
