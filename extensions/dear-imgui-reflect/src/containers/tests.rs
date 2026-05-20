use super::*;
use crate::{ArraySettings, MapSettings};
use std::collections::BTreeMap;

use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::response;
use crate::{ImGuiValue, ReflectEvent, ReflectResponse};

fn test_guard() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn new_test_ctx() -> imgui::Context {
    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx
}

#[derive(Clone, Debug, Default)]
struct Probe {
    id: usize,
}

impl ImGuiValue for Probe {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let _ = (ui, label);
        let id = value.id;
        response::record_event(ReflectEvent::VecInserted {
            path: response::current_field_path(),
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

    let mut resp = ReflectResponse::default();
    {
        let ui = ctx.frame();
        response::with_response(&mut resp, || {
            response::with_field_path("items", || {
                let _ = imgui_vec_with_settings(ui, "items", &mut values, &vec_settings);
            });
        });
    }
    ctx.render();

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

    let mut resp = ReflectResponse::default();
    {
        let ui = ctx.frame();
        response::with_response(&mut resp, || {
            response::with_field_path("arr", || {
                let _ = imgui_array_with_settings(ui, "arr", &mut values, &arr_settings);
            });
        });
    }
    ctx.render();

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

    let mut resp = ReflectResponse::default();
    {
        let ui = ctx.frame();
        response::with_response(&mut resp, || {
            response::with_field_path("map", || {
                let _ = imgui_btree_map_with_settings(ui, "map", &mut map, &map_settings);
            });
        });
    }
    ctx.render();

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
    let mut resp = ReflectResponse::default();
    {
        let ui = ctx.frame();
        response::with_response(&mut resp, || {
            response::with_field_path("tup", || {
                let _ = <(Probe, Probe) as ImGuiValue>::imgui_value(ui, "tup", &mut tuple);
            });
        });
    }
    ctx.render();

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
