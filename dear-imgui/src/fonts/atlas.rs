//! Font atlas management for Dear ImGui v1.92+
//!
//! This module provides a modern, type-safe interface to Dear ImGui's dynamic font system.
//! Key features:
//! - Dynamic glyph loading (no need to pre-specify glyph ranges)
//! - Runtime font size adjustment
//! - Custom font loaders
//! - Incremental texture updates

use crate::fonts::Font;
use crate::sys;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_char};
use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;

const RASTERIZER_MULTIPLY_MAX: f32 = 16_000_000.0;

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

fn assert_non_negative_i8(caller: &str, name: &str, value: i8) {
    assert!(value >= 0, "{caller} {name} must be non-negative");
}

fn validate_font_size_pixels(caller: &str, name: &str, size_pixels: f32) -> f32 {
    assert_non_negative_f32(caller, name, size_pixels);
    size_pixels
}

fn validate_font_size_pixels_option(caller: &str, name: &str, size_pixels: Option<f32>) -> f32 {
    let size_pixels = size_pixels.unwrap_or(0.0);
    validate_font_size_pixels(caller, name, size_pixels)
}

fn assert_reference_font_size_for_metrics(
    caller: &str,
    size_pixels: f32,
    has_reference_size_dependent_metrics: bool,
) {
    assert!(
        !has_reference_size_dependent_metrics || size_pixels > 0.0,
        "{caller} glyph offset/advance overrides require a positive reference size"
    );
}

fn assert_font_source_for_add_font(caller: &str, raw: &sys::ImFontConfig) {
    let has_font_data = !raw.FontData.is_null() && raw.FontDataSize > 0;
    let has_font_loader = !raw.FontLoader.is_null();
    assert!(
        has_font_data || has_font_loader,
        "{caller} requires FontData/FontDataSize or FontLoader"
    );
    if has_font_loader {
        unsafe {
            assert!(
                (*raw.FontLoader).FontBakedLoadGlyph.is_some(),
                "{caller} FontLoader must provide FontBakedLoadGlyph"
            );
        }
    }
}

/// Font atlas that manages multiple fonts and their texture data
///
/// The font atlas is responsible for:
/// - Loading and managing multiple fonts
/// - Packing font glyphs into texture atlases
/// - Providing texture data for rendering
#[derive(Debug)]
pub struct FontAtlas {
    raw: *mut sys::ImFontAtlas,
    owned: bool,
    _phantom: PhantomData<*mut sys::ImFontAtlas>,
}

/// Shared view of a font atlas.
///
/// This type allows read-only atlas inspection without exposing safe font mutation from
/// `Context::font_atlas()`.
#[derive(Debug, Clone, Copy)]
pub struct FontAtlasRef<'atlas> {
    raw: *const sys::ImFontAtlas,
    _phantom: PhantomData<&'atlas sys::ImFontAtlas>,
}

/// A persistent, atlas-validated font handle.
///
/// `FontId` can be stored in application style state and later passed to
/// [`Ui::push_font`](crate::Ui::push_font), but it is not just a raw `ImFont*`.
/// The handle records the originating atlas and atlas generation. Safe push
/// APIs validate that the handle still belongs to the current context's atlas
/// and has not been invalidated by [`FontAtlas::clear`],
/// [`FontAtlas::clear_fonts`], or [`FontAtlas::remove_font`] before calling
/// Dear ImGui.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct FontId {
    pub(crate) raw: *mut sys::ImFont,
    atlas: *mut sys::ImFontAtlas,
    atlas_stamp: u64,
    generation: u64,
    _not_send_sync: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FontAtlasState {
    stamp: u64,
    generation: u64,
}

#[derive(Default)]
struct FontAtlasStates {
    next_stamp: u64,
    by_atlas: HashMap<usize, FontAtlasState>,
}

thread_local! {
    static FONT_ATLAS_STATES: RefCell<FontAtlasStates> = RefCell::new(FontAtlasStates {
        next_stamp: 1,
        by_atlas: HashMap::new(),
    });
}

fn font_atlas_state(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        if let Some(state) = states.by_atlas.get(&key).copied() {
            return state;
        }
        let stamp = states.next_stamp;
        states.next_stamp = states
            .next_stamp
            .checked_add(1)
            .expect("font atlas stamp counter overflowed");
        let state = FontAtlasState {
            stamp,
            generation: 0,
        };
        states.by_atlas.insert(key, state);
        state
    })
}

fn bump_font_atlas_generation(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        let mut state = states.by_atlas.get(&key).copied().unwrap_or_else(|| {
            let stamp = states.next_stamp;
            states.next_stamp = states
                .next_stamp
                .checked_add(1)
                .expect("font atlas stamp counter overflowed");
            FontAtlasState {
                stamp,
                generation: 0,
            }
        });
        state.generation = state
            .generation
            .checked_add(1)
            .expect("font atlas generation counter overflowed");
        states.by_atlas.insert(key, state);
        state
    })
}

pub(crate) fn forget_font_atlas_generation(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        states.borrow_mut().by_atlas.remove(&(raw as usize));
    });
}

fn font_atlas_contains_font(atlas: *mut sys::ImFontAtlas, font: *mut sys::ImFont) -> bool {
    if atlas.is_null() || font.is_null() {
        return false;
    }
    unsafe {
        let fonts = &(*atlas).Fonts;
        if fonts.Size <= 0 || fonts.Data.is_null() {
            return false;
        }
        for index in 0..fonts.Size {
            if *fonts.Data.add(index as usize) == font {
                return true;
            }
        }
    }
    false
}

fn current_context_font_atlas(caller: &str) -> *mut sys::ImFontAtlas {
    unsafe {
        let ctx = sys::igGetCurrentContext();
        assert!(!ctx.is_null(), "{caller} requires an active ImGui context");
        let io = sys::igGetIO_ContextPtr(ctx);
        assert!(!io.is_null(), "{caller} requires a valid ImGui IO");
        let atlas = (*io).Fonts;
        assert!(
            !atlas.is_null(),
            "{caller} requires the current ImGui context to have a font atlas"
        );
        atlas
    }
}

impl FontId {
    pub(crate) fn from_raw_parts(font: *mut sys::ImFont, atlas: *mut sys::ImFontAtlas) -> Self {
        assert!(!font.is_null(), "FontId requires a non-null ImFont pointer");
        assert!(
            !atlas.is_null(),
            "FontId requires a non-null ImFontAtlas pointer"
        );
        let state = font_atlas_state(atlas);
        Self {
            raw: font,
            atlas,
            atlas_stamp: state.stamp,
            generation: state.generation,
            _not_send_sync: PhantomData,
        }
    }

    pub(crate) unsafe fn from_font(font: *mut sys::ImFont, caller: &str) -> Self {
        assert!(!font.is_null(), "{caller} requires a non-null font");
        let atlas = unsafe { (*font).OwnerAtlas };
        assert!(
            !atlas.is_null(),
            "{caller} requires the font to have an owning atlas"
        );
        Self::from_raw_parts(font, atlas)
    }
}

pub(crate) fn validate_font_id_for_current_context(id: FontId, caller: &str) -> *mut sys::ImFont {
    let atlas = current_context_font_atlas(caller);
    validate_font_id_for_atlas(id, atlas, caller)
}

pub(crate) fn validate_font_for_current_context(font: &Font, caller: &str) -> *mut sys::ImFont {
    let atlas = current_context_font_atlas(caller);
    validate_font_for_atlas(font, atlas, caller)
}

pub(crate) fn validate_font_id_for_atlas(
    id: FontId,
    atlas: *mut sys::ImFontAtlas,
    caller: &str,
) -> *mut sys::ImFont {
    assert!(!id.raw.is_null(), "{caller} received a null FontId");
    assert!(
        std::ptr::addr_eq(id.atlas.cast_const(), atlas.cast_const()),
        "{caller} received a FontId from a different font atlas"
    );
    let state = font_atlas_state(atlas);
    assert!(
        state.stamp == id.atlas_stamp,
        "{caller} received a FontId from a destroyed or reused font atlas"
    );
    assert!(
        state.generation == id.generation,
        "{caller} received a stale FontId invalidated by font atlas mutation"
    );
    assert!(
        font_atlas_contains_font(atlas, id.raw),
        "{caller} received a FontId that is not present in the current font atlas"
    );
    id.raw
}

pub(crate) fn validate_font_for_atlas(
    font: &Font,
    atlas: *mut sys::ImFontAtlas,
    caller: &str,
) -> *mut sys::ImFont {
    let raw = font.raw();
    assert!(!raw.is_null(), "{caller} received a null font");
    unsafe {
        let owner = (*raw).OwnerAtlas;
        assert!(
            std::ptr::addr_eq(owner.cast_const(), atlas.cast_const()),
            "{caller} received a font from a different font atlas"
        );
    }
    assert!(
        font_atlas_contains_font(atlas, raw),
        "{caller} received a font that is not present in the current font atlas"
    );
    raw
}

/// Font loader interface for custom font backends
///
/// This provides a safe Rust interface to Dear ImGui's ImFontLoader system,
/// allowing custom font loading implementations.
pub struct FontLoader {
    raw: sys::ImFontLoader,
    _name: CString,
}

impl FontLoader {
    /// Creates a new font loader with the given name
    pub fn new(name: &str) -> Result<Self, std::ffi::NulError> {
        let name_cstring = CString::new(name)?;
        // Initialize via ImGui constructor to future-proof defaults
        let mut raw = unsafe {
            let p = sys::ImFontLoader_ImFontLoader();
            if p.is_null() {
                panic!("ImFontLoader_ImFontLoader() returned null");
            }
            let v = *p;
            sys::ImFontLoader_destroy(p);
            v
        };
        raw.Name = name_cstring.as_ptr();

        Ok(Self {
            raw,
            _name: name_cstring,
        })
    }

    /// Returns a pointer to the raw ImFontLoader
    pub(crate) fn as_ptr(&self) -> *const sys::ImFontLoader {
        &self.raw
    }

    /// Sets the loader initialization callback
    pub fn with_loader_init<F>(self, _callback: F) -> Self
    where
        F: Fn(&mut FontAtlas) -> bool + 'static,
    {
        // Note: For now, we'll use the default STB TrueType loader
        // Custom callbacks would require more complex lifetime management
        self
    }
}

/// Font loader flags for controlling font loading behavior.
///
/// These bits mirror Dear ImGui's `ImGuiFreeTypeLoaderFlags` (see
/// `misc/freetype/imgui_freetype.h`) and are only interpreted by the
/// FreeType font backend. When using the stb_truetype backend, they
/// are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLoaderFlags(pub u32);

impl FontLoaderFlags {
    /// No special flags
    pub const NONE: Self = Self(0);

    /// Disable hinting (more faithful to the original glyph shapes, but blurrier)
    pub const NO_HINTING: Self = Self(1 << 0);

    /// Disable auto-hinter (prefer the font's native hinter only)
    pub const NO_AUTOHINT: Self = Self(1 << 1);

    /// Prefer auto-hinter over the font's native hinter
    pub const FORCE_AUTOHINT: Self = Self(1 << 2);

    /// Light hinting (often closer to Windows ClearType appearance)
    pub const LIGHT_HINTING: Self = Self(1 << 3);

    /// Strong/mono hinting (intended for monochrome outputs)
    pub const MONO_HINTING: Self = Self(1 << 4);

    /// Artificially embolden the font
    pub const BOLD: Self = Self(1 << 5);

    /// Artificially slant the font (oblique)
    pub const OBLIQUE: Self = Self(1 << 6);

    /// Disable anti-aliasing (combine with `MONO_HINTING` for best results)
    pub const MONOCHROME: Self = Self(1 << 7);

    /// Enable color-layered glyphs (e.g. color emoji)
    pub const LOAD_COLOR: Self = Self(1 << 8);

    /// Enable FreeType bitmap glyphs
    pub const BITMAP: Self = Self(1 << 9);
}

impl std::ops::BitOr for FontLoaderFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for FontLoaderFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A shared font atlas that can be used across multiple contexts
///
/// This allows multiple ImGui contexts to share the same font atlas,
/// which is useful for applications with multiple windows or contexts.
#[derive(Debug, Clone)]
pub struct SharedFontAtlas(pub(crate) Rc<*mut sys::ImFontAtlas>);

impl SharedFontAtlas {
    /// Creates a new shared font atlas
    pub fn create() -> SharedFontAtlas {
        unsafe {
            let raw_atlas = sys::ImFontAtlas_ImFontAtlas();
            if raw_atlas.is_null() {
                panic!("ImFontAtlas_ImFontAtlas() returned null");
            }
            font_atlas_state(raw_atlas);
            SharedFontAtlas(Rc::new(raw_atlas))
        }
    }

    /// Returns a mutable raw pointer to the underlying ImFontAtlas
    pub(crate) fn as_ptr_mut(&mut self) -> *mut sys::ImFontAtlas {
        *self.0
    }
}

impl Drop for SharedFontAtlas {
    fn drop(&mut self) {
        // Only drop if this is the last reference
        if Rc::strong_count(&self.0) == 1 {
            unsafe {
                let atlas_ptr = *self.0;
                if !atlas_ptr.is_null() {
                    forget_font_atlas_generation(atlas_ptr);
                    sys::ImFontAtlas_destroy(atlas_ptr);
                }
            }
        }
    }
}

impl<'atlas> FontAtlasRef<'atlas> {
    pub(crate) unsafe fn from_raw(raw: *const sys::ImFontAtlas) -> Self {
        assert!(
            !raw.is_null(),
            "FontAtlasRef::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw.cast_mut());
        Self {
            raw,
            _phantom: PhantomData,
        }
    }

    /// Returns the raw ImFontAtlas pointer.
    pub fn raw(&self) -> *const sys::ImFontAtlas {
        self.raw
    }

    /// Gets the current font loader flags.
    pub fn font_loader_flags(&self) -> FontLoaderFlags {
        unsafe { FontLoaderFlags((*self.raw).FontLoaderFlags) }
    }

    /// Check if the texture is built.
    pub fn is_built(&self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        unsafe { (*self.raw).TexIsBuilt }
    }

    /// Get texture data information.
    pub fn get_tex_data_info(&self) -> Option<(u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let min_width = (*self.raw).TexMinWidth as u32;
                let min_height = (*self.raw).TexMinHeight as u32;
                Some((min_width, min_height))
            } else {
                None
            }
        }
    }

    /// Get raw texture data pointer and dimensions.
    ///
    /// # Safety
    /// The returned pointer is only valid while the FontAtlas exists and the texture is built.
    /// The caller must ensure proper lifetime management.
    pub unsafe fn get_tex_data_ptr(&self) -> Option<(*const u8, u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let tex_data = (*self.raw).TexData;
                if !tex_data.is_null() {
                    let width = (*tex_data).Width as u32;
                    let height = (*tex_data).Height as u32;
                    let pixels = (*tex_data).Pixels;
                    if !pixels.is_null() {
                        Some((pixels, width, height))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    /// Get texture reference for the font atlas.
    pub fn get_tex_ref(&self) -> sys::ImTextureRef {
        unsafe { (*self.raw).TexRef }
    }

    /// Get texture data pointer.
    pub fn get_tex_data(&self) -> *mut sys::ImTextureData {
        unsafe { (*self.raw).TexData }
    }

    /// Get a shared view of the atlas texture data, if available.
    pub fn tex_data(&self) -> Option<&crate::texture::TextureData> {
        let ptr = unsafe { (*self.raw).TexData };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { crate::texture::TextureData::from_raw_ref(ptr) })
        }
    }

    /// Get texture UV scale.
    pub fn get_tex_uv_scale(&self) -> [f32; 2] {
        unsafe {
            let scale = (*self.raw).TexUvScale;
            [scale.x, scale.y]
        }
    }

    /// Get texture UV white pixel coordinates.
    pub fn get_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            let pixel = (*self.raw).TexUvWhitePixel;
            [pixel.x, pixel.y]
        }
    }
}

impl FontAtlas {
    /// Creates a new font atlas with default settings
    pub fn new() -> Self {
        unsafe {
            let raw = sys::ImFontAtlas_ImFontAtlas();
            if raw.is_null() {
                panic!("ImFontAtlas_ImFontAtlas() returned null");
            }
            font_atlas_state(raw);
            Self {
                raw,
                owned: true,
                _phantom: PhantomData,
            }
        }
    }

    /// Creates a new font atlas with a custom font loader.
    ///
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*`.
    pub fn with_font_loader(loader: &'static FontLoader) -> Self {
        let mut atlas = Self::new();
        atlas.set_font_loader(loader);
        atlas
    }

    /// Creates a FontAtlas wrapper from a raw ImFontAtlas pointer
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and points to a valid ImFontAtlas
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImFontAtlas) -> Self {
        assert!(
            !raw.is_null(),
            "FontAtlas::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw);
        Self {
            raw,
            owned: false,
            _phantom: PhantomData,
        }
    }

    /// Returns the raw ImFontAtlas pointer
    pub fn raw(&self) -> *mut sys::ImFontAtlas {
        self.raw
    }

    pub(crate) fn font_id_for_raw(&self, font: *mut sys::ImFont) -> FontId {
        FontId::from_raw_parts(font, self.raw)
    }

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

    /// Add a font to the atlas using FontSource
    #[doc(alias = "AddFont")]
    pub fn add_font(&mut self, font_sources: &[FontSource<'_>]) -> crate::fonts::FontId {
        let Some((head, tail)) = font_sources.split_first() else {
            panic!("FontAtlas::add_font requires at least one FontSource");
        };
        let font_id = self.add_font_internal(head, false);
        for font in tail {
            self.add_font_internal(font, true);
        }
        font_id
    }

    fn add_font_internal(
        &mut self,
        font_source: &FontSource<'_>,
        merge_mode: bool,
    ) -> crate::fonts::FontId {
        match font_source {
            FontSource::DefaultFontData {
                size_pixels,
                config,
            } => {
                // For v1.92+, we can use dynamic sizing by passing 0.0
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self.add_font_default(Some(&cfg)).raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::TtfData {
                data,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_memory_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add TTF font from memory")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::CompressedTtfData {
                data,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_memory_compressed_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add compressed TTF font from memory")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::CompressedTtfBase85 {
                data,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_memory_compressed_base85_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add base85 compressed TTF font from memory")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::TtfFile {
                path,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_file_ttf(path, size, Some(&cfg), None)
                    .expect("Failed to add TTF font from file")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
        }
    }

    /// Add a font to the atlas using FontConfig
    #[doc(alias = "AddFont")]
    pub fn add_font_with_config(&mut self, font_cfg: &FontConfig) -> &mut Font {
        font_cfg.validate_for_add_font("FontAtlas::add_font_with_config()");
        unsafe {
            let font_ptr = sys::ImFontAtlas_AddFont(self.raw, font_cfg.raw());
            if font_cfg.raw.MergeMode {
                self.discard_bakes(0);
            }
            Font::from_raw_mut(font_ptr)
        }
    }

    /// Add the default font to the atlas
    #[doc(alias = "AddFontDefault")]
    pub fn add_font_default(&mut self, font_cfg: Option<&FontConfig>) -> &mut Font {
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_default("FontAtlas::add_font_default()");
        }
        unsafe {
            let cfg_ptr = font_cfg.map_or(ptr::null(), |cfg| cfg.raw());
            let font_ptr = sys::ImFontAtlas_AddFontDefault(self.raw, cfg_ptr);
            if let Some(cfg) = font_cfg {
                if cfg.raw.MergeMode {
                    self.discard_bakes(0);
                }
            }
            Font::from_raw_mut(font_ptr)
        }
    }

    /// Add a font from a TTF file
    #[doc(alias = "AddFontFromFileTTF")]
    pub fn add_font_from_file_ttf(
        &mut self,
        filename: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_file_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size("FontAtlas::add_font_from_file_ttf()", size_pixels);
        }
        unsafe {
            let filename_cstr = std::ffi::CString::new(filename).ok()?;
            let cfg_ptr = font_cfg.map_or(ptr::null(), |cfg| cfg.raw());
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromFileTTF(
                self.raw,
                filename_cstr.as_ptr(),
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                if let Some(cfg) = font_cfg {
                    if cfg.raw.MergeMode {
                        self.discard_bakes(0);
                    }
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Add a font from memory (TTF data)
    #[doc(alias = "AddFontFromMemoryTTF")]
    pub fn add_font_from_memory_ttf(
        &mut self,
        font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_memory_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size(
                "FontAtlas::add_font_from_memory_ttf()",
                size_pixels,
            );
        }
        // Dear ImGui asserts on suspiciously small buffers to catch common mistakes.
        // Mirror that behavior by returning `None` instead of panicking/aborting in debug builds.
        if font_data.len() <= 100 {
            return None;
        }
        let font_data_len = i32::try_from(font_data.len()).ok()?;
        unsafe {
            // SAFETY: `AddFontFromMemoryTTF()` stores the pointer for (potential) rebuilds and may
            // free it later depending on `FontDataOwnedByAtlas`. Never pass a pointer into
            // Rust-owned stack/Vec memory here.
            //
            // Allocate and copy the bytes using Dear ImGui's allocator, then let the atlas own it.
            // This avoids use-after-free, double-free, and leaking uninitialized padding bytes
            // across the C++ boundary.
            let mem = sys::igMemAlloc(font_data.len());
            if mem.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(font_data.as_ptr(), mem as *mut u8, font_data.len());

            let cfg = font_cfg
                .cloned()
                .unwrap_or_default()
                .font_data_owned_by_atlas(true);
            let is_merge = cfg.raw.MergeMode;
            let cfg_ptr = cfg.raw();
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw,
                mem,
                font_data_len,
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                sys::igMemFree(mem);
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Add a font from memory (compressed TTF data).
    ///
    /// Dear ImGui will decompress the data immediately and keep the decompressed buffer alive
    /// (owned by the atlas), so the `compressed_font_data` slice does not need to outlive this call.
    #[doc(alias = "AddFontFromMemoryCompressedTTF")]
    pub fn add_font_from_memory_compressed_ttf(
        &mut self,
        compressed_font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_memory_compressed_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size(
                "FontAtlas::add_font_from_memory_compressed_ttf()",
                size_pixels,
            );
        }
        if compressed_font_data.is_empty() {
            return None;
        }
        let compressed_len = i32::try_from(compressed_font_data.len()).ok()?;

        unsafe {
            let cfg = font_cfg.cloned().unwrap_or_default();
            let is_merge = cfg.raw.MergeMode;
            let cfg_ptr = cfg.raw();
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedTTF(
                self.raw,
                compressed_font_data.as_ptr() as *const std::os::raw::c_void,
                compressed_len,
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Add a font from memory (compressed + base85-encoded TTF data).
    ///
    /// The input string must be NUL-terminated for Dear ImGui; this wrapper allocates a `CString`
    /// and passes it to the backend.
    #[doc(alias = "AddFontFromMemoryCompressedBase85TTF")]
    pub fn add_font_from_memory_compressed_base85_ttf(
        &mut self,
        compressed_font_data_base85: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_memory_compressed_base85_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size(
                "FontAtlas::add_font_from_memory_compressed_base85_ttf()",
                size_pixels,
            );
        }
        if compressed_font_data_base85.is_empty() {
            return None;
        }
        let base85 = std::ffi::CString::new(compressed_font_data_base85).ok()?;

        unsafe {
            let cfg = font_cfg.cloned().unwrap_or_default();
            let is_merge = cfg.raw.MergeMode;
            let cfg_ptr = cfg.raw();
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedBase85TTF(
                self.raw,
                base85.as_ptr(),
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Remove a font from the atlas.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "RemoveFont")]
    pub fn remove_font(&mut self, font: &mut Font) {
        let font = validate_font_for_atlas(font, self.raw, "FontAtlas::remove_font()");
        unsafe { sys::ImFontAtlas_RemoveFont(self.raw, font) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear all fonts and texture data.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "Clear")]
    pub fn clear(&mut self) {
        unsafe { sys::ImFontAtlas_Clear(self.raw) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear only the fonts (keep texture data).
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&mut self) {
        unsafe { sys::ImFontAtlas_ClearFonts(self.raw) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear only the texture data (keep fonts)
    #[doc(alias = "ClearTexData")]
    pub fn clear_tex_data(&mut self) {
        unsafe { sys::ImFontAtlas_ClearTexData(self.raw) }
    }

    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default_ptr(&self) -> *const sys::ImWchar {
        if self.raw.is_null() {
            return std::ptr::null();
        }
        unsafe { sys::ImFontAtlas_GetGlyphRangesDefault(self.raw) }
    }

    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    ///
    /// The returned slice is terminated by a `0` sentinel, matching Dear ImGui's
    /// `ImWchar` range list format: `[start, end, start, end, ..., 0]`.
    ///
    /// Prefer [`Self::get_glyph_ranges_default_ptr`] when passing glyph ranges
    /// back into FFI, to avoid any scanning on the Rust side.
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default(&self) -> &[sys::ImWchar] {
        unsafe {
            let ptr = self.get_glyph_ranges_default_ptr();
            if ptr.is_null() {
                &[]
            } else {
                // Count the ranges (terminated by 0). Dear ImGui stores the list as
                // pairs: [start, end, start, end, ..., 0].
                //
                // This assumes the pointer points to a valid, null-terminated
                // static array as provided by Dear ImGui.
                const MAX_WORDS: usize = 2048;
                let mut i = 0usize;
                while i < MAX_WORDS {
                    if *ptr.add(i) == 0 {
                        return std::slice::from_raw_parts(ptr, i + 1);
                    }
                    i = i.saturating_add(2);
                }

                debug_assert!(
                    false,
                    "ImFontAtlas_GetGlyphRangesDefault() did not terminate within MAX_WORDS"
                );
                &[]
            }
        }
    }

    /// Build the font atlas texture
    ///
    /// This is a simplified build process. For more control, use the individual build functions.
    ///
    /// Note: with Dear ImGui 1.92+ "new backend" texture system, you should generally
    /// not call `build()` manually. The renderer should set `ImGuiBackendFlags_RendererHasTextures`
    /// and the atlas will be built/updated on demand.
    ///
    /// In particular, calling `build()` before the renderer sets `RendererHasTextures`
    /// may cause Dear ImGui to assert on the next frame.
    #[doc(alias = "Build")]
    pub fn build(&mut self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        // NOTE: In Dear ImGui, `ImFontAtlasBuildMain()` will call `ImFontAtlasBuildInit()`
        // lazily if needed (Builder == NULL). Calling BuildInit unconditionally would leak
        // the builder and is not idempotent.
        unsafe {
            sys::igImFontAtlasBuildMain(self.raw);
            (*self.raw).TexIsBuilt
        }
    }

    /// Discard baked font caches.
    ///
    /// This clears cached glyph data (including cached "not found" entries) so that
    /// newly added font sources (e.g. merged CJK/emoji fonts) can take effect.
    ///
    /// Pass `unused_frames = 0` to discard everything (recommended after font merging).
    ///
    /// Notes:
    /// - Only call this when the atlas is not locked (typically before `Context::frame()`).
    /// - No-op if the atlas builder hasn't been created yet.
    #[doc(alias = "ImFontAtlasBuildDiscardBakes")]
    pub fn discard_bakes(&mut self, unused_frames: i32) {
        if self.raw.is_null() {
            return;
        }
        unsafe {
            if (*self.raw).Builder.is_null() {
                return;
            }
            sys::igImFontAtlasBuildDiscardBakes(self.raw, unused_frames);
        }
    }

    /// Check if the texture is built
    pub fn is_built(&self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        unsafe { (*self.raw).TexIsBuilt }
    }

    /// Get texture data information
    ///
    /// Returns (min_width, min_height) if texture is built
    /// Note: Our Dear ImGui version uses a different texture management system
    pub fn get_tex_data_info(&self) -> Option<(u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let min_width = (*self.raw).TexMinWidth as u32;
                let min_height = (*self.raw).TexMinHeight as u32;
                Some((min_width, min_height))
            } else {
                None
            }
        }
    }

    /// Get raw texture data pointer and dimensions
    ///
    /// # Safety
    /// The returned pointer is only valid while the FontAtlas exists and the texture is built.
    /// The caller must ensure proper lifetime management.
    pub unsafe fn get_tex_data_ptr(&self) -> Option<(*const u8, u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let tex_data = (*self.raw).TexData;
                if !tex_data.is_null() {
                    let width = (*tex_data).Width as u32;
                    let height = (*tex_data).Height as u32;
                    let pixels = (*tex_data).Pixels;
                    if !pixels.is_null() {
                        Some((pixels, width, height))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    /// Get texture reference for the font atlas
    ///
    /// Note: Our Dear ImGui version uses ImTextureRef instead of a simple texture ID
    pub fn get_tex_ref(&self) -> sys::ImTextureRef {
        unsafe { (*self.raw).TexRef }
    }

    /// Set texture reference for the font atlas
    pub fn set_tex_ref(&mut self, tex_ref: sys::ImTextureRef) {
        unsafe {
            (*self.raw).TexRef = tex_ref;
        }
    }

    /// Get a mutable view of the atlas texture data, if available
    pub fn tex_data_mut(&mut self) -> Option<&mut crate::texture::TextureData> {
        let ptr = unsafe { (*self.raw).TexData };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { crate::texture::TextureData::from_raw(ptr) })
        }
    }

    /// Convenience: set atlas texture id and mark status OK
    /// Also updates TexRef so draw commands continue to follow the managed
    /// `ImTextureData` when one is available.
    pub fn set_texture_id(&mut self, tex_id: crate::texture::TextureId) {
        let tex_ref = if let Some(td) = self.tex_data_mut() {
            td.set_tex_id(tex_id);
            td.set_status(crate::texture::TextureStatus::OK);
            td.texture_ref().raw()
        } else {
            sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: tex_id.id() as sys::ImTextureID,
            }
        };

        self.set_tex_ref(tex_ref);
    }

    /// Get texture data pointer
    ///
    /// Returns the current texture data used by the atlas
    pub fn get_tex_data(&self) -> *mut sys::ImTextureData {
        unsafe { (*self.raw).TexData }
    }

    /// Get texture UV scale
    pub fn get_tex_uv_scale(&self) -> [f32; 2] {
        unsafe {
            let scale = (*self.raw).TexUvScale;
            [scale.x, scale.y]
        }
    }

    /// Get texture UV white pixel coordinates
    pub fn get_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            let pixel = (*self.raw).TexUvWhitePixel;
            [pixel.x, pixel.y]
        }
    }
}

impl Default for FontAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FontAtlas {
    fn drop(&mut self) {
        if self.owned && !self.raw.is_null() {
            unsafe {
                forget_font_atlas_generation(self.raw);
                sys::ImFontAtlas_destroy(self.raw);
            }
        }
    }
}

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.

/// Font configuration for loading fonts with v1.92+ features
#[derive(Debug)]
pub struct FontConfig {
    raw: sys::ImFontConfig,
    glyph_exclude_ranges: Option<Vec<sys::ImWchar>>,
}

impl Clone for FontConfig {
    fn clone(&self) -> Self {
        let mut raw = self.raw;
        let glyph_exclude_ranges = self.glyph_exclude_ranges.clone();
        if let Some(ref ranges) = glyph_exclude_ranges {
            raw.GlyphExcludeRanges = ranges.as_ptr();
        }
        Self {
            raw,
            glyph_exclude_ranges,
        }
    }
}

impl FontConfig {
    /// Creates a new font configuration with default settings
    pub fn new() -> Self {
        // Use ImGui's constructor to ensure all defaults are initialized
        // (e.g., RasterizerDensity defaults to 1.0f which avoids assertions).
        unsafe {
            let cfg = sys::ImFontConfig_ImFontConfig();
            if cfg.is_null() {
                panic!("ImFontConfig_ImFontConfig() returned null");
            }
            let raw = *cfg;
            sys::ImFontConfig_destroy(cfg);
            Self {
                raw,
                glyph_exclude_ranges: None,
            }
        }
    }

    /// Returns the raw ImFontConfig pointer
    pub(crate) fn raw(&self) -> *const sys::ImFontConfig {
        &self.raw
    }

    fn has_reference_size_dependent_metrics(&self) -> bool {
        self.raw.GlyphOffset.x != 0.0
            || self.raw.GlyphOffset.y != 0.0
            || self.raw.GlyphMinAdvanceX != 0.0
            || self.raw.GlyphMaxAdvanceX != f32::MAX
    }

    fn validate_common(&self, caller: &str) {
        validate_font_size_pixels(caller, "SizePixels", self.raw.SizePixels);
        assert_finite_vec2(
            caller,
            "GlyphOffset",
            [self.raw.GlyphOffset.x, self.raw.GlyphOffset.y],
        );
        assert_non_negative_f32(caller, "GlyphMinAdvanceX", self.raw.GlyphMinAdvanceX);
        assert_non_negative_f32(caller, "GlyphMaxAdvanceX", self.raw.GlyphMaxAdvanceX);
        assert!(
            self.raw.GlyphMinAdvanceX <= self.raw.GlyphMaxAdvanceX,
            "{caller} GlyphMinAdvanceX must be less than or equal to GlyphMaxAdvanceX"
        );
        assert_finite_f32(caller, "GlyphExtraAdvanceX", self.raw.GlyphExtraAdvanceX);
        assert_non_negative_f32(caller, "RasterizerMultiply", self.raw.RasterizerMultiply);
        assert!(
            self.raw.RasterizerMultiply <= RASTERIZER_MULTIPLY_MAX,
            "{caller} RasterizerMultiply must be less than or equal to {RASTERIZER_MULTIPLY_MAX}"
        );
        assert_positive_f32(caller, "RasterizerDensity", self.raw.RasterizerDensity);
        assert_non_negative_i8(caller, "OversampleH", self.raw.OversampleH);
        assert_non_negative_i8(caller, "OversampleV", self.raw.OversampleV);
    }

    fn validate_for_add_font(&self, caller: &str) {
        self.validate_common(caller);
        assert_font_source_for_add_font(caller, &self.raw);
        assert_reference_font_size_for_metrics(
            caller,
            self.raw.SizePixels,
            self.has_reference_size_dependent_metrics(),
        );
    }

    fn validate_for_add_font_default(&self, caller: &str) {
        self.validate_common(caller);
    }

    fn validate_for_add_font_with_size(&self, caller: &str, size_pixels: f32) {
        self.validate_common(caller);
        let effective_size_pixels = if size_pixels > 0.0 {
            size_pixels
        } else {
            self.raw.SizePixels
        };
        assert_reference_font_size_for_metrics(
            caller,
            effective_size_pixels,
            self.has_reference_size_dependent_metrics(),
        );
    }

    /// Set the font size in pixels
    ///
    /// Note: With v1.92+ dynamic fonts, size can be 0.0 to use default sizing
    pub fn size_pixels(mut self, size: f32) -> Self {
        validate_font_size_pixels("FontConfig::size_pixels()", "size", size);
        self.raw.SizePixels = size;
        self
    }

    /// Set whether to merge this font with the previous one
    pub fn merge_mode(mut self, merge: bool) -> Self {
        self.raw.MergeMode = merge;
        self
    }

    /// Control whether the atlas takes ownership of `FontData` passed from memory.
    ///
    /// Dear ImGui's `AddFontFromMemoryTTF()` stores the `FontData` pointer for potential rebuilds.
    /// When this flag is `true`, the atlas will later free `FontData` using Dear ImGui's allocator.
    /// When it is `false`, Dear ImGui will *not* free the pointer and the caller must ensure the
    /// memory stays valid for as long as the atlas may use it.
    pub fn font_data_owned_by_atlas(mut self, owned: bool) -> Self {
        self.raw.FontDataOwnedByAtlas = owned;
        self
    }

    /// Set font loader flags for this specific font
    ///
    /// These flags override the global atlas flags for this font.
    pub fn font_loader_flags(mut self, flags: FontLoaderFlags) -> Self {
        self.raw.FontLoaderFlags = flags.0;
        self
    }

    /// Set inclusive glyph ranges to exclude from this font.
    ///
    /// The input is a slice of `(start, end)` pairs. It is converted to Dear ImGui's
    /// `[start, end, ..., 0]` format.
    pub fn glyph_exclude_ranges(mut self, ranges: &[(u32, u32)]) -> Self {
        if ranges.is_empty() {
            self.raw.GlyphExcludeRanges = ptr::null();
            self.glyph_exclude_ranges = None;
            return self;
        }

        const IMWCHAR_MAX: u32 = if std::mem::size_of::<sys::ImWchar>() == 2 {
            0xFFFF
        } else {
            0x10FFFF
        };
        let mut converted: Vec<sys::ImWchar> = Vec::with_capacity(ranges.len() * 2 + 1);
        for &(start, end) in ranges {
            assert!(
                start <= end,
                "glyph_exclude_ranges range start must be <= end: {start:#x}..={end:#x}"
            );
            assert!(
                start <= IMWCHAR_MAX,
                "glyph_exclude_ranges value out of range for ImWchar (max {IMWCHAR_MAX:#x}): {start:#x}"
            );
            assert!(
                end <= IMWCHAR_MAX,
                "glyph_exclude_ranges value out of range for ImWchar (max {IMWCHAR_MAX:#x}): {end:#x}"
            );
            let start = sys::ImWchar::try_from(start).unwrap_or_else(|_| {
                panic!("glyph_exclude_ranges value {start:#x} was not representable as ImWchar")
            });
            let end = sys::ImWchar::try_from(end).unwrap_or_else(|_| {
                panic!("glyph_exclude_ranges value {end:#x} was not representable as ImWchar")
            });
            converted.push(start);
            converted.push(end);
        }
        if converted.last().copied() != Some(0) {
            converted.push(0);
        }

        self.raw.GlyphExcludeRanges = converted.as_ptr();
        self.glyph_exclude_ranges = Some(converted);
        self
    }

    /// Set a custom font loader for this font.
    ///
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*` in the
    /// atlas font source.
    pub fn font_loader(mut self, loader: &'static FontLoader) -> Self {
        self.raw.FontLoader = loader.as_ptr();
        self
    }

    /// Set the font name for debugging
    pub fn name(mut self, name: &str) -> Self {
        let name_bytes = name.as_bytes();
        let copy_len = std::cmp::min(name_bytes.len(), self.raw.Name.len() - 1);

        // Clear the array first
        for i in 0..self.raw.Name.len() {
            self.raw.Name[i] = 0;
        }

        // Copy the name
        for (i, &byte) in name_bytes.iter().take(copy_len).enumerate() {
            self.raw.Name[i] = byte as c_char;
        }

        self
    }

    /// Set glyph offset for this font
    pub fn glyph_offset(mut self, offset: [f32; 2]) -> Self {
        assert_finite_vec2("FontConfig::glyph_offset()", "offset", offset);
        self.raw.GlyphOffset.x = offset[0];
        self.raw.GlyphOffset.y = offset[1];
        self
    }

    /// Set minimum advance X for glyphs
    pub fn glyph_min_advance_x(mut self, advance: f32) -> Self {
        assert_non_negative_f32("FontConfig::glyph_min_advance_x()", "advance", advance);
        assert!(
            advance <= self.raw.GlyphMaxAdvanceX,
            "FontConfig::glyph_min_advance_x() advance must be less than or equal to current glyph_max_advance_x"
        );
        self.raw.GlyphMinAdvanceX = advance;
        self
    }

    /// Set maximum advance X for glyphs
    pub fn glyph_max_advance_x(mut self, advance: f32) -> Self {
        assert_non_negative_f32("FontConfig::glyph_max_advance_x()", "advance", advance);
        assert!(
            advance >= self.raw.GlyphMinAdvanceX,
            "FontConfig::glyph_max_advance_x() advance must be greater than or equal to current glyph_min_advance_x"
        );
        self.raw.GlyphMaxAdvanceX = advance;
        self
    }

    /// Set extra advance X for glyphs (spacing between characters)
    pub fn glyph_extra_advance_x(mut self, advance: f32) -> Self {
        assert_finite_f32("FontConfig::glyph_extra_advance_x()", "advance", advance);
        self.raw.GlyphExtraAdvanceX = advance;
        self
    }

    /// Set rasterizer multiply factor
    pub fn rasterizer_multiply(mut self, multiply: f32) -> Self {
        assert_non_negative_f32("FontConfig::rasterizer_multiply()", "multiply", multiply);
        assert!(
            multiply <= RASTERIZER_MULTIPLY_MAX,
            "FontConfig::rasterizer_multiply() multiply must be less than or equal to {RASTERIZER_MULTIPLY_MAX}"
        );
        self.raw.RasterizerMultiply = multiply;
        self
    }

    /// Set rasterizer density for DPI scaling
    pub fn rasterizer_density(mut self, density: f32) -> Self {
        assert_positive_f32("FontConfig::rasterizer_density()", "density", density);
        self.raw.RasterizerDensity = density;
        self
    }

    /// Set pixel snap horizontally
    pub fn pixel_snap_h(mut self, snap: bool) -> Self {
        self.raw.PixelSnapH = snap;
        self
    }

    /// Set horizontal oversampling
    pub fn oversample_h(mut self, oversample: i8) -> Self {
        assert_non_negative_i8("FontConfig::oversample_h()", "oversample", oversample);
        self.raw.OversampleH = oversample;
        self
    }

    /// Set vertical oversampling
    pub fn oversample_v(mut self, oversample: i8) -> Self {
        assert_non_negative_i8("FontConfig::oversample_v()", "oversample", oversample);
        self.raw.OversampleV = oversample;
        self
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_config_glyph_exclude_ranges_converts_and_terminates() {
        let cfg = FontConfig::new().glyph_exclude_ranges(&[(0x41, 0x5a)]);
        assert!(!cfg.raw.GlyphExcludeRanges.is_null());
        unsafe {
            assert_eq!(*cfg.raw.GlyphExcludeRanges.add(0), 0x41 as sys::ImWchar);
            assert_eq!(*cfg.raw.GlyphExcludeRanges.add(1), 0x5a as sys::ImWchar);
            assert_eq!(*cfg.raw.GlyphExcludeRanges.add(2), 0);
        }
    }

    #[test]
    fn font_config_glyph_exclude_ranges_accepts_non_bmp_when_wchar32() {
        if std::mem::size_of::<sys::ImWchar>() != 4 {
            return;
        }
        let cfg = FontConfig::new().glyph_exclude_ranges(&[(0x1_0000, 0x1_0001)]);
        assert!(!cfg.raw.GlyphExcludeRanges.is_null());
        unsafe {
            assert_eq!(*cfg.raw.GlyphExcludeRanges.add(0), 0x1_0000 as sys::ImWchar);
            assert_eq!(*cfg.raw.GlyphExcludeRanges.add(1), 0x1_0001 as sys::ImWchar);
            assert_eq!(*cfg.raw.GlyphExcludeRanges.add(2), 0);
        }
    }

    #[test]
    fn font_config_glyph_exclude_ranges_rejects_out_of_range() {
        let out_of_range = if std::mem::size_of::<sys::ImWchar>() == 2 {
            0x1_0000
        } else {
            0x11_0000
        };
        let res = std::panic::catch_unwind(|| {
            let _ = FontConfig::new().glyph_exclude_ranges(&[(out_of_range, out_of_range)]);
        });
        assert!(res.is_err());
    }

    #[test]
    fn font_config_glyph_exclude_ranges_rejects_reversed_ranges() {
        let res = std::panic::catch_unwind(|| {
            let _ = FontConfig::new().glyph_exclude_ranges(&[(0x42, 0x41)]);
        });
        assert!(res.is_err());
    }

    #[test]
    fn font_config_rejects_invalid_numeric_inputs() {
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().size_pixels(f32::NAN);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().size_pixels(-1.0);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().glyph_offset([0.0, f32::INFINITY]);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().glyph_min_advance_x(-1.0);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new()
                    .glyph_min_advance_x(12.0)
                    .glyph_max_advance_x(8.0);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().glyph_extra_advance_x(f32::NAN);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().rasterizer_multiply(-0.1);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().rasterizer_multiply(RASTERIZER_MULTIPLY_MAX * 2.0);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().rasterizer_density(0.0);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = FontConfig::new().oversample_h(-1);
            })
            .is_err()
        );

        let cfg = FontConfig::new()
            .size_pixels(0.0)
            .glyph_offset([0.0, 0.0])
            .glyph_min_advance_x(0.0)
            .glyph_max_advance_x(f32::MAX)
            .glyph_extra_advance_x(-1.0)
            .rasterizer_multiply(256.0)
            .rasterizer_density(1.0)
            .oversample_h(0)
            .oversample_v(1);
        assert_eq!(cfg.raw.SizePixels, 0.0);
        assert_eq!(cfg.raw.GlyphExtraAdvanceX, -1.0);
        assert_eq!(cfg.raw.RasterizerMultiply, 256.0);
    }

    #[test]
    fn font_atlas_rejects_glyph_metric_overrides_without_reference_size() {
        let mut atlas = FontAtlas::new();
        let cfg = FontConfig::new().glyph_offset([1.0, 0.0]);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = atlas.add_font_from_memory_ttf(&[0u8; 10], 0.0, Some(&cfg), None);
            }))
            .is_err()
        );

        assert!(
            atlas
                .add_font_from_memory_ttf(&[0u8; 10], 13.0, Some(&cfg), None)
                .is_none()
        );

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cfg = FontConfig::new()
                    .font_data_owned_by_atlas(false)
                    .glyph_min_advance_x(4.0);
                let _ = atlas.add_font_with_config(&cfg);
            }))
            .is_err()
        );
    }

    #[test]
    fn add_font_with_config_rejects_missing_font_source_before_ffi() {
        let mut atlas = FontAtlas::new();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = atlas.add_font_with_config(&FontConfig::new());
            }))
            .is_err()
        );
    }

    #[test]
    fn add_font_from_memory_ttf_rejects_too_small_buffers() {
        let mut ctx = crate::Context::create();
        let mut fonts = ctx.font_atlas_mut();
        assert!(
            fonts
                .add_font_from_memory_ttf(&[0u8; 10], 13.0, None, None)
                .is_none()
        );
    }

    #[test]
    fn font_id_is_invalidated_by_clear_fonts_before_push_font_ffi() {
        let mut ctx = crate::Context::create();
        let font_id = {
            let mut fonts = ctx.font_atlas_mut();
            fonts.add_font(&[FontSource::default_font()])
        };
        {
            let mut fonts = ctx.font_atlas_mut();
            fonts.clear_fonts();
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = validate_font_id_for_current_context(font_id, "test stale FontId");
        }));

        assert!(result.is_err());
    }

    #[test]
    fn font_id_from_another_atlas_is_rejected_before_push_font_ffi() {
        let mut ctx_a = crate::Context::create();
        let font_id = {
            let mut fonts = ctx_a.font_atlas_mut();
            fonts.add_font(&[FontSource::default_font()])
        };
        let suspended_a = ctx_a.suspend();

        let mut ctx_b = crate::Context::create();
        let _ = ctx_b.font_atlas_mut().build();
        ctx_b.io_mut().set_display_size([128.0, 128.0]);
        ctx_b.io_mut().set_delta_time(1.0 / 60.0);
        let ui = ctx_b.frame();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _token = ui.push_font(font_id);
        }));

        assert!(result.is_err());

        drop(ctx_b);
        drop(suspended_a);
    }

    #[test]
    fn font_id_from_shared_atlas_is_valid_through_another_atlas_view() {
        let shared_atlas = SharedFontAtlas::create();
        let raw = *shared_atlas.0;
        let font_id = {
            let mut atlas = unsafe { FontAtlas::from_raw(raw) };
            atlas.add_font(&[FontSource::default_font()])
        };

        let _ = validate_font_id_for_atlas(font_id, raw, "test shared FontId");
    }

    #[test]
    fn font_sources_reject_invalid_sizes_before_ffi() {
        let mut atlas = FontAtlas::new();

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = atlas.add_font(&[FontSource::default_font_with_size(f32::NAN)]);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = atlas.add_font(&[FontSource::ttf_data_with_size(&[0u8; 10], -1.0)]);
            }))
            .is_err()
        );
    }

    #[test]
    fn set_texture_id_preserves_managed_tex_data_reference() {
        let mut ctx = crate::Context::create();
        let mut fonts = ctx.font_atlas_mut();
        let _ = fonts.build();

        let raw_tex_data = fonts.get_tex_data();
        assert!(!raw_tex_data.is_null());

        let texture_id = crate::texture::TextureId::new(0x1234);
        fonts.set_texture_id(texture_id);

        let mut tex_ref = fonts.get_tex_ref();
        assert_eq!(tex_ref._TexData, raw_tex_data);

        let resolved = unsafe { sys::ImTextureRef_GetTexID(&mut tex_ref) };
        assert_eq!(resolved, texture_id.id() as sys::ImTextureID);
    }
}

/// A source for font data with v1.92+ dynamic font support
#[derive(Clone, Debug)]
pub enum FontSource<'a> {
    /// Default font included with the library (ProggyClean.ttf)
    ///
    /// With v1.92+, size_pixels can be 0.0 for dynamic sizing
    DefaultFontData {
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Binary TTF/OTF font data
    ///
    /// With v1.92+, size_pixels can be 0.0 for dynamic sizing
    TtfData {
        data: &'a [u8],
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Compressed TTF font data (stb-compressed)
    ///
    /// Dear ImGui decompresses immediately and keeps the decompressed buffer owned by the atlas.
    CompressedTtfData {
        data: &'a [u8],
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Compressed + base85-encoded TTF font data
    ///
    /// The provided string is converted into a NUL-terminated `CString` for Dear ImGui.
    CompressedTtfBase85 {
        data: &'a str,
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Font from file path
    ///
    /// With v1.92+, size_pixels can be 0.0 for dynamic sizing
    TtfFile {
        path: &'a str,
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },
}

impl<'a> FontSource<'a> {
    /// Creates a default font source with dynamic sizing
    pub fn default_font() -> Self {
        Self::DefaultFontData {
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a default font source with specific size
    pub fn default_font_with_size(size: f32) -> Self {
        Self::DefaultFontData {
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a TTF data source with dynamic sizing
    pub fn ttf_data(data: &'a [u8]) -> Self {
        Self::TtfData {
            data,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a TTF data source with specific size
    pub fn ttf_data_with_size(data: &'a [u8], size: f32) -> Self {
        Self::TtfData {
            data,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a compressed TTF data source with dynamic sizing
    pub fn compressed_ttf_data(data: &'a [u8]) -> Self {
        Self::CompressedTtfData {
            data,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a compressed TTF data source with specific size
    pub fn compressed_ttf_data_with_size(data: &'a [u8], size: f32) -> Self {
        Self::CompressedTtfData {
            data,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a base85 compressed TTF source with dynamic sizing
    pub fn compressed_ttf_base85(data: &'a str) -> Self {
        Self::CompressedTtfBase85 {
            data,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a base85 compressed TTF source with specific size
    pub fn compressed_ttf_base85_with_size(data: &'a str, size: f32) -> Self {
        Self::CompressedTtfBase85 {
            data,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a TTF file source with dynamic sizing
    pub fn ttf_file(path: &'a str) -> Self {
        Self::TtfFile {
            path,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a TTF file source with specific size
    pub fn ttf_file_with_size(path: &'a str, size: f32) -> Self {
        Self::TtfFile {
            path,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Sets the font configuration for this source
    pub fn with_config(mut self, config: FontConfig) -> Self {
        match &mut self {
            Self::DefaultFontData { config: cfg, .. } => *cfg = Some(config),
            Self::TtfData { config: cfg, .. } => *cfg = Some(config),
            Self::CompressedTtfData { config: cfg, .. } => *cfg = Some(config),
            Self::CompressedTtfBase85 { config: cfg, .. } => *cfg = Some(config),
            Self::TtfFile { config: cfg, .. } => *cfg = Some(config),
        }
        self
    }
}

/// Handle to a font atlas texture
#[derive(Clone, Debug)]
pub struct FontAtlasTexture<'a> {
    /// Texture width (in pixels)
    pub width: u32,
    /// Texture height (in pixels)
    pub height: u32,
    /// Raw texture data (in bytes).
    ///
    /// The format depends on which function was called to obtain this data:
    /// - For RGBA32: 4 bytes per pixel (R, G, B, A)
    /// - For Alpha8: 1 byte per pixel (Alpha only)
    pub data: &'a [u8],
}
