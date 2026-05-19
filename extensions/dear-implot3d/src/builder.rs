use std::marker::PhantomData;

use crate::ui::{Plot3DContextBinding, Plot3DToken};
use crate::{Plot3DFlags, debug_begin_plot, imvec2, sys};
use dear_imgui_rs::Ui;

/// Plot builder for configuring the 3D plot
pub struct Plot3DBuilder<'ui> {
    pub(crate) binding: Plot3DContextBinding,
    pub(crate) imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    pub(crate) title: String,
    pub(crate) size: Option<[f32; 2]>,
    pub(crate) flags: Plot3DFlags,
    pub(crate) _lifetime: PhantomData<&'ui Ui>,
}

impl<'ui> Plot3DBuilder<'ui> {
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = Some(size);
        self
    }
    pub fn flags(mut self, flags: Plot3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn build(self) -> Option<Plot3DToken<'ui>> {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
        self.binding.bind();
        if self.title.contains('\0') {
            return None;
        }
        let title = self.title;
        let size = self.size.unwrap_or([0.0, 0.0]);
        let ok = dear_imgui_rs::with_scratch_txt(&title, |title_ptr| unsafe {
            // Defensive: ensure style.Colormap is in range before plotting
            let style = sys::ImPlot3D_GetStyle();
            if !style.is_null() {
                let count = sys::ImPlot3D_GetColormapCount();
                if count > 0 && ((*style).Colormap < 0 || (*style).Colormap >= count) {
                    (*style).Colormap = 0;
                }
            }
            sys::ImPlot3D_BeginPlot(
                title_ptr,
                imvec2(size[0], size[1]),
                self.flags.bits() as i32,
            )
        });
        if ok {
            debug_begin_plot();
            Some(Plot3DToken {
                binding: self.binding,
                imgui_alive: self.imgui_alive.clone(),
                _lifetime: PhantomData,
            })
        } else {
            None
        }
    }
}
