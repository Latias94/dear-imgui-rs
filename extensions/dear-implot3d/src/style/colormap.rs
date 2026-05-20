use crate::sys;

use super::tokens::ColormapToken;
use super::types::{ColormapColorIndex, ColormapIndex};
use std::marker::PhantomData;

#[inline]
pub fn push_colormap(cmap: impl Into<ColormapIndex>) -> ColormapToken {
    unsafe { sys::ImPlot3D_PushColormap_Plot3DColormap(cmap.into().raw()) }
    ColormapToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

#[inline]
pub fn push_colormap_name(name: &str) -> ColormapToken {
    dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe { sys::ImPlot3D_PushColormap_Str(ptr) });
    ColormapToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}
#[inline]
pub fn colormap_count() -> usize {
    colormap_count_from_i32(
        unsafe { sys::ImPlot3D_GetColormapCount() },
        "colormap_count()",
    )
}

pub(super) fn colormap_count_from_i32(raw: i32, caller: &str) -> usize {
    assert!(raw >= 0, "{caller} returned a negative colormap count");
    usize::try_from(raw).expect("non-negative colormap count must fit usize")
}

#[inline]
pub fn colormap_name(index: impl Into<ColormapIndex>) -> String {
    unsafe {
        let p = sys::ImPlot3D_GetColormapName(index.into().raw());
        if p.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// Get number of keys (colors) in a given colormap index
#[inline]
pub fn colormap_size(index: impl Into<ColormapIndex>) -> usize {
    colormap_count_from_i32(
        unsafe { sys::ImPlot3D_GetColormapSize(index.into().raw()) },
        "colormap_size()",
    )
}

/// Get current default colormap index set in ImPlot3D style
#[inline]
pub fn get_style_colormap_index() -> Option<ColormapIndex> {
    unsafe {
        let style = sys::ImPlot3D_GetStyle();
        if style.is_null() {
            return None;
        }
        ColormapIndex::from_raw((*style).Colormap)
    }
}

/// Get current default colormap name (if index valid)
#[inline]
pub fn get_style_colormap_name() -> Option<String> {
    let idx = get_style_colormap_index()?;
    let count = colormap_count();
    if idx.get() >= count {
        return None;
    }
    Some(colormap_name(idx))
}

/// Permanently set the default colormap used by ImPlot3D (persists across plots/frames)
#[inline]
pub fn set_style_colormap(index: impl Into<ColormapIndex>) {
    unsafe {
        let style = sys::ImPlot3D_GetStyle();
        if !style.is_null() {
            let count = colormap_count();
            if count > 0 {
                let index = index.into().get();
                let idx = index.min(count - 1);
                (*style).Colormap = ColormapIndex::from(idx).raw();
            }
        }
    }
}

/// Look up a colormap index by its name.
#[inline]
pub fn colormap_index_by_name(name: &str) -> Option<ColormapIndex> {
    if name.contains('\0') {
        return None;
    }
    let index =
        dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe { sys::ImPlot3D_GetColormapIndex(ptr) });
    ColormapIndex::from_raw(index)
}

/// Convenience: set default colormap by name (no-op if name is invalid)
#[inline]
pub fn set_style_colormap_by_name(name: &str) {
    if let Some(idx) = colormap_index_by_name(name) {
        set_style_colormap(idx);
    }
}

/// Get a color from the current colormap at index
pub fn get_colormap_color(index: ColormapColorIndex) -> [f32; 4] {
    unsafe {
        // Pass -1 for "current" colormap (upstream convention)
        let out = crate::compat_ffi::ImPlot3D_GetColormapColor(
            index.raw(),
            (-1) as sys::ImPlot3DColormap,
        );
        [out.x, out.y, out.z, out.w]
    }
}

/// Get next colormap color (advances internal counter)
pub fn next_colormap_color() -> [f32; 4] {
    unsafe {
        let out = crate::compat_ffi::ImPlot3D_NextColormapColor();
        [out.x, out.y, out.z, out.w]
    }
}
