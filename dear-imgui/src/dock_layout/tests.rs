use super::compile::{NodeIndex, SplitCommand, WindowAssignment, compile_layout};
use super::{DockLayout, DockLayoutError, DockSplit, DockspaceOptions};
use crate::{DockNodeFlags, Id, sys};
use std::ffi::CString;

fn native_window_id(title: &str) -> Id {
    let title = CString::new(title).expect("test title must not contain an interior NUL");
    // SAFETY: `title` is readable and NUL-terminated. ImHashStr does not require a Context.
    Id::from(unsafe { sys::igImHashStr(title.as_ptr(), 0, 0) })
}

#[test]
fn options_reject_zero_root_and_unknown_flags() {
    assert!(matches!(
        DockspaceOptions::new(Id::from(0)),
        Err(DockLayoutError::ZeroRootId)
    ));

    let unknown_bits = sys::ImGuiDockNodeFlags_DockSpace;
    assert_eq!(unknown_bits & DockNodeFlags::all().bits(), 0);
    let options = DockspaceOptions::new(Id::from(1))
        .unwrap()
        .flags(DockNodeFlags::from_bits_retain(unknown_bits));
    assert_eq!(
        options.validate(),
        Err(DockLayoutError::UnsupportedDockNodeFlags { bits: unknown_bits })
    );
}

#[test]
fn split_ratios_must_be_finite_and_strictly_inside_unit_interval() {
    for ratio in [f32::NAN, f32::INFINITY, 0.0, 1.0, -0.1, 1.1] {
        let layout = DockLayout::split(
            DockSplit::Left,
            ratio,
            DockLayout::tabs(["Left"]),
            DockLayout::tabs(["Right"]),
        );
        assert!(matches!(
            layout.validate(),
            Err(DockLayoutError::InvalidSplitRatio { ratio: actual })
                if actual.to_bits() == ratio.to_bits()
        ));
    }

    DockLayout::split(
        DockSplit::Left,
        0.5,
        DockLayout::tabs(["Left"]),
        DockLayout::tabs(["Right"]),
    )
    .validate()
    .unwrap();
}

#[test]
fn window_titles_reject_empty_nul_and_empty_stable_ids() {
    assert_eq!(
        DockLayout::tabs([""]).validate(),
        Err(DockLayoutError::EmptyWindowTitle)
    );
    assert_eq!(
        DockLayout::tabs(["bad\0title"]).validate(),
        Err(DockLayoutError::WindowTitleContainsNul {
            title: "bad\0title".to_owned()
        })
    );
    assert_eq!(
        DockLayout::tabs(["Visible title###"]).validate(),
        Err(DockLayoutError::EmptyWindowId {
            title: "Visible title###".to_owned()
        })
    );
}

#[test]
fn window_titles_reject_stable_id_and_real_hash_collisions() {
    let duplicate = DockLayout::split(
        DockSplit::Right,
        0.4,
        DockLayout::tabs(["Left###Shared"]),
        DockLayout::tabs(["Other", "Right###Shared"]),
    );
    assert_eq!(
        duplicate.validate(),
        Err(DockLayoutError::DuplicateWindowId {
            first_title: "Left###Shared".to_owned(),
            second_title: "Right###Shared".to_owned(),
            id: native_window_id("###Shared"),
        })
    );

    // This pair collides under Dear ImGui's current CRC32C hash, including both the
    // portable lookup-table implementation and the optional SSE4.2 implementation.
    const FIRST_COLLISION: &str = "Dock_31B880E0DEB60BA1";
    const SECOND_COLLISION: &str = "Dock_FF90DC3128AFC905";
    let collision_id = native_window_id(FIRST_COLLISION);
    assert_ne!(collision_id.raw(), 0);
    assert_eq!(native_window_id(SECOND_COLLISION), collision_id);
    assert_eq!(
        DockLayout::tabs([FIRST_COLLISION, SECOND_COLLISION]).validate(),
        Err(DockLayoutError::DuplicateWindowId {
            first_title: FIRST_COLLISION.to_owned(),
            second_title: SECOND_COLLISION.to_owned(),
            id: collision_id,
        })
    );
}

#[test]
fn empty_tabs_compile_to_an_unassigned_leaf() {
    let compiled = compile_layout(&DockLayout::tabs(std::iter::empty::<String>())).unwrap();
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
            DockLayout::tabs(["Bottom Left"]),
            DockLayout::tabs(["Top Left"]),
        ),
        DockLayout::tabs(["Center", "Inspector"]),
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
                title: CString::new("Bottom Left").unwrap(),
            },
            WindowAssignment {
                node: NodeIndex(4),
                title: CString::new("Top Left").unwrap(),
            },
            WindowAssignment {
                node: NodeIndex(2),
                title: CString::new("Center").unwrap(),
            },
            WindowAssignment {
                node: NodeIndex(2),
                title: CString::new("Inspector").unwrap(),
            },
        ]
    );
    assert_eq!(
        compiled.assignments[0].title.as_bytes_with_nul(),
        b"Bottom Left\0"
    );
}
