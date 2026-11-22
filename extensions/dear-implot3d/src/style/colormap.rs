use crate::sys;
use crate::{Plot3DContext, Plot3DScope, Plot3DUi};

use super::types::{ColormapColorIndex, ColormapIndex};

impl Plot3DUi<'_> {
    /// Uses a registered colormap for `f`. An invalid index panics before the native push.
    pub fn with_colormap<R>(&self, cmap: impl Into<ColormapIndex>, f: impl FnOnce() -> R) -> R {
        let cmap = cmap.into();
        self.with_style_scope(
            1,
            || {
                assert!(
                    cmap.raw() < unsafe { sys::ImPlot3D_GetColormapCount() },
                    "unregistered ImPlot3D colormap"
                );
                unsafe { sys::ImPlot3D_PushColormap_Plot3DColormap(cmap.raw()) };
            },
            sys::ImPlot3D_PopColormap,
            f,
        )
    }

    /// Uses a registered colormap by name for `f`. Returns `None` for an unknown name.
    pub fn with_colormap_name<R>(&self, name: &str, f: impl FnOnce() -> R) -> Option<R> {
        let index = self.context.colormap_index_by_name(name)?;
        Some(self.with_colormap(index, f))
    }
}

pub(super) fn colormap_count_from_i32(raw: i32, caller: &str) -> usize {
    assert!(raw >= 0, "{caller} returned a negative colormap count");
    usize::try_from(raw).expect("non-negative colormap count must fit usize")
}

impl Plot3DContext {
    /// Return the number of available ImPlot3D colormaps.
    #[inline]
    pub fn colormap_count(&self) -> usize {
        self.with_bound_context(|| {
            colormap_count_from_i32(
                unsafe { sys::ImPlot3D_GetColormapCount() },
                "Plot3DContext::colormap_count()",
            )
        })
    }

    /// Return a colormap name, or an empty string if the index is invalid for this context.
    #[inline]
    pub fn colormap_name(&self, index: impl Into<ColormapIndex>) -> String {
        let index = index.into();
        self.with_bound_context(|| unsafe {
            let p = sys::ImPlot3D_GetColormapName(index.raw());
            if p.is_null() {
                return String::new();
            }
            std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
        })
    }

    /// Returns the number of color entries, or zero for an index outside this context.
    #[inline]
    pub fn colormap_size(&self, index: impl Into<ColormapIndex>) -> usize {
        let index = index.into();
        self.with_bound_context(|| {
            if index.raw() >= unsafe { sys::ImPlot3D_GetColormapCount() } {
                return 0;
            }
            colormap_count_from_i32(
                unsafe { sys::ImPlot3D_GetColormapSize(index.raw()) },
                "Plot3DContext::colormap_size()",
            )
        })
    }

    /// Return the default colormap stored in this ImPlot3D context's style.
    #[inline]
    pub fn style_colormap_index(&self) -> Option<ColormapIndex> {
        self.with_bound_context(|| unsafe {
            let style = sys::ImPlot3D_GetStyle();
            if style.is_null() {
                return None;
            }
            ColormapIndex::from_raw((*style).Colormap)
        })
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
        let index = index.into().get();
        self.with_bound_context(|| unsafe {
            let style = sys::ImPlot3D_GetStyle();
            if !style.is_null() {
                let count = colormap_count_from_i32(
                    sys::ImPlot3D_GetColormapCount(),
                    "Plot3DContext::set_style_colormap()",
                );
                if count > 0 {
                    let idx = index.min(count - 1);
                    (*style).Colormap = ColormapIndex::from(idx).raw();
                }
            }
        })
    }

    /// Look up a colormap index by its name.
    #[inline]
    pub fn colormap_index_by_name(&self, name: &str) -> Option<ColormapIndex> {
        if name.contains('\0') {
            return None;
        }
        let index = self.with_bound_context(|| {
            dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe {
                sys::ImPlot3D_GetColormapIndex(ptr)
            })
        });
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
        self.with_bound_context(|| unsafe {
            let out = sys::ImPlot3D_GetColormapColor(index.raw(), (-1) as sys::ImPlot3DColormap);
            [out.x, out.y, out.z, out.w]
        })
    }
}

impl Plot3DUi<'_> {
    /// Return a color from this UI's active ImPlot3D colormap.
    pub fn colormap_color(&self, index: ColormapColorIndex) -> [f32; 4] {
        self.context.colormap_color(index)
    }
}

impl Plot3DScope<'_> {
    /// Return the next color from this plot's current colormap and advance its color cursor.
    pub fn next_colormap_color(&self) -> [f32; 4] {
        self.with_bound_context(|| unsafe {
            let out = sys::ImPlot3D_NextColormapColor();
            [out.x, out.y, out.z, out.w]
        })
    }
}
