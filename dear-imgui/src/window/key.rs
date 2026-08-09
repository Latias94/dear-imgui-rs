use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use thiserror::Error;

use crate::{Id, sys};

const ID_SEPARATOR: &str = "###";

#[derive(Debug)]
struct WindowKeyIdentity {
    stable_id: Box<str>,
    default_title: Box<str>,
    docking_name: CString,
    native_id: Id,
}

/// Stable Dear ImGui identity for a top-level window.
///
/// A key stores a default displayed title separately from the identity used by docking and INI
/// persistence. Use the same key in [`DockLayout`](crate::DockLayout) and [`Ui::window`](crate::Ui::window)
/// so a displayed-title change cannot silently create a different native window.
///
/// ```no_run
/// # use dear_imgui_rs::*;
/// # fn draw(ui: &Ui) -> Result<(), WindowKeyError> {
/// let scene = WindowKey::new("scene", "Scene")?;
/// let layout = DockLayout::tabs([&scene]);
///
/// ui.window(scene.label("Scene (Debug)"))
///     .build(|| ui.text("The stable identity is still `scene`."));
/// # let _ = layout;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct WindowKey {
    identity: Arc<WindowKeyIdentity>,
}

impl WindowKey {
    /// Create a validated stable identity and its default displayed title.
    pub fn new(
        stable_id: impl Into<String>,
        default_title: impl Into<String>,
    ) -> Result<Self, WindowKeyError> {
        let stable_id = stable_id.into();
        if stable_id.is_empty() {
            return Err(WindowKeyError::EmptyStableId);
        }
        if stable_id.as_bytes().contains(&0) {
            return Err(WindowKeyError::StableIdContainsNul);
        }
        if stable_id.contains(ID_SEPARATOR) {
            return Err(WindowKeyError::StableIdContainsSeparator);
        }

        let mut docking_name = String::with_capacity(ID_SEPARATOR.len() + stable_id.len());
        docking_name.push_str(ID_SEPARATOR);
        docking_name.push_str(&stable_id);
        let docking_name = CString::new(docking_name)
            .expect("a validated window key must produce a valid native name");
        // SAFETY: `docking_name` is readable and NUL-terminated. ImHashStr is context-free.
        let native_id = Id::from(unsafe { sys::igImHashStr(docking_name.as_ptr(), 0, 0) });
        if native_id.raw() == 0 {
            return Err(WindowKeyError::NativeIdIsZero);
        }

        Ok(Self {
            identity: Arc::new(WindowKeyIdentity {
                stable_id: stable_id.into_boxed_str(),
                default_title: default_title.into().into_boxed_str(),
                docking_name,
                native_id,
            }),
        })
    }

    /// Return the stable identity string.
    pub fn stable_id(&self) -> &str {
        &self.identity.stable_id
    }

    /// Return the default displayed title.
    pub fn default_title(&self) -> &str {
        &self.identity.default_title
    }

    /// Use a different displayed title without changing the stable identity.
    pub fn label<'a>(&'a self, title: impl Into<Cow<'a, str>>) -> WindowLabel<'a> {
        WindowLabel::Keyed {
            key: self,
            title: title.into(),
        }
    }

    pub(crate) fn docking_name(&self) -> &CStr {
        &self.identity.docking_name
    }

    pub(crate) fn native_id(&self) -> Id {
        self.identity.native_id
    }
}

impl PartialEq for WindowKey {
    fn eq(&self, other: &Self) -> bool {
        self.stable_id() == other.stable_id()
    }
}

impl Eq for WindowKey {}

impl Hash for WindowKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.stable_id().hash(state);
    }
}

impl fmt::Debug for WindowKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowKey")
            .field("stable_id", &self.stable_id())
            .field("default_title", &self.default_title())
            .field("native_id", &self.native_id())
            .finish()
    }
}

impl From<&WindowKey> for WindowKey {
    fn from(key: &WindowKey) -> Self {
        key.clone()
    }
}

/// Window label accepted by [`Ui::window`](crate::Ui::window).
///
/// Plain strings retain Dear ImGui's native `##`/`###` behavior. Labels created by
/// [`WindowKey::label`] always append the validated stable identity after the displayed title.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum WindowLabel<'a> {
    Plain(Cow<'a, str>),
    Keyed {
        key: &'a WindowKey,
        title: Cow<'a, str>,
    },
}

impl WindowLabel<'_> {
    /// Return the displayed title supplied to Dear ImGui.
    pub fn title(&self) -> &str {
        match self {
            Self::Plain(title) | Self::Keyed { title, .. } => title,
        }
    }

    /// Return the stable key, when this label is keyed.
    pub fn key(&self) -> Option<&WindowKey> {
        match self {
            Self::Plain(_) => None,
            Self::Keyed { key, .. } => Some(key),
        }
    }
}

impl<'a> From<&'a str> for WindowLabel<'a> {
    fn from(title: &'a str) -> Self {
        Self::Plain(Cow::Borrowed(title))
    }
}

impl<'a> From<&'a String> for WindowLabel<'a> {
    fn from(title: &'a String) -> Self {
        Self::Plain(Cow::Borrowed(title))
    }
}

impl From<String> for WindowLabel<'_> {
    fn from(title: String) -> Self {
        Self::Plain(Cow::Owned(title))
    }
}

impl<'a> From<Cow<'a, str>> for WindowLabel<'a> {
    fn from(title: Cow<'a, str>) -> Self {
        Self::Plain(title)
    }
}

impl<'a> From<&'a WindowKey> for WindowLabel<'a> {
    fn from(key: &'a WindowKey) -> Self {
        Self::Keyed {
            key,
            title: Cow::Borrowed(key.default_title()),
        }
    }
}

/// Validation failure while creating a [`WindowKey`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowKeyError {
    #[error("a stable window ID cannot be empty")]
    EmptyStableId,
    #[error("a stable window ID cannot contain an interior NUL byte")]
    StableIdContainsNul,
    #[error("a stable window ID cannot contain Dear ImGui's `###` identity separator")]
    StableIdContainsSeparator,
    #[error("the stable window ID hashes to Dear ImGui's reserved zero ID")]
    NativeIdIsZero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_or_invalid_stable_ids() {
        assert_eq!(
            WindowKey::new("", "Title"),
            Err(WindowKeyError::EmptyStableId)
        );
        assert_eq!(
            WindowKey::new("bad\0id", "Title"),
            Err(WindowKeyError::StableIdContainsNul)
        );
        assert_eq!(
            WindowKey::new("first###second", "Title"),
            Err(WindowKeyError::StableIdContainsSeparator)
        );
    }

    #[test]
    fn title_changes_preserve_equality_and_native_identity() {
        let scene = WindowKey::new("scene", "Scene").unwrap();
        let renamed = WindowKey::new("scene", "Scene (Debug)").unwrap();
        assert_eq!(scene, renamed);
        assert_eq!(scene.native_id(), renamed.native_id());
        assert_eq!(scene.docking_name().to_bytes(), b"###scene");
    }
}
