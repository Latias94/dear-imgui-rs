//! Shared container helpers for dear-imgui-reflect.
//!
//! This module centralizes the editing logic for arrays, vectors and
//! string-keyed maps, including the temporary state needed for map insertion
//! popups and the emission of [`ReflectEvent`](crate::ReflectEvent) values.

mod array;
mod map;
mod path_state;
#[cfg(test)]
mod tests;
mod vec;

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::response;
use crate::{ImGuiValue, VecSettings, imgui};

pub use self::array::imgui_array_with_settings;
pub use self::map::{imgui_btree_map_with_settings, imgui_hash_map_with_settings};
pub use self::vec::imgui_vec_with_settings;
