use super::compile::{Command, CommandExecutor, NodeIndex, compile_layout, execute_transaction};
use super::{DockLayout, DockLayoutError, DockSplit, DockspaceTarget};
use crate::{DockNodeFlags, Id, sys};
use std::ffi::CStr;

#[test]
fn target_rejects_zero_root_and_invalid_geometry() {
    assert!(matches!(
        DockspaceTarget::new(Id::from(0), [0.0, 0.0], [100.0, 100.0]),
        Err(DockLayoutError::ZeroRootId)
    ));
    assert!(matches!(
        DockspaceTarget::new(Id::from(1), [f32::NAN, 0.0], [100.0, 100.0]),
        Err(DockLayoutError::NonFiniteInitialPosition { position })
            if position[0].is_nan() && position[1] == 0.0
    ));
    assert!(matches!(
        DockspaceTarget::new(Id::from(1), [0.0, 0.0], [0.0, 100.0]),
        Err(DockLayoutError::InvalidInitialSize { size }) if size == [0.0, 100.0]
    ));

    let private_flag = DockNodeFlags::from_bits_retain(sys::ImGuiDockNodeFlags_DockSpace);
    let target = DockspaceTarget::new(Id::from(1), [0.0, 0.0], [100.0, 100.0])
        .unwrap()
        .flags(private_flag);
    assert_eq!(
        target.validate(),
        Err(DockLayoutError::UnsupportedDockNodeFlags {
            bits: sys::ImGuiDockNodeFlags_DockSpace
        })
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
fn window_titles_reject_empty_nul_and_duplicate_stable_ids() {
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
        })
    );

    DockLayout::tabs(["Left###First", "Right###Second"])
        .validate()
        .unwrap();

    assert_eq!(
        DockLayout::tabs(["Panel#####id", "##id"]).validate(),
        Err(DockLayoutError::DuplicateWindowId {
            first_title: "Panel#####id".to_owned(),
            second_title: "##id".to_owned(),
        })
    );
    assert_eq!(
        DockLayout::tabs(["Panel####", "#"]).validate(),
        Err(DockLayoutError::DuplicateWindowId {
            first_title: "Panel####".to_owned(),
            second_title: "#".to_owned(),
        })
    );
}

#[derive(Default)]
struct FailingExecutor {
    split_calls: usize,
    fail_split_at: usize,
    finished: Vec<sys::ImGuiID>,
    rolled_back: Vec<sys::ImGuiID>,
}

impl CommandExecutor for FailingExecutor {
    fn split_node(
        &mut self,
        _parent: sys::ImGuiID,
        _direction: DockSplit,
        _ratio: f32,
    ) -> Option<(sys::ImGuiID, sys::ImGuiID)> {
        let call = self.split_calls;
        self.split_calls += 1;
        if call == self.fail_split_at {
            None
        } else {
            Some((100 + call as u32 * 2, 101 + call as u32 * 2))
        }
    }

    fn dock_window(&mut self, _title: &CStr, _node: sys::ImGuiID) {}

    fn finish(&mut self, root: sys::ImGuiID) {
        self.finished.push(root);
    }

    fn rollback(&mut self, root: sys::ImGuiID) {
        self.rolled_back.push(root);
    }
}

#[test]
fn command_failure_rolls_back_instead_of_finishing_a_partial_tree() {
    let layout = DockLayout::split(
        DockSplit::Left,
        0.5,
        DockLayout::tabs(["Left"]),
        DockLayout::split(
            DockSplit::Down,
            0.5,
            DockLayout::tabs(["Bottom"]),
            DockLayout::tabs(["Center"]),
        ),
    );
    let compiled = compile_layout(&layout).unwrap();
    let mut executor = FailingExecutor {
        fail_split_at: 1,
        ..FailingExecutor::default()
    };

    assert!(matches!(
        execute_transaction(Id::from(7), &compiled, &mut executor),
        Err(DockLayoutError::SplitFailed {
            direction: DockSplit::Down,
            ratio: 0.5,
        })
    ));
    assert!(executor.finished.is_empty());
    assert_eq!(executor.rolled_back, [7]);
}

#[test]
fn empty_tabs_are_an_intentional_valid_leaf() {
    let layout = DockLayout::tabs(std::iter::empty::<String>());
    let compiled = compile_layout(&layout).unwrap();
    assert_eq!(compiled.node_count, 1);
    assert!(compiled.commands.is_empty());
}

#[test]
fn nested_splits_compile_in_stable_preorder() {
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
        compiled.commands,
        vec![
            Command::Split {
                parent: NodeIndex(0),
                direction: DockSplit::Left,
                ratio: 0.25,
                first: NodeIndex(1),
                second: NodeIndex(2),
            },
            Command::Split {
                parent: NodeIndex(1),
                direction: DockSplit::Down,
                ratio: 0.4,
                first: NodeIndex(3),
                second: NodeIndex(4),
            },
            Command::DockWindow {
                node: NodeIndex(3),
                title: "Bottom Left",
            },
            Command::DockWindow {
                node: NodeIndex(4),
                title: "Top Left",
            },
            Command::DockWindow {
                node: NodeIndex(2),
                title: "Center",
            },
            Command::DockWindow {
                node: NodeIndex(2),
                title: "Inspector",
            },
        ]
    );
}
