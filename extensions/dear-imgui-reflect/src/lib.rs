//! Reflection-based helpers for dear-imgui-rs.
//!
//! This crate provides traits, derive macros, and helpers to automatically
//! generate Dear ImGui widgets for your Rust types, in the spirit of the
//! C++ ImReflect library.
//!
//! At a high level:
//! - derive [`ImGuiReflect`](trait.ImGuiReflect.html) for your structs/enums;
//! - call [`input`] or [`ImGuiReflectExt::input_reflect`] each frame to render
//!   an editor for a value;
//! - optionally customize container / numeric behavior via
//!   [`ReflectSettings`] and [`MemberSettings`];
//! - optionally collect structural change events with
//!   [`input_with_response`].
//!
//! The goal is to let you build "data inspector" style UIs quickly without
//! hand-writing widgets for every field.
//!
//! # Quick start
//!
//! Derive [`ImGuiReflect`] for your type and use `input_reflect`:
//!
//! ```no_run
//! use dear_imgui_reflect as reflect;
//! use reflect::ImGuiReflectExt;
//!
//! #[derive(reflect::ImGuiReflect, Default)]
//! struct Player {
//!     #[imgui(slider, min = 0.0, max = 100.0)]
//!     health: f32,
//!     #[imgui(multiline, lines = 3)]
//!     notes: String,
//!     inventory: Vec<String>,
//! }
//!
//! fn draw_ui(ui: &reflect::imgui::Ui, player: &mut Player) {
//!     // Returns true if any field changed this frame.
//!     if ui.input_reflect("Player", player) {
//!         // React to edits (save, mark dirty, etc).
//!     }
//! }
//! ```
//!
//! # Supported patterns (what this crate is good at)
//!
//! This crate is designed around a few common use cases:
//!
//! - **Configuration panels / settings windows**:
//!   derive [`ImGuiReflect`] for your config structs and call
//!   [`ImGuiReflectExt::input_reflect`] each frame to build a live editor.
//! - **Game/engine inspectors**:
//!   derive [`ImGuiReflect`] for components or resource types, then render
//!   inspectors for the currently selected entity/resource inside a docked
//!   window.
//! - **Collection-heavy UIs**:
//!   use the built-in support for `Vec<T>`, `[T; N]`, `Option<T>`,
//!   `HashMap<String, V>` and `BTreeMap<String, V>` to build list views,
//!   property bags and key/value editors with insertion/removal/reordering.
//! - **Tooling and data browsers**:
//!   combine [`input_with_response`] and [`ReflectResponse`] to track when
//!   items are inserted/removed/reordered, and synchronize those changes
//!   back into your engine or persistence layer.
//! - **Math-heavy editors**:
//!   enable the `glam` or `mint` features to edit vector, quaternion and
//!   matrix types using familiar ImGui `input_float*` widgets.
//!
//! The derive macro understands a subset of field attributes inspired by
//! ImReflect, such as:
//! - `#[imgui(skip)]` – do not generate any UI for this field;
//! - `#[imgui(name = "Custom Label")]` – override the field label;
//! - numeric helpers like
//!   `#[imgui(slider, min = 0.0, max = 1.0, format = "%.2f")]`,
//!   `#[imgui(as_drag, speed = 0.1)]`, `#[imgui(as_input, step = 1)]`;
//! - text helpers like `#[imgui(multiline, lines = 4, hint = "Search...")]`,
//!   `#[imgui(read_only)]`, `#[imgui(display_only)]`;
//! - tuple layout helpers such as
//!   `#[imgui(tuple_render = "grid", tuple_columns = 3)]`.
//!
//! See the documentation on the re-exported [`ImGuiReflect` derive macro]
//! for the full list of supported attributes and validation rules.
//!
//! # Settings and per-field overrides
//!
//! The global [`ReflectSettings`] object controls how generic containers
//! are rendered (for example, whether `Vec<T>` is insertable/reorderable, or
//! how numeric sliders behave). You can adjust it at startup:
//!
//! ```no_run
//! use dear_imgui_reflect as reflect;
//!
//! fn configure_reflect() {
//!     reflect::with_settings(|s| {
//!         // Make all Vec<T> non-reorderable by default.
//!         s.vec_mut().reorderable = false;
//!
//!         // Use a 0..1 slider for all f32 values by default.
//!         let f32_settings = s.numerics_f32().clone().slider_0_to_1(2); // "%.2f"
//!         *s.numerics_f32_mut() = f32_settings;
//!     });
//! }
//! ```
//!
//! For finer control, [`MemberSettings`] lets you override behavior for a
//! specific field, identified by type and field name:
//!
//! ```no_run
//! # use dear_imgui_reflect as reflect;
//! #
//! #[derive(reflect::ImGuiReflect)]
//! struct Settings {
//!     weights: Vec<f32>,
//! }
//!
//! fn configure_per_field() {
//!     reflect::with_settings(|s| {
//!         // For Settings::weights, allow reordering only.
//!         s.for_member::<Settings>("weights")
//!             .vec_reorder_only()
//!             .numerics_f32_slider_0_to_1(3);
//!     });
//! }
//! ```
//!
//! The [`with_settings_scope`] helper lets you temporarily override global
//! settings for a single panel or widget subtree and automatically restore
//! the previous configuration afterwards.
//!
//! # Collecting structural change events
//!
//! The [`input`] and [`ImGuiReflectExt::input_reflect`] helpers return a
//! simple `bool` indicating whether any field changed. If you also want to
//! react to container-structure changes (insert/remove/reorder/rename), use
//! [`input_with_response`] and inspect the resulting [`ReflectResponse`]:
//!
//! ```no_run
//! use dear_imgui_reflect as reflect;
//!
//! #[derive(reflect::ImGuiReflect, Default)]
//! struct AppState {
//!     tags: Vec<String>,
//! }
//!
//! fn draw_tags(ui: &reflect::imgui::Ui, state: &mut AppState) {
//!     let mut resp = reflect::ReflectResponse::default();
//!     let _changed = reflect::input_with_response(ui, "Tags", state, &mut resp);
//!
//!     for event in resp.events() {
//!         match event {
//!             reflect::ReflectEvent::VecInserted { path, index } => {
//!                 // path is "tags" when generated via the derive macro.
//!                 println!("Inserted element at {index} in {:?}", path);
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```
//!
//! # Math integrations
//!
//! When the `glam` feature is enabled, this crate implements [`ImGuiValue`]
//! for `glam::Vec2/Vec3/Vec4`, `glam::Quat`, and `glam::Mat4`, using the
//! corresponding `input_float*` widgets for inspection and editing.
//!
//! When the `mint` feature is enabled, `mint::Vector2/Vector3/Vector4<f32>`
//! are also editable via `input_float*` controls. This is useful when your
//! engine uses `mint` as a math interop layer.
//!
//! # Example: simple inspector-style UI
//!
//! The following example shows how you might use `dear-imgui-reflect` to
//! build a small "inspector" for a list of game entities. Each entity is a
//! struct with nested fields, and the inspector lets you select an entity
//! from a list and edit its properties in place:
//!
//! ```no_run
//! use dear_imgui_reflect as reflect;
//! use reflect::ImGuiReflectExt;
//!
//! #[derive(reflect::ImGuiReflect, Default)]
//! struct Transform {
//!     #[imgui(tuple_render = "grid", tuple_columns = 3)]
//!     position: (f32, f32, f32),
//!     #[imgui(tuple_render = "grid", tuple_columns = 3)]
//!     rotation_euler: (f32, f32, f32),
//!     #[imgui(slider, min = 0.1, max = 10.0)]
//!     uniform_scale: f32,
//! }
//!
//! #[derive(reflect::ImGuiReflect, Default)]
//! struct Enemy {
//!     #[imgui(name = "Name")]
//!     name: String,
//!     #[imgui(slider, min = 0, max = 100)]
//!     health: i32,
//!     #[imgui(slider, min = 0.0, max = 1.0)]
//!     aggression: f32,
//!     #[imgui(name = "Waypoints")]
//!     patrol_points: Vec<(f32, f32)>,
//!     transform: Transform,
//! }
//!
//! #[derive(Default)]
//! struct EnemyInspector {
//!     enemies: Vec<Enemy>,
//!     selected: usize,
//! }
//!
//! impl EnemyInspector {
//!     fn ui(&mut self, ui: &reflect::imgui::Ui) {
//!         ui.window("Enemies").build(|| {
//!             // Left side: list of enemies with selection.
//!             ui.child_window("EnemyList")
//!                 .size([200.0, 0.0])
//!                 .build(ui, || {
//!                     for (i, enemy) in self.enemies.iter().enumerate() {
//!                         let label = format!("{i}: {}", enemy.name);
//!                         if ui.selectable_config(&label)
//!                             .selected(self.selected == i)
//!                             .build()
//!                         {
//!                             self.selected = i;
//!                         }
//!                     }
//!                 });
//!
//!             ui.same_line();
//!
//!             // Right side: reflected editor for the selected enemy.
//!             ui.child_window("EnemyInspector")
//!                 .size([0.0, 0.0])
//!                 .build(ui, || {
//!                     if let Some(enemy) = self.enemies.get_mut(self.selected) {
//!                         ui.input_reflect("Enemy", enemy);
//!                     } else {
//!                         ui.text("No enemy selected");
//!                     }
//!                 });
//!         });
//!     }
//! }
//! ```
//!
//! This pattern generalizes well to component-based engines, material
//! editors, and other data-driven UIs: derive [`ImGuiReflect`] for the
//! types you care about, then compose editors using `input_reflect` inside
//! whatever layout best fits your application.

#![deny(rust_2018_idioms)]
#![deny(missing_docs)]
#![allow(clippy::needless_lifetimes)]

use std::rc::Rc;
use std::sync::Arc;

/// Re-export the dear-imgui-rs crate for convenience.
///
/// Users can write `use dear_imgui_reflect::imgui::*;` instead of depending
/// on `dear-imgui-rs` directly if they only need basic types.
pub use dear_imgui_rs as imgui;

mod containers;
mod response;
mod settings;
mod values;

pub use containers::{
    imgui_array_with_settings, imgui_btree_map_with_settings, imgui_hash_map_with_settings,
    imgui_vec_with_settings,
};
pub use response::{ReflectEvent, ReflectResponse, with_field_path};
pub use settings::{
    ArraySettings, BoolSettings, BoolStyle, MapSettings, MemberSettings, NumericDefaultRange,
    NumericRange, NumericTypeSettings, NumericWidgetKind, ReflectSettings, TupleRenderMode,
    TupleSettings, VecSettings, current_settings, with_settings, with_settings_scope,
};
pub use values::imgui_tuple_body;

/// Trait for values that can render themselves as a single ImGui input widget.
///
/// This is implemented for common primitive types and can be implemented
/// manually for your own types. Most users will interact with
/// [`ImGuiReflect`](trait.ImGuiReflect.html) instead of this trait directly.
pub trait ImGuiValue {
    /// Draw a widget for this value.
    ///
    /// Returns `true` if the value was modified.
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool;
}

/// Trait for complex types (structs/enums) that can generate ImGui controls
/// for all of their fields.
///
/// You can derive this trait with `#[derive(ImGuiReflect)]` from this crate.
pub trait ImGuiReflect {
    /// Draw an ImGui editor for this value with the given label.
    ///
    /// Returns `true` if any field was modified.
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool;
}

/// Blanket implementation: any type that implements [`ImGuiReflect`] can also
/// be used wherever an [`ImGuiValue`] is expected.
impl<T: ImGuiReflect> ImGuiValue for T {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        value.imgui_reflect(ui, label)
    }
}

/// Transparent reflection for boxed values.
///
/// This allows `Box<T>` where `T: ImGuiReflect` to be edited like `T` itself,
/// matching ImReflect's behavior for smart pointers that simply forward to
/// the pointed-to value when engaged.
impl<T: ImGuiReflect> ImGuiReflect for Box<T> {
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool {
        self.as_mut().imgui_reflect(ui, label)
    }
}

/// Transparent reflection for reference-counted values (`Rc<T>`).
///
/// When there is exactly one strong reference, this forwards editing to the
/// inner `T`. Otherwise, it renders a read-only marker indicating that the
/// value is shared and cannot be safely mutated.
impl<T: ImGuiReflect> ImGuiReflect for Rc<T> {
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool {
        if let Some(inner) = Rc::get_mut(self) {
            inner.imgui_reflect(ui, label)
        } else {
            ui.text(label);
            ui.same_line();
            ui.text("<Rc shared (read-only)>");
            false
        }
    }
}

/// Transparent reflection for atomically reference-counted values (`Arc<T>`).
///
/// When there is exactly one strong reference, this forwards editing to the
/// inner `T`. Otherwise, it renders a read-only marker indicating that the
/// value is shared and cannot be safely mutated.
impl<T: ImGuiReflect> ImGuiReflect for Arc<T> {
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool {
        if let Some(inner) = Arc::get_mut(self) {
            inner.imgui_reflect(ui, label)
        } else {
            ui.text(label);
            ui.same_line();
            ui.text("<Arc shared (read-only)>");
            false
        }
    }
}

/// Render ImGui controls for a value that implements [`ImGuiReflect`].
///
/// This is the main entry point mirroring the C++ `ImReflect::Input` API.
pub fn input<T: ImGuiReflect>(ui: &imgui::Ui, label: &str, value: &mut T) -> bool {
    value.imgui_reflect(ui, label)
}

/// Variant of [`input`] that additionally collects container-level change events
/// into the provided [`ReflectResponse`].
///
/// The returned boolean is identical to [`input`]: `true` if any field was
/// modified. Container editors emit structural change events into `response`
/// while this function is executing. User-defined `ImGuiValue`/`ImGuiReflect`
/// implementations that only rely on the existing APIs will continue to work,
/// but will not automatically populate `response` unless they call into
/// `dear-imgui-reflect`'s container helpers.
pub fn input_with_response<T: ImGuiReflect>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut T,
    response: &mut ReflectResponse,
) -> bool {
    response::with_response(response, || input(ui, label, value))
}

/// Extension methods on `Ui` for reflection-based widgets.
pub trait ImGuiReflectExt {
    /// Render a reflected editor for a value.
    ///
    /// Returns `true` if any field changed.
    fn input_reflect<T: ImGuiReflect>(&self, label: &str, value: &mut T) -> bool;
}

impl ImGuiReflectExt for imgui::Ui {
    fn input_reflect<T: ImGuiReflect>(&self, label: &str, value: &mut T) -> bool {
        input(self, label, value)
    }
}

/// Derive macro for [`ImGuiReflect`], re-exported for convenience.
///
/// The macro understands a set of `#[imgui(...)]` field attributes that
/// configure per-field behavior (labels, sliders vs drags, multiline text,
/// tuple layout, etc). See the macro documentation on docs.rs for the full
/// list and examples.
#[cfg(feature = "derive")]
pub use dear_imgui_reflect_derive::ImGuiReflect;
