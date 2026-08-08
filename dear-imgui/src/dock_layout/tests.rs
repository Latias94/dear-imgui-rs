use super::compile::{NodeIndex, SplitCommand, WindowAssignment, compile_layout};
use super::{DockLayout, DockSplit, DockspaceConfig, DockspaceError};
use crate::{DockNodeFlags, Id, WindowKey, sys};

fn key(stable_id: &str) -> WindowKey {
    WindowKey::new(stable_id, stable_id).unwrap()
}

#[test]
fn config_rejects_zero_root_and_unknown_flags() {
    assert!(matches!(
        DockspaceConfig::new(Id::from(0), DockNodeFlags::NONE, None).validate(),
        Err(DockspaceError::ZeroRootId)
    ));

    let unknown_bits = sys::ImGuiDockNodeFlags_DockSpace;
    assert_eq!(unknown_bits & DockNodeFlags::all().bits(), 0);
    assert!(matches!(
        DockspaceConfig::new(
            Id::from(1),
            DockNodeFlags::from_bits_retain(unknown_bits),
            None,
        )
        .validate(),
        Err(DockspaceError::UnsupportedDockNodeFlags { bits }) if bits == unknown_bits
    ));
}

#[test]
fn split_ratios_must_be_finite_and_strictly_inside_unit_interval() {
    for ratio in [f32::NAN, f32::INFINITY, 0.0, 1.0, -0.1, 1.1] {
        let layout = DockLayout::split(
            DockSplit::Left,
            ratio,
            DockLayout::tabs([key("left")]),
            DockLayout::tabs([key("right")]),
        );
        assert!(matches!(
            layout.validate(),
            Err(DockspaceError::InvalidSplitRatio { ratio: actual })
                if actual.to_bits() == ratio.to_bits()
        ));
    }

    DockLayout::split(
        DockSplit::Left,
        0.5,
        DockLayout::tabs([key("left")]),
        DockLayout::tabs([key("right")]),
    )
    .validate()
    .unwrap();
}

#[test]
fn window_keys_reject_duplicate_stable_ids_and_real_hash_collisions() {
    let duplicate = DockLayout::split(
        DockSplit::Right,
        0.4,
        DockLayout::tabs([WindowKey::new("shared", "Left").unwrap()]),
        DockLayout::tabs([key("other"), WindowKey::new("shared", "Right").unwrap()]),
    );
    assert_eq!(
        duplicate.validate(),
        Err(DockspaceError::DuplicateWindowKey {
            first_key: "shared".to_owned(),
            second_key: "shared".to_owned(),
            id: key("shared").native_id(),
        })
    );

    // This pair collides under Dear ImGui's current CRC32C hash, including both the
    // portable lookup-table implementation and the optional SSE4.2 implementation.
    const FIRST_COLLISION: &str = "Dock_31B880E0DEB60BA1";
    const SECOND_COLLISION: &str = "Dock_FF90DC3128AFC905";
    let collision_id = key(FIRST_COLLISION).native_id();
    assert_ne!(collision_id.raw(), 0);
    assert_eq!(key(SECOND_COLLISION).native_id(), collision_id);
    assert_eq!(
        DockLayout::tabs([key(FIRST_COLLISION), key(SECOND_COLLISION)]).validate(),
        Err(DockspaceError::DuplicateWindowKey {
            first_key: FIRST_COLLISION.to_owned(),
            second_key: SECOND_COLLISION.to_owned(),
            id: collision_id,
        })
    );
}

#[test]
fn empty_tabs_compile_to_an_unassigned_leaf() {
    let compiled = compile_layout(&DockLayout::tabs(std::iter::empty::<WindowKey>())).unwrap();
    assert_eq!(compiled.node_count, 1);
    assert!(compiled.splits.is_empty());
    assert!(compiled.assignments.is_empty());
}

#[test]
fn nested_splits_and_owned_titles_compile_in_stable_preorder() {
    let layout = DockLayout::split(
        DockSplit::Left,
        0.25,
        DockLayout::split(
            DockSplit::Down,
            0.4,
            DockLayout::tabs([key("bottom-left")]),
            DockLayout::tabs([key("top-left")]),
        ),
        DockLayout::tabs([key("center"), key("inspector")]),
    );

    let compiled = compile_layout(&layout).unwrap();
    assert_eq!(compiled.node_count, 5);
    assert_eq!(
        compiled.splits,
        vec![
            SplitCommand {
                parent: NodeIndex(0),
                direction: DockSplit::Left,
                ratio: 0.25,
                first: NodeIndex(1),
                second: NodeIndex(2),
            },
            SplitCommand {
                parent: NodeIndex(1),
                direction: DockSplit::Down,
                ratio: 0.4,
                first: NodeIndex(3),
                second: NodeIndex(4),
            },
        ]
    );
    assert_eq!(
        compiled.assignments,
        vec![
            WindowAssignment {
                node: NodeIndex(3),
                key: key("bottom-left"),
            },
            WindowAssignment {
                node: NodeIndex(4),
                key: key("top-left"),
            },
            WindowAssignment {
                node: NodeIndex(2),
                key: key("center"),
            },
            WindowAssignment {
                node: NodeIndex(2),
                key: key("inspector"),
            },
        ]
    );
    assert_eq!(
        compiled.assignments[0]
            .key
            .docking_name()
            .to_bytes_with_nul(),
        b"###bottom-left\0"
    );
}
