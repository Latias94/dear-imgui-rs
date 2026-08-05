#[cfg(any(feature = "binding-spec", test))]
pub mod binding;
mod native_deps;
mod patches;
mod prebuilt;
#[cfg(any(feature = "binding-spec", test))]
pub mod source_inventory;

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub use native_deps::*;
pub use patches::*;
pub use prebuilt::*;
