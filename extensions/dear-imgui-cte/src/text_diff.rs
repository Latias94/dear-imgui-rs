use crate::{
    CteError, CteResult, Language, Palette,
    context::CteContextBinding,
    error::c_string,
    sys,
    validation::{
        validate_finite_f32, validate_finite_vec2, validate_nonzero_usize, validate_render_flags,
    },
};
use dear_imgui_rs::{ChildFlags, Context, ContextId, Ui, WindowFlags};
use std::{marker::PhantomData, ptr::NonNull, rc::Rc};

/// An owned, context-bound syntax-highlighted text diff.
///
/// The diff is intentionally neither [`Send`] nor [`Sync`]. Every native call is
/// made while its originating Dear ImGui context is current.
pub struct TextDiff {
    raw: NonNull<sys::TextDiff>,
    binding: CteContextBinding,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl TextDiff {
    /// Creates a text diff bound to `context`.
    pub fn try_create(context: &Context) -> CteResult<Self> {
        let binding = CteContextBinding::new(context);
        let raw = binding.try_with_bound_context("TextDiff::try_create", || unsafe {
            sys::TextDiff_TextDiff()
        })?;
        let raw = NonNull::new(raw).ok_or(CteError::CreationFailed { object: "TextDiff" })?;
        Ok(Self {
            raw,
            binding,
            _not_send_sync: PhantomData,
        })
    }

    /// Creates a text diff and panics if native allocation fails.
    pub fn create(context: &Context) -> Self {
        Self::try_create(context).expect("failed to create cimCTE TextDiff")
    }

    /// Returns the stable identity of the owning Dear ImGui context.
    pub fn context_id(&self) -> ContextId {
        self.binding.id()
    }

    /// Returns the raw text-diff pointer.
    ///
    /// # Safety
    ///
    /// The pointer may only be used while the owning Dear ImGui context is current. The
    /// caller must preserve every cimCTE precondition, ownership and pointer-lifetime rule,
    /// and invariant relied on by this safe wrapper. In particular, the caller must not
    /// destroy the diff, retain borrowed pointers, set a zero tab size, or install a language
    /// pointer that can become invalid.
    pub unsafe fn as_raw(&self) -> *mut sys::TextDiff {
        self.raw.as_ptr()
    }

    /// Replaces both documents after validating both strings.
    pub fn set_text(&mut self, left: &str, right: &str) -> CteResult<()> {
        const OPERATION: &str = "TextDiff::set_text";
        let left = c_string(OPERATION, left)?;
        let right = c_string(OPERATION, right)?;
        self.with_context(OPERATION, |raw| unsafe {
            sys::TextDiff_SetText(raw, left.as_ptr(), right.as_ptr())
        });
        Ok(())
    }

    pub fn set_tab_size(&mut self, value: usize) -> CteResult<()> {
        validate_nonzero_usize("TextDiff::set_tab_size", "value", value)?;
        self.with_context("TextDiff::set_tab_size", |raw| unsafe {
            sys::TextDiff_SetTabSize(raw, value)
        });
        Ok(())
    }

    pub fn tab_size(&self) -> usize {
        self.with_context("TextDiff::tab_size", |raw| unsafe {
            sys::TextDiff_GetTabSize(raw)
        })
    }

    /// Sets line spacing. The native widget clamps finite values to `1.0..=2.0`.
    pub fn set_line_spacing(&mut self, value: f32) -> CteResult<()> {
        validate_finite_f32("TextDiff::set_line_spacing", "value", value)?;
        self.with_context("TextDiff::set_line_spacing", |raw| unsafe {
            sys::TextDiff_SetLineSpacing(raw, value)
        });
        Ok(())
    }

    pub fn line_spacing(&self) -> f32 {
        self.with_context("TextDiff::line_spacing", |raw| unsafe {
            sys::TextDiff_GetLineSpacing(raw)
        })
    }

    pub fn set_language(&mut self, language: Option<Language>) {
        let language = language.map_or(std::ptr::null(), Language::as_raw);
        self.with_context("TextDiff::set_language", |raw| unsafe {
            sys::TextDiff_SetLanguage(raw, language)
        });
    }

    pub fn language(&self) -> Option<Language> {
        self.with_context("TextDiff::language", |raw| unsafe {
            Language::from_raw(sys::TextDiff_GetLanguage(raw))
        })
    }

    /// Sets the packed ImGui colors used for added and deleted lines.
    pub fn set_colors(&mut self, added: u32, deleted: u32) {
        self.with_context("TextDiff::set_colors", |raw| unsafe {
            sys::TextDiff_SetColors(raw, added, deleted)
        });
    }

    /// Copies a Rust palette into the text diff.
    pub fn set_palette(&mut self, palette: &Palette) -> CteResult<()> {
        self.with_context("TextDiff::set_palette", |raw| {
            palette.with_native(|native| unsafe { sys::TextDiff_SetPalette(raw, native) })
        })
    }

    /// Returns an owned copy of the text-diff palette.
    pub fn palette(&self) -> Palette {
        self.with_context("TextDiff::palette", |raw| unsafe {
            Palette::copy_from_raw(sys::TextDiff_GetPalette(raw))
        })
    }

    /// Requests keyboard focus on the next render.
    pub fn focus(&mut self) {
        self.with_context("TextDiff::focus", |raw| unsafe {
            sys::TextDiff_SetFocus(raw)
        });
    }

    fn with_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::TextDiff) -> R,
    ) -> R {
        self.try_with_context(operation, f)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn try_with_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::TextDiff) -> R,
    ) -> CteResult<R> {
        let raw = self.raw.as_ptr();
        self.binding.try_with_bound_context(operation, || f(raw))
    }
}

macro_rules! bool_property {
    ($setter:ident, $getter:ident, $raw_setter:path, $raw_getter:path) => {
        impl TextDiff {
            pub fn $setter(&mut self, value: bool) {
                self.with_context(concat!("TextDiff::", stringify!($setter)), |raw| unsafe {
                    $raw_setter(raw, value)
                });
            }

            pub fn $getter(&self) -> bool {
                self.with_context(concat!("TextDiff::", stringify!($getter)), |raw| unsafe {
                    $raw_getter(raw)
                })
            }
        }
    };
}

bool_property!(
    set_side_by_side,
    is_side_by_side,
    sys::TextDiff_SetSideBySideMode,
    sys::TextDiff_GetSideBySideMode
);
bool_property!(
    set_word_wrap_enabled,
    is_word_wrap_enabled,
    sys::TextDiff_SetWordWrapEnabled,
    sys::TextDiff_IsWordWrapEnabled
);
bool_property!(
    set_show_whitespaces,
    shows_whitespaces,
    sys::TextDiff_SetShowWhitespacesEnabled,
    sys::TextDiff_IsShowWhitespacesEnabled
);
bool_property!(
    set_show_spaces,
    shows_spaces,
    sys::TextDiff_SetShowSpacesEnabled,
    sys::TextDiff_IsShowSpacesEnabled
);
bool_property!(
    set_show_tabs,
    shows_tabs,
    sys::TextDiff_SetShowTabsEnabled,
    sys::TextDiff_IsShowTabsEnabled
);
bool_property!(
    set_show_scrollbar_minimap,
    shows_scrollbar_minimap,
    sys::TextDiff_SetShowScrollbarMiniMapEnabled,
    sys::TextDiff_IsShowScrollbarMiniMapEnabled
);

impl Drop for TextDiff {
    fn drop(&mut self) {
        let raw = self.raw;
        let _ = self
            .binding
            .try_with_bound_context("TextDiff::drop", || unsafe {
                sys::TextDiff_destroy(raw.as_ptr())
            });
        // If context teardown already started, touching CTE state is no longer proven safe.
        // The native handle is intentionally leaked rather than calling into a dead context.
    }
}

/// Builder for one TextDiff render submission.
#[must_use = "call build() to render the text diff"]
pub struct TextDiffRenderer<'ui, 'diff> {
    ui: &'ui Ui,
    diff: &'diff mut TextDiff,
    title: String,
    size: [f32; 2],
    child_flags: ChildFlags,
    window_flags: WindowFlags,
}

impl TextDiffRenderer<'_, '_> {
    pub(crate) fn new<'ui, 'diff>(
        ui: &'ui Ui,
        diff: &'diff mut TextDiff,
        title: String,
    ) -> TextDiffRenderer<'ui, 'diff> {
        TextDiffRenderer {
            ui,
            diff,
            title,
            size: [0.0, 0.0],
            child_flags: ChildFlags::empty(),
            window_flags: WindowFlags::NO_MOVE,
        }
    }

    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    pub fn child_flags(mut self, flags: ChildFlags) -> Self {
        self.child_flags = flags;
        self
    }

    /// Sets additional window flags while preserving the widget's required `NO_MOVE` flag.
    pub fn window_flags(mut self, flags: WindowFlags) -> Self {
        self.window_flags = flags | WindowFlags::NO_MOVE;
        self
    }

    pub fn build(self) -> CteResult<()> {
        const OPERATION: &str = "TextDiffRenderer::build";
        self.diff.binding.require_ui(OPERATION, self.ui)?;
        validate_finite_vec2(OPERATION, "size", self.size)?;
        validate_render_flags(OPERATION, self.child_flags, self.window_flags)?;
        let title = c_string(OPERATION, &self.title)?;
        let child_flags = self.child_flags.bits() as i32;
        self.diff.try_with_context(OPERATION, |raw| unsafe {
            sys::TextDiff_Render(
                raw,
                title.as_ptr(),
                self.size.into(),
                child_flags,
                self.window_flags.bits(),
            )
        })
    }
}
