use taffy::prelude::*;
use taffy_test_helpers::new_test_tree;

#[test]
fn nested_flex_container_exports_child_baseline_once() {
    let mut taffy = new_test_tree();

    let short = taffy
        .new_leaf(Style {
            size: Size { width: length(10.0), height: length(10.0) },
            ..Default::default()
        })
        .unwrap();
    let tall = taffy
        .new_leaf(Style {
            size: Size { width: length(10.0), height: length(20.0) },
            ..Default::default()
        })
        .unwrap();
    let nested = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                align_items: Some(AlignItems::Baseline),
                ..Default::default()
            },
            &[short, tall],
        )
        .unwrap();
    let sibling = taffy
        .new_leaf(Style {
            size: Size { width: length(10.0), height: length(20.0) },
            ..Default::default()
        })
        .unwrap();
    let root = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                align_items: Some(AlignItems::Baseline),
                ..Default::default()
            },
            &[nested, sibling],
        )
        .unwrap();

    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

    assert_eq!(taffy.layout(root).unwrap().size.height, 20.0);
    assert_eq!(taffy.layout(nested).unwrap().location.y, 0.0);
    assert_eq!(taffy.layout(sibling).unwrap().location.y, 0.0);
    assert_eq!(taffy.layout(short).unwrap().location.y, 10.0);
    assert_eq!(taffy.layout(tall).unwrap().location.y, 0.0);
}
