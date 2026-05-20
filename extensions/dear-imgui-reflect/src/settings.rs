//! Global and per-member settings for dear-imgui-reflect.
//!
//! This module defines [`ReflectSettings`] and [`MemberSettings`], along with
//! container and numeric configuration types that mirror many concepts from
//! ImReflect's `ImSettings` API.

mod bool;
mod container;
mod global;
mod member;
mod numeric;
#[cfg(test)]
mod tests;
mod tuple;

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub use self::bool::{BoolSettings, BoolStyle};
pub use self::container::{ArraySettings, MapSettings, VecSettings};
pub(crate) use self::global::with_settings_read;
pub use self::global::{ReflectSettings, current_settings, with_settings, with_settings_scope};
pub use self::member::MemberSettings;
pub use self::numeric::{
    NumericDefaultRange, NumericRange, NumericTypeSettings, NumericWidgetKind,
};
pub use self::tuple::{TupleRenderMode, TupleSettings};
