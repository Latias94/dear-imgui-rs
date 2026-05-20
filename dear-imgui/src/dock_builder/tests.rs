use super::DockBuilder;
use super::validation::dock_node_depth_from_i32;
use crate::Id;

#[test]
fn dock_node_depth_rejects_negative_raw_values() {
    assert_eq!(dock_node_depth_from_i32(0), 0);
    assert_eq!(dock_node_depth_from_i32(3), 3);
    assert!(
        std::panic::catch_unwind(|| {
            let _ = dock_node_depth_from_i32(-1);
        })
        .is_err()
    );
}

#[test]
fn copy_dock_space_with_window_remap_rejects_interior_nul_names() {
    assert!(
        std::panic::catch_unwind(|| {
            DockBuilder::copy_dock_space_with_window_remap(
                Id::from(1),
                Id::from(2),
                &[("bad\0src", "dst")],
            );
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            DockBuilder::copy_dock_space_with_window_remap(
                Id::from(1),
                Id::from(2),
                &[("src", "bad\0dst")],
            );
        })
        .is_err()
    );
}
