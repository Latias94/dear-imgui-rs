//! Shared math conversions for Dear ImGui extensions.

/// A three-dimensional vector that can be read and updated as three floats.
pub trait Vec3Like {
    /// Reads the components in x, y, z order.
    fn to_array(&self) -> [f32; 3];
    /// Replaces the components in x, y, z order.
    fn set_from_array(&mut self, value: [f32; 3]);
}

impl Vec3Like for [f32; 3] {
    fn to_array(&self) -> [f32; 3] {
        *self
    }

    fn set_from_array(&mut self, value: [f32; 3]) {
        *self = value;
    }
}

impl Vec3Like for (f32, f32, f32) {
    fn to_array(&self) -> [f32; 3] {
        [self.0, self.1, self.2]
    }

    fn set_from_array(&mut self, value: [f32; 3]) {
        *self = (value[0], value[1], value[2]);
    }
}

impl Vec3Like for mint::Vector3<f32> {
    fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    fn set_from_array(&mut self, value: [f32; 3]) {
        *self = value.into();
    }
}

#[cfg(feature = "glam")]
impl Vec3Like for glam::Vec3 {
    fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    fn set_from_array(&mut self, value: [f32; 3]) {
        *self = Self::from_array(value);
    }
}
