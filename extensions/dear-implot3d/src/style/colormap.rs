use crate::sys;
use crate::{Plot3DContext, Plot3DUi};

use super::tokens::ColormapToken;
use super::types::{ColormapColorIndex, ColormapIndex};
use std::marker::PhantomData;

impl<'ui> Plot3DUi<'ui> {
    #[inline]
    pub fn push_colormap(&self, cmap: impl Into<ColormapIndex>) -> ColormapToken<'_> {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_PushColormap_Plot3DColormap(cmap.into().raw()) }
        ColormapToken {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            was_popped: false,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }

    #[inline]
    pub fn push_colormap_name(&self, name: &str) -> ColormapToken<'_> {
        assert!(!name.contains('\0'), "colormap name contained NUL");
        let _guard = self.bind();
        dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe { sys::ImPlot3D_PushColormap_Str(ptr) });
        ColormapToken {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            was_popped: false,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }
}

pub(super) fn colormap_count_from_i32(raw: i32, caller: &str) -> usize {
    assert!(raw >= 0, "{caller} returned a negative colormap count");
    usize::try_from(raw).expect("non-negative colormap count must fit usize")
}

impl Plot3DContext {
    #[inline]
    fn with_bound_colormap<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.assert_imgui_alive(caller);
        let _guard = self.binding().bind();
        f()
    }

    /// Return the number of available ImPlot3D colormaps.
    #[inline]
    pub fn colormap_count(&self) -> usize {
        self.with_bound_colormap("dear-implot3d: Plot3DContext::colormap_count()", || {
            colormap_count_from_i32(
                unsafe { sys::ImPlot3D_GetColormapCount() },
                "Plot3DContext::colormap_count()",
            )
        })
    }

    /// Return a colormap name, or an empty string if the index is invalid for this context.
    #[inline]
    pub fn colormap_name(&self, index: impl Into<ColormapIndex>) -> String {
        self.with_bound_colormap("dear-implot3d: Plot3DContext::colormap_name()", || unsafe {
            let p = sys::ImPlot3D_GetColormapName(index.into().raw());
            if p.is_null() {
                return String::new();
            }
            std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
        })
    }

    /// Return the number of color entries in a colormap.
    #[inline]
    pub fn colormap_size(&self, index: impl Into<ColormapIndex>) -> usize {
        self.with_bound_colormap("dear-implot3d: Plot3DContext::colormap_size()", || {
            colormap_count_from_i32(
                unsafe { sys::ImPlot3D_GetColormapSize(index.into().raw()) },
                "Plot3DContext::colormap_size()",
            )
        })
    }

    /// Return the default colormap stored in this ImPlot3D context's style.
    #[inline]
    pub fn style_colormap_index(&self) -> Option<ColormapIndex> {
        self.with_bound_colormap(
            "dear-implot3d: Plot3DContext::style_colormap_index()",
            || unsafe {
                let style = sys::ImPlot3D_GetStyle();
                if style.is_null() {
                    return None;
                }
                ColormapIndex::from_raw((*style).Colormap)
            },
        )
    }

    /// Return this context's default colormap name.
    #[inline]
    pub fn style_colormap_name(&self) -> Option<String> {
        let idx = self.style_colormap_index()?;
        let count = self.colormap_count();
        if idx.get() >= count {
            return None;
        }
        Some(self.colormap_name(idx))
    }

    /// Permanently set the default colormap used by this ImPlot3D context.
    #[inline]
    pub fn set_style_colormap(&self, index: impl Into<ColormapIndex>) {
        self.with_bound_colormap(
            "dear-implot3d: Plot3DContext::set_style_colormap()",
            || unsafe {
                let style = sys::ImPlot3D_GetStyle();
                if !style.is_null() {
                    let count = colormap_count_from_i32(
                        sys::ImPlot3D_GetColormapCount(),
                        "Plot3DContext::set_style_colormap()",
                    );
                    if count > 0 {
                        let index = index.into().get();
                        let idx = index.min(count - 1);
                        (*style).Colormap = ColormapIndex::from(idx).raw();
                    }
                }
            },
        )
    }

    /// Look up a colormap index by its name.
    #[inline]
    pub fn colormap_index_by_name(&self, name: &str) -> Option<ColormapIndex> {
        if name.contains('\0') {
            return None;
        }
        let index = self.with_bound_colormap(
            "dear-implot3d: Plot3DContext::colormap_index_by_name()",
            || {
                dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe {
                    sys::ImPlot3D_GetColormapIndex(ptr)
                })
            },
        );
        ColormapIndex::from_raw(index)
    }

    /// Permanently set the default colormap by name. Invalid names are ignored.
    #[inline]
    pub fn set_style_colormap_by_name(&self, name: &str) {
        if let Some(idx) = self.colormap_index_by_name(name) {
            self.set_style_colormap(idx);
        }
    }

    /// Return a color from this context's active colormap.
    pub fn colormap_color(&self, index: ColormapColorIndex) -> [f32; 4] {
        self.with_bound_colormap(
            "dear-implot3d: Plot3DContext::colormap_color()",
            || unsafe {
                let out = crate::compat_ffi::ImPlot3D_GetColormapColor(
                    index.raw(),
                    (-1) as sys::ImPlot3DColormap,
                );
                [out.x, out.y, out.z, out.w]
            },
        )
    }

    /// Return the next color from this context's current colormap and advance its color cursor.
    pub fn next_colormap_color(&self) -> [f32; 4] {
        self.with_bound_colormap(
            "dear-implot3d: Plot3DContext::next_colormap_color()",
            || unsafe {
                let out = crate::compat_ffi::ImPlot3D_NextColormapColor();
                [out.x, out.y, out.z, out.w]
            },
        )
    }
}

impl Plot3DUi<'_> {
    /// Return a color from this UI's active ImPlot3D colormap.
    pub fn colormap_color(&self, index: ColormapColorIndex) -> [f32; 4] {
        let _guard = self.bind();
        unsafe {
            let out = crate::compat_ffi::ImPlot3D_GetColormapColor(
                index.raw(),
                (-1) as sys::ImPlot3DColormap,
            );
            [out.x, out.y, out.z, out.w]
        }
    }

    /// Return the next color from this UI's current colormap and advance its color cursor.
    pub fn next_colormap_color(&self) -> [f32; 4] {
        let _guard = self.bind();
        unsafe {
            let out = crate::compat_ffi::ImPlot3D_NextColormapColor();
            [out.x, out.y, out.z, out.w]
        }
    }
}
