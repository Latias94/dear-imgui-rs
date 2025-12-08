use crate::flags::Marker3D;
use crate::sys;

#[inline]
pub fn style_colors_dark() {
    unsafe { sys::ImPlot3D_StyleColorsDark(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_light() {
    unsafe { sys::ImPlot3D_StyleColorsLight(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_classic() {
    unsafe { sys::ImPlot3D_StyleColorsClassic(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_auto() {
    unsafe { sys::ImPlot3D_StyleColorsAuto(std::ptr::null_mut()) }
}

#[inline]
pub fn push_style_color(idx: i32, col: [f32; 4]) {
    unsafe {
        sys::ImPlot3D_PushStyleColor_Vec4(
            idx,
            sys::ImVec4_c {
                x: col[0],
                y: col[1],
                z: col[2],
                w: col[3],
            },
        );
    }
}
#[inline]
pub fn pop_style_color(count: i32) {
    unsafe { sys::ImPlot3D_PopStyleColor(count) }
}

/// Push a style variable (float variant)
#[inline]
pub fn push_style_var_f32(idx: i32, val: f32) {
    unsafe { sys::ImPlot3D_PushStyleVar_Float(idx, val) }
}

/// Push a style variable (int variant)
#[inline]
pub fn push_style_var_i32(idx: i32, val: i32) {
    unsafe { sys::ImPlot3D_PushStyleVar_Int(idx, val) }
}

/// Push a style variable (Vec2 variant)
#[inline]
pub fn push_style_var_vec2(idx: i32, val: [f32; 2]) {
    unsafe {
        sys::ImPlot3D_PushStyleVar_Vec2(
            idx,
            sys::ImVec2_c {
                x: val[0],
                y: val[1],
            },
        )
    }
}

/// Pop style variable(s)
#[inline]
pub fn pop_style_var(count: i32) {
    unsafe { sys::ImPlot3D_PopStyleVar(count) }
}

#[inline]
pub fn set_next_line_style(col: [f32; 4], weight: f32) {
    unsafe {
        sys::ImPlot3D_SetNextLineStyle(
            sys::ImVec4_c {
                x: col[0],
                y: col[1],
                z: col[2],
                w: col[3],
            },
            weight,
        )
    }
}

#[inline]
pub fn set_next_fill_style(col: [f32; 4], alpha_mod: f32) {
    unsafe {
        sys::ImPlot3D_SetNextFillStyle(
            sys::ImVec4_c {
                x: col[0],
                y: col[1],
                z: col[2],
                w: col[3],
            },
            alpha_mod,
        )
    }
}

#[inline]
pub fn set_next_marker_style(
    marker: Marker3D,
    size: f32,
    fill: [f32; 4],
    weight: f32,
    outline: [f32; 4],
) {
    unsafe {
        sys::ImPlot3D_SetNextMarkerStyle(
            marker as i32,
            size,
            sys::ImVec4_c {
                x: fill[0],
                y: fill[1],
                z: fill[2],
                w: fill[3],
            },
            weight,
            sys::ImVec4_c {
                x: outline[0],
                y: outline[1],
                z: outline[2],
                w: outline[3],
            },
        )
    }
}

#[inline]
pub fn push_colormap_index(cmap_index: i32) {
    unsafe { sys::ImPlot3D_PushColormap_Plot3DColormap(cmap_index) }
}
#[inline]
pub fn push_colormap_name(name: &str) {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { sys::ImPlot3D_PushColormap_Str(c.as_ptr()) }
}
#[inline]
pub fn pop_colormap(count: i32) {
    unsafe { sys::ImPlot3D_PopColormap(count) }
}
#[inline]
pub fn colormap_count() -> i32 {
    unsafe { sys::ImPlot3D_GetColormapCount() }
}
#[inline]
pub fn colormap_name(index: i32) -> String {
    unsafe {
        let p = sys::ImPlot3D_GetColormapName(index);
        if p.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// Get number of keys (colors) in a given colormap index
#[inline]
pub fn colormap_size(index: i32) -> i32 {
    unsafe { sys::ImPlot3D_GetColormapSize(index) }
}

/// Get current default colormap index set in ImPlot3D style
#[inline]
pub fn get_style_colormap_index() -> i32 {
    unsafe {
        let style = sys::ImPlot3D_GetStyle();
        if style.is_null() {
            return -1;
        }
        (*style).Colormap
    }
}

/// Get current default colormap name (if index valid)
#[inline]
pub fn get_style_colormap_name() -> Option<String> {
    let idx = get_style_colormap_index();
    if idx < 0 {
        return None;
    }
    let count = colormap_count();
    if idx >= count {
        return None;
    }
    Some(colormap_name(idx))
}

/// Permanently set the default colormap used by ImPlot3D (persists across plots/frames)
#[inline]
pub fn set_style_colormap_index(index: i32) {
    unsafe {
        let style = sys::ImPlot3D_GetStyle();
        if !style.is_null() {
            let count = sys::ImPlot3D_GetColormapCount();
            if count > 0 {
                let idx = if index < 0 {
                    0
                } else if index >= count {
                    count - 1
                } else {
                    index
                };
                (*style).Colormap = idx;
            }
        }
    }
}

/// Look up a colormap index by its name; returns -1 if not found
#[inline]
pub fn colormap_index_by_name(name: &str) -> i32 {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { sys::ImPlot3D_GetColormapIndex(c.as_ptr()) }
}

/// Convenience: set default colormap by name (no-op if name is invalid)
#[inline]
pub fn set_style_colormap_by_name(name: &str) {
    let idx = colormap_index_by_name(name);
    if idx >= 0 {
        set_style_colormap_index(idx);
    }
}

/// Get a color from the current colormap at index
pub fn get_colormap_color(idx: i32) -> [f32; 4] {
    unsafe {
        // Pass -1 for "current" colormap (upstream convention)
        let out = sys::ImPlot3D_GetColormapColor(idx, -1);
        [out.x, out.y, out.z, out.w]
    }
}

/// Get next colormap color (advances internal counter)
pub fn next_colormap_color() -> [f32; 4] {
    unsafe {
        let out = sys::ImPlot3D_NextColormapColor();
        [out.x, out.y, out.z, out.w]
    }
}
