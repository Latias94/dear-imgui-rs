use crate::fs::FileSystem;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

/// State-owned filesystem capability.
///
/// A blocking filesystem never crosses a thread boundary. Native background
/// scans require the stronger `Arc + Send + Sync` capability explicitly.
pub(crate) enum FileSystemCapability {
    Blocking(Box<dyn FileSystem>),
    #[cfg(not(target_arch = "wasm32"))]
    Background(Arc<dyn FileSystem + Send + Sync>),
}

impl std::fmt::Debug for FileSystemCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocking(_) => f.write_str("FileSystemCapability::Blocking(..)"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Background(_) => f.write_str("FileSystemCapability::Background(..)"),
        }
    }
}

impl FileSystemCapability {
    pub(crate) fn blocking(filesystem: Box<dyn FileSystem>) -> Self {
        Self::Blocking(filesystem)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn background(filesystem: Arc<dyn FileSystem + Send + Sync>) -> Self {
        Self::Background(filesystem)
    }

    pub(crate) fn as_file_system(&self) -> &dyn FileSystem {
        match self {
            Self::Blocking(filesystem) => filesystem.as_ref(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Background(filesystem) => filesystem.as_ref(),
        }
    }

    pub(crate) fn supports_background(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            matches!(self, Self::Background(_))
        }

        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn background_file_system(&self) -> Option<Arc<dyn FileSystem + Send + Sync>> {
        match self {
            Self::Background(filesystem) => Some(Arc::clone(filesystem)),
            Self::Blocking(_) => None,
        }
    }
}
