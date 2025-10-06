use super::{Plot3D, Plot3DError, validate_nonempty};
use crate::{Mesh3DFlags, Plot3DUi};

pub struct Mesh3D<'a> {
    pub label: &'a str,
    pub vertices: &'a [[f32; 3]],
    pub indices: &'a [u32],
    pub flags: Mesh3DFlags,
}

impl<'a> Mesh3D<'a> {
    pub fn new(label: &'a str, vertices: &'a [[f32; 3]], indices: &'a [u32]) -> Self {
        Self {
            label,
            vertices,
            indices,
            flags: Mesh3DFlags::NONE,
        }
    }
    pub fn flags(mut self, flags: Mesh3DFlags) -> Self {
        self.flags = flags;
        self
    }
}

impl<'a> Plot3D for Mesh3D<'a> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        validate_nonempty(self.vertices)?;
        validate_nonempty(self.indices)?;
        ui.mesh(self.label, self.vertices, self.indices)
            .flags(self.flags)
            .plot();
        Ok(())
    }
}
