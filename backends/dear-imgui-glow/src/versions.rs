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

        let is_webgl = version_string.contains("WebGL");
        let is_es = version_string.contains("OpenGL ES") || is_webgl;

        // Extract version numbers
        let (major, minor) = if is_webgl {
            if version_string.contains("WebGL 2.0") {
                (3, 0)
            } else if version_string.contains("WebGL 1.0") {
                (2, 0)
            } else {
                Self::parse_version_numbers(version_string).unwrap_or((2, 0))
            }
        } else {
            Self::parse_version_numbers(version_string).unwrap_or(if is_es {
                (2, 0)
            } else {
                (2, 1)
            })
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

    /// Returns whether this context meets the backend's state-restoration contract.
    pub fn is_supported(self) -> bool {
        self.major >= 3
    }

    /// Returns whether this version supports `glDrawElementsBaseVertex`.
    pub fn supports_vertex_offset(self) -> bool {
        if self.is_es {
            false // Not supported in OpenGL ES
        } else {
            self.major > 3 || (self.major == 3 && self.minor >= 2) // OpenGL 3.2+
        }
    }

    /// Returns whether this version supports `GL_CLIP_ORIGIN` in core.
    pub fn supports_clip_origin(self) -> bool {
        if self.is_es {
            false // Not supported in OpenGL ES
        } else {
            self.major > 4 || (self.major == 4 && self.minor >= 5) // OpenGL 4.5+
        }
    }

    /// Returns whether the context supports independent sampler objects.
    pub fn supports_sampler_objects(self) -> bool {
        if self.is_es {
            self.major >= 3 // OpenGL ES 3.0+
        } else {
            self.major > 3 || (self.major == 3 && self.minor >= 3) // OpenGL 3.3+
        }
    }

    /// Returns whether this version supports `glPolygonMode`.
    pub fn supports_polygon_mode(self) -> bool {
        !self.is_es // Not supported in OpenGL ES
    }

    /// Returns whether this version uses desktop `GL_PRIMITIVE_RESTART` state.
    pub fn supports_primitive_restart(self) -> bool {
        !self.is_es && (self.major > 3 || (self.major == 3 && self.minor >= 1))
    }

    /// Returns whether front and back polygon modes must be restored independently.
    pub(crate) fn uses_separate_polygon_modes(self, compatibility_profile: bool) -> bool {
        self.supports_polygon_mode()
            && ((self.major == 3 && self.minor <= 1) || compatibility_profile)
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

#[cfg(test)]
mod tests {
    use super::GlVersion;

    #[test]
    fn supported_contexts_have_vertex_array_objects() {
        for version in [
            GlVersion {
                major: 3,
                minor: 0,
                is_es: false,
            },
            GlVersion {
                major: 3,
                minor: 0,
                is_es: true,
            },
            GlVersion::parse("WebGL 2.0 (OpenGL ES 3.0 Chromium)"),
        ] {
            assert!(version.is_supported());
        }

        for version in [
            GlVersion {
                major: 2,
                minor: 1,
                is_es: false,
            },
            GlVersion {
                major: 2,
                minor: 0,
                is_es: true,
            },
            GlVersion::parse("WebGL 1.0 (OpenGL ES 2.0 Chromium)"),
        ] {
            assert!(!version.is_supported());
        }
    }

    #[test]
    fn sampler_objects_follow_the_live_api_version() {
        let desktop_32 = GlVersion {
            major: 3,
            minor: 2,
            is_es: false,
        };
        let desktop_33 = GlVersion {
            major: 3,
            minor: 3,
            is_es: false,
        };
        let es_30 = GlVersion {
            major: 3,
            minor: 0,
            is_es: true,
        };

        assert!(!desktop_32.supports_sampler_objects());
        assert!(desktop_33.supports_sampler_objects());
        assert!(es_30.supports_sampler_objects());
    }

    #[test]
    fn parses_real_es_minor_versions_without_using_embedded_webgl_versions() {
        assert_eq!(
            GlVersion::parse("OpenGL ES 3.1 Mesa 25.1"),
            GlVersion {
                major: 3,
                minor: 1,
                is_es: true,
            }
        );
        assert_eq!(
            GlVersion::parse("OpenGL ES 3.2 NVIDIA 610.47"),
            GlVersion {
                major: 3,
                minor: 2,
                is_es: true,
            }
        );
        assert_eq!(
            GlVersion::parse("WebGL 2.0 (OpenGL ES 3.2 Chromium)"),
            GlVersion {
                major: 3,
                minor: 0,
                is_es: true,
            }
        );
    }

    #[test]
    fn vertex_offset_and_primitive_restart_are_not_claimed_on_es() {
        let es_32 = GlVersion {
            major: 3,
            minor: 2,
            is_es: true,
        };
        assert!(!es_32.supports_vertex_offset());
        assert!(!es_32.supports_primitive_restart());
    }

    #[test]
    fn polygon_restore_matches_desktop_profile_rules() {
        let desktop_31 = GlVersion {
            major: 3,
            minor: 1,
            is_es: false,
        };
        let desktop_32 = GlVersion {
            major: 3,
            minor: 2,
            is_es: false,
        };
        assert!(desktop_31.uses_separate_polygon_modes(false));
        assert!(!desktop_32.uses_separate_polygon_modes(false));
        assert!(desktop_32.uses_separate_polygon_modes(true));
    }
}
