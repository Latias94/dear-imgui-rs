//! Reflection-based helpers for dear-imgui-rs.
//!
//! This crate provides traits and helpers to automatically generate Dear ImGui
//! widgets for your Rust types, similar to the C++ ImReflect library.

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

// Re-export the derive macro when the "derive" feature is enabled so users can
// simply depend on `dear-imgui-reflect` and write `#[derive(ImGuiReflect)]`.
#[cfg(feature = "derive")]
pub use dear_imgui_reflect_derive::ImGuiReflect;
