//! Shared container helpers for dear-imgui-reflect.
//!
//! This module centralizes the editing logic for arrays, vectors and
//! string-keyed maps, including the temporary state needed for map insertion
//! popups and the emission of [`ReflectEvent`](crate::ReflectEvent) values.

mod array;
mod map;
#[cfg(test)]
mod tests;
mod vec;

use std::collections::HashMap;

use crate::{ImGuiValue, Inspector, VecSettings, imgui};

pub use self::array::imgui_array_with_settings;
pub use self::map::{imgui_btree_map_with_settings, imgui_hash_map_with_settings};
pub use self::vec::imgui_vec_with_settings;

pub(super) fn escape_field_path_key(key: &str) -> String {
    key.replace('\\', "\\\\").replace('"', "\\\"")
}
