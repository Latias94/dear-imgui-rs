use crate::sys;

impl crate::ui::Ui {
    /// Get cursor position in screen coordinates.
    #[doc(alias = "GetCursorScreenPos")]
    pub fn get_cursor_screen_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorScreenPos() };
        [pos.x, pos.y]
    }

    /// Get available content region size.
    #[doc(alias = "GetContentRegionAvail")]
    pub fn get_content_region_avail(&self) -> [f32; 2] {
        let size = unsafe { sys::igGetContentRegionAvail() };
        [size.x, size.y]
    }

    /// Check if a point is inside a rectangle.
    pub fn is_point_in_rect(
        &self,
        point: [f32; 2],
        rect_min: [f32; 2],
        rect_max: [f32; 2],
    ) -> bool {
        point[0] >= rect_min[0]
            && point[0] <= rect_max[0]
            && point[1] >= rect_min[1]
            && point[1] <= rect_max[1]
    }

    /// Calculate distance between two points.
    pub fn distance(&self, p1: [f32; 2], p2: [f32; 2]) -> f32 {
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate squared distance between two points (faster than distance).
    pub fn distance_squared(&self, p1: [f32; 2], p2: [f32; 2]) -> f32 {
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        dx * dx + dy * dy
    }

    /// Check if two line segments intersect.
    pub fn line_segments_intersect(
        &self,
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        p4: [f32; 2],
    ) -> bool {
        let d1 = self.cross_product(
            [p4[0] - p3[0], p4[1] - p3[1]],
            [p1[0] - p3[0], p1[1] - p3[1]],
        );
        let d2 = self.cross_product(
            [p4[0] - p3[0], p4[1] - p3[1]],
            [p2[0] - p3[0], p2[1] - p3[1]],
        );
        let d3 = self.cross_product(
            [p2[0] - p1[0], p2[1] - p1[1]],
            [p3[0] - p1[0], p3[1] - p1[1]],
        );
        let d4 = self.cross_product(
            [p2[0] - p1[0], p2[1] - p1[1]],
            [p4[0] - p1[0], p4[1] - p1[1]],
        );

        (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
    }

    /// Calculate cross product of two 2D vectors.
    fn cross_product(&self, v1: [f32; 2], v2: [f32; 2]) -> f32 {
        v1[0] * v2[1] - v1[1] * v2[0]
    }

    /// Normalize a 2D vector.
    pub fn normalize(&self, v: [f32; 2]) -> [f32; 2] {
        let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
        if len > f32::EPSILON {
            [v[0] / len, v[1] / len]
        } else {
            [0.0, 0.0]
        }
    }

    /// Calculate dot product of two 2D vectors.
    pub fn dot_product(&self, v1: [f32; 2], v2: [f32; 2]) -> f32 {
        v1[0] * v2[0] + v1[1] * v2[1]
    }

    /// Calculate the angle between two vectors in radians.
    pub fn angle_between_vectors(&self, v1: [f32; 2], v2: [f32; 2]) -> f32 {
        let dot = self.dot_product(v1, v2);
        let len1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
        let len2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt();

        if len1 > f32::EPSILON && len2 > f32::EPSILON {
            (dot / (len1 * len2)).acos()
        } else {
            0.0
        }
    }

    /// Check if a point is inside a circle.
    pub fn is_point_in_circle(&self, point: [f32; 2], center: [f32; 2], radius: f32) -> bool {
        self.distance_squared(point, center) <= radius * radius
    }

    /// Calculate the area of a triangle given three points.
    pub fn triangle_area(&self, p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> f32 {
        let cross = self.cross_product(
            [p2[0] - p1[0], p2[1] - p1[1]],
            [p3[0] - p1[0], p3[1] - p1[1]],
        );
        cross.abs() * 0.5
    }
}
