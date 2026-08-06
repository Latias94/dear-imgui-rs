use super::*;
use crate::{ArraySettings, MapSettings};
use std::collections::BTreeMap;

use crate::test_guard;
use crate::{ImGuiValue, Inspector, ReflectEvent, ReflectSession};

fn new_test_ctx() -> imgui::Context {
    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx
}

#[derive(Clone, Debug, Default)]
struct Probe {
    id: usize,
}

impl ImGuiValue for Probe {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let _ = (inspector.ui(), label);
        let id = value.id;
        inspector.record_event(ReflectEvent::VecInserted {
            path: inspector.current_path(),
            index: id,
        });
        false
    }
}

#[test]
fn nested_vec_element_paths_include_index_segments() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

    let mut values = vec![Probe { id: 0 }, Probe { id: 1 }];
    let vec_settings = VecSettings {
        insertable: false,
        removable: false,
        reorderable: false,
        dropdown: false,
    };

    let session = ReflectSession::new();
    let resp = {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let path = inspector.push_path_static("items");
        let _ = imgui_vec_with_settings(&mut inspector, "items", &mut values, &vec_settings);
        drop(path);
        inspector.into_response()
    };
    drop(ctx.render_legacy());

    let paths: Vec<Option<String>> = resp
        .events()
        .iter()
        .filter_map(|event| match event {
            ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].as_deref(), Some("items[0]"));
    assert_eq!(paths[1].as_deref(), Some("items[1]"));
}

#[test]
fn nested_array_element_paths_include_index_segments() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

    let mut values = [Probe { id: 0 }, Probe { id: 1 }];
    let arr_settings = ArraySettings {
        dropdown: false,
        reorderable: false,
    };

    let session = ReflectSession::new();
    let resp = {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let path = inspector.push_path_static("arr");
        let _ = imgui_array_with_settings(&mut inspector, "arr", &mut values, &arr_settings);
        drop(path);
        inspector.into_response()
    };
    drop(ctx.render_legacy());

    let paths: Vec<Option<String>> = resp
        .events()
        .iter()
        .filter_map(|event| match event {
            ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].as_deref(), Some("arr[0]"));
    assert_eq!(paths[1].as_deref(), Some("arr[1]"));
}

#[test]
fn nested_map_value_paths_include_key_segments() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

    let mut map = BTreeMap::from([
        ("a".to_owned(), Probe { id: 0 }),
        ("b\"c".to_owned(), Probe { id: 1 }),
    ]);
    let map_settings = MapSettings {
        dropdown: false,
        insertable: false,
        removable: false,
        use_table: false,
        columns: 3,
    };

    let session = ReflectSession::new();
    let resp = {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let path = inspector.push_path_static("map");
        let _ = imgui_btree_map_with_settings(&mut inspector, "map", &mut map, &map_settings);
        drop(path);
        inspector.into_response()
    };
    drop(ctx.render_legacy());

    let paths: Vec<Option<String>> = resp
        .events()
        .iter()
        .filter_map(|event| match event {
            ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].as_deref(), Some("map[\"a\"]"));
    assert_eq!(paths[1].as_deref(), Some("map[\"b\\\"c\"]"));
}

#[test]
fn nested_tuple_element_paths_include_index_segments() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

    let mut tuple = (Probe { id: 0 }, Probe { id: 1 });
    let session = ReflectSession::new();
    let resp = {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let path = inspector.push_path_static("tup");
        let _ = <(Probe, Probe) as ImGuiValue>::imgui_value(&mut inspector, "tup", &mut tuple);
        drop(path);
        inspector.into_response()
    };
    drop(ctx.render_legacy());

    let paths: Vec<Option<String>> = resp
        .events()
        .iter()
        .filter_map(|event| match event {
            ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].as_deref(), Some("tup[0]"));
    assert_eq!(paths[1].as_deref(), Some("tup[1]"));
}

#[test]
fn nested_containers_render_without_reentrant_settings_state() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let mut session = ReflectSession::new();
    session.settings_mut().vec_mut().dropdown = false;
    let mut values = vec![vec![1_i32, 2_i32]];

    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let _ = <Vec<Vec<i32>> as ImGuiValue>::imgui_value(&mut inspector, "nested", &mut values);
    }
    drop(ctx.render_legacy());
}
