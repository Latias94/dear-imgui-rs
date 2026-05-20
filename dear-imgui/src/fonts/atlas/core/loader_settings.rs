use crate::fonts::atlas::loader::{FontLoader, FontLoaderFlags};
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Sets the font loader for this atlas.
    ///
    /// This allows using custom font backends like FreeType with additional features.
    /// Must be called before adding any fonts.
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*`.
    pub fn set_font_loader(&mut self, loader: &'static FontLoader) {
        unsafe {
            sys::ImFontAtlas_SetFontLoader(self.raw, loader.as_ptr());
        }
    }

    // Note: switching to the FreeType loader at runtime requires access to the
    // C++ symbol ImGuiFreeType_GetFontLoader(), which may not be available in
    // prebuilt dear-imgui-sys distributions. If needed, prefer configuring the
    // loader from the sys layer or ensure the symbol is exported, then add a
    // thin wrapper here.

    /// Sets global font loader flags
    ///
    /// These flags apply to all fonts loaded with this atlas unless overridden
    /// in individual FontConfig instances.
    pub fn set_font_loader_flags(&mut self, flags: FontLoaderFlags) {
        unsafe {
            (*self.raw).FontLoaderFlags = flags.0;
        }
    }

    /// Gets the current font loader flags
    pub fn font_loader_flags(&self) -> FontLoaderFlags {
        unsafe { FontLoaderFlags((*self.raw).FontLoaderFlags) }
    }
}
