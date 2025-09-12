//! OpenGL version detection and feature support

use glow::{Context, HasContext};

/// OpenGL version information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlVersion {
    pub major: u32,
    pub minor: u32,
    pub is_es: bool,
}

impl GlVersion {
    /// Read the OpenGL version from the current context
    pub fn read(gl: &Context) -> Self {
        let version_string = unsafe { gl.get_parameter_string(glow::VERSION) };
        Self::parse(&version_string)
    }

    /// Parse OpenGL version from version string
    pub fn parse(version_string: &str) -> Self {
        // Examples:
        // "4.6.0 NVIDIA 460.89"
        // "OpenGL ES 3.0 (OpenGL ES GLSL ES 3.00)"
        // "WebGL 2.0 (OpenGL ES 3.0 Chromium)"

        let is_es = version_string.contains("OpenGL ES") || version_string.contains("WebGL");

        // Extract version numbers
        let (major, minor) = if is_es {
            if version_string.contains("WebGL 2.0") || version_string.contains("OpenGL ES 3.") {
                (3, 0)
            } else if version_string.contains("WebGL 1.0")
                || version_string.contains("OpenGL ES 2.")
            {
                (2, 0)
            } else {
                // Try to parse manually
                Self::parse_version_numbers(version_string).unwrap_or((2, 0))
            }
        } else {
            Self::parse_version_numbers(version_string).unwrap_or((2, 1))
        };

        Self {
            major,
            minor,
            is_es,
        }
    }

    fn parse_version_numbers(version_string: &str) -> Option<(u32, u32)> {
        // Look for pattern like "3.2" or "4.6.0"
        for word in version_string.split_whitespace() {
            if let Some(dot_pos) = word.find('.') {
                let major_str = &word[..dot_pos];
                let rest = &word[dot_pos + 1..];

                if let Ok(major) = major_str.parse::<u32>() {
                    // Find the next dot or end of string for minor version
                    let minor_str = if let Some(next_dot) = rest.find('.') {
                        &rest[..next_dot]
                    } else {
                        rest
                    };

                    if let Ok(minor) = minor_str.parse::<u32>() {
                        return Some((major, minor));
                    }
                }
            }
        }
        None
    }

    /// Check if this version supports vertex array objects
    pub fn bind_vertex_array_support(self) -> bool {
        self.major >= 3 // OpenGL 3.0+ or OpenGL ES 3.0+
    }

    /// Check if this version supports glDrawElementsBaseVertex
    pub fn vertex_offset_support(self) -> bool {
        if self.is_es {
            false // Not supported in OpenGL ES
        } else {
            self.major > 3 || (self.major == 3 && self.minor >= 2) // OpenGL 3.2+
        }
    }

    /// Check if this version supports GL_CLIP_ORIGIN
    pub fn clip_origin_support(self) -> bool {
        if self.is_es {
            false // Not supported in OpenGL ES
        } else {
            self.major > 4 || (self.major == 4 && self.minor >= 5) // OpenGL 4.5+
        }
    }

    /// Check if this version supports glBindSampler
    pub fn bind_sampler_support(self) -> bool {
        if self.is_es {
            self.major >= 3 // OpenGL ES 3.0+
        } else {
            self.major > 3 || (self.major == 3 && self.minor >= 3) // OpenGL 3.3+
        }
    }

    /// Check if this version supports glPolygonMode
    pub fn polygon_mode_support(self) -> bool {
        !self.is_es // Not supported in OpenGL ES
    }

    /// Check if this version supports GL_PRIMITIVE_RESTART
    pub fn primitive_restart_support(self) -> bool {
        if self.is_es {
            self.major >= 3 // OpenGL ES 3.0+
        } else {
            self.major > 3 || (self.major == 3 && self.minor >= 1) // OpenGL 3.1+
        }
    }
}

/// GLSL version information
#[derive(Debug, Clone)]
pub struct GlslVersion {
    pub version_string: String,
}

impl GlslVersion {
    /// Get the appropriate GLSL version string for the given OpenGL version
    pub fn for_gl_version(gl_version: GlVersion) -> Self {
        let version_string = if gl_version.is_es {
            if gl_version.major >= 3 {
                "#version 300 es".to_string()
            } else {
                "#version 100".to_string()
            }
        } else {
            match (gl_version.major, gl_version.minor) {
                (4, minor) if minor >= 6 => "#version 460 core".to_string(),
                (4, minor) if minor >= 5 => "#version 450 core".to_string(),
                (4, minor) if minor >= 4 => "#version 440 core".to_string(),
                (4, minor) if minor >= 3 => "#version 430 core".to_string(),
                (4, minor) if minor >= 2 => "#version 420 core".to_string(),
                (4, minor) if minor >= 1 => "#version 410 core".to_string(),
                (4, 0) => "#version 400 core".to_string(),
                (3, minor) if minor >= 3 => "#version 330 core".to_string(),
                (3, 2) => "#version 150 core".to_string(),
                (3, 1) => "#version 140".to_string(),
                (3, 0) => "#version 130".to_string(),
                (2, 1) => "#version 120".to_string(),
                (2, 0) => "#version 110".to_string(),
                _ => "#version 130".to_string(), // Default fallback
            }
        };

        Self { version_string }
    }

    /// Get the version string
    pub fn as_str(&self) -> &str {
        &self.version_string
    }
}
