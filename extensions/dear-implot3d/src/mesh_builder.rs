use std::borrow::Cow;

use crate::item_style::{Plot3DItemStyle, plot3d_spec_with_style};
use crate::{
    Item3DFlags, Mesh3DFlags, Plot3DDataLayout, Plot3DUi, debug_before_plot, len_i32, sys,
};

/// Mesh plot builder
pub struct Mesh3DBuilder<'ui> {
    pub(crate) _ui: &'ui Plot3DUi<'ui>,
    pub(crate) label: Cow<'ui, str>,
    pub(crate) vertices: &'ui [[f32; 3]],
    pub(crate) indices: &'ui [u32],
    pub(crate) flags: Mesh3DFlags,
    pub(crate) item_flags: Item3DFlags,
    pub(crate) style: Plot3DItemStyle,
}

impl<'ui> Mesh3DBuilder<'ui> {
    pub fn flags(mut self, flags: Mesh3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn plot(self) {
        self._ui.bind();
        let Some(vtx_count) = len_i32(self.vertices.len()) else {
            return;
        };
        let Some(idx_count) = len_i32(self.indices.len()) else {
            return;
        };
        let mut xs = Vec::with_capacity(self.vertices.len());
        let mut ys = Vec::with_capacity(self.vertices.len());
        let mut zs = Vec::with_capacity(self.vertices.len());
        for [x, y, z] in self.vertices.iter().copied() {
            xs.push(x);
            ys.push(y);
            zs.push(z);
        }

        let label = self.label.as_ref();
        let label = if label.contains('\0') { "mesh" } else { label };
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            debug_before_plot();
            let spec = plot3d_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                Plot3DDataLayout::DEFAULT,
            );
            sys::ImPlot3D_PlotMesh_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                self.indices.as_ptr(),
                vtx_count,
                idx_count,
                spec,
            );
        })
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Start a mesh plot from vertices (x,y,z) and triangle indices
    pub fn mesh(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        vertices: &'ui [[f32; 3]],
        indices: &'ui [u32],
    ) -> Mesh3DBuilder<'ui> {
        self.bind();
        Mesh3DBuilder {
            _ui: self,
            label: label.into(),
            vertices,
            indices,
            flags: Mesh3DFlags::NONE,
            item_flags: Item3DFlags::NONE,
            style: Plot3DItemStyle::default(),
        }
    }
}
