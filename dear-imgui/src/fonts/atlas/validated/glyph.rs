use ttf_parser::FromData;

use crate::FontError;

use super::{
    MAX_COMPONENTS_PER_GLYPH, MAX_COMPOSITE_DEPTH, MAX_EXPANDED_GLYPH_COMPLEXITY, MaxpLimits, Table,
};

#[derive(Debug)]
pub(super) enum GlyphKind {
    Empty,
    Simple {
        points: usize,
        contours: usize,
        bounds: Option<CoordinateBounds>,
    },
    Composite {
        components: Vec<CompositeComponent>,
    },
}

#[derive(Debug)]
pub(super) struct CompositeComponent {
    pub(super) glyph_id: u16,
    pub(super) transform: CompositeTransform,
    pub(super) source_offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CompositeTransform {
    pub(super) xx: f32,
    pub(super) yx: f32,
    pub(super) xy: f32,
    pub(super) yy: f32,
    pub(super) dx: i16,
    pub(super) dy: i16,
}

impl Default for CompositeTransform {
    fn default() -> Self {
        Self {
            xx: 1.0,
            yx: 0.0,
            xy: 0.0,
            yy: 1.0,
            dx: 0,
            dy: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CoordinateBounds {
    pub(super) min_x: i16,
    pub(super) max_x: i16,
    pub(super) min_y: i16,
    pub(super) max_y: i16,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ExpandedGlyph {
    points: usize,
    contours: usize,
    references: usize,
    depth: usize,
    bounds: Option<CoordinateBounds>,
}

#[derive(Clone, Copy)]
struct Glyph<'a> {
    bytes: &'a [u8],
    offset: usize,
    id: u16,
}

impl<'a> Glyph<'a> {
    fn error(self, offset: usize, reason: &'static str) -> FontError {
        FontError::InvalidGlyph {
            glyph_id: self.id,
            offset: self.offset + offset,
            reason,
        }
    }

    fn range(self, offset: usize, len: usize, reason: &'static str) -> Result<&'a [u8], FontError> {
        self.bytes
            .get(offset..)
            .and_then(|tail| tail.get(..len))
            .ok_or_else(|| self.error(offset, reason))
    }

    fn read<T: FromData>(self, offset: usize) -> Result<T, FontError> {
        let reason = "glyph byte stream is truncated";
        T::parse(self.range(offset, T::SIZE, reason)?).ok_or_else(|| self.error(offset, reason))
    }

    fn skip_instructions(self, offset: usize, max_bytes: usize) -> Result<usize, FontError> {
        let length = usize::from(self.read::<u16>(offset)?);
        if length > max_bytes {
            return Err(self.error(
                offset,
                "instruction length exceeds maxp.maxSizeOfInstructions",
            ));
        }
        let start = offset + 2;
        self.range(start, length, "glyph instructions are truncated")?;
        Ok(start + length)
    }
}

pub(super) fn validate_glyphs(
    table: Table<'_>,
    locations: &[usize],
    limits: MaxpLimits,
) -> Result<(), FontError> {
    let glyph_count = usize::from(limits.glyph_count);
    let mut glyphs = Vec::with_capacity(glyph_count);
    for (index, pair) in locations.windows(2).enumerate() {
        let start = pair[0];
        let end = pair[1];
        let glyph = Glyph {
            bytes: &table.bytes[start..end],
            offset: table.offset + start,
            id: index as u16,
        };
        if start == end {
            glyphs.push(GlyphKind::Empty);
            continue;
        }
        glyph.range(0, 10, "nonempty glyph is shorter than its ten-byte header")?;
        if glyph.read::<i16>(2)? > glyph.read::<i16>(6)?
            || glyph.read::<i16>(4)? > glyph.read::<i16>(8)?
        {
            return Err(glyph.error(2, "glyph bounding box minimum exceeds its maximum"));
        }
        let contour_count = glyph.read::<i16>(0)?;
        let kind = match contour_count {
            1.. => parse_simple_glyph(glyph, contour_count as usize, limits)?,
            ..=-1 => parse_composite_glyph(glyph, limits)?,
            0 => GlyphKind::Empty,
        };
        glyphs.push(kind);
    }

    let mut states = vec![0_u8; glyph_count];
    let mut memo = vec![None; glyph_count];
    for glyph_index in 0..glyph_count {
        let expanded = expand_glyph(glyph_index, &glyphs, &mut states, &mut memo, 0)?;
        if matches!(glyphs[glyph_index], GlyphKind::Composite { .. }) {
            let reason = if expanded.points > limits.max_composite_points {
                "expanded point count exceeds maxp.maxCompositePoints"
            } else if expanded.contours > limits.max_composite_contours {
                "expanded contour count exceeds maxp.maxCompositeContours"
            } else if expanded.depth > limits.max_component_depth {
                "composite depth exceeds maxp.maxComponentDepth"
            } else {
                continue;
            };
            return Err(FontError::InvalidGlyph {
                glyph_id: glyph_index as u16,
                offset: table.offset + locations[glyph_index],
                reason,
            });
        }
    }
    Ok(())
}

fn parse_simple_glyph(
    glyph: Glyph<'_>,
    contour_count: usize,
    limits: MaxpLimits,
) -> Result<GlyphKind, FontError> {
    if contour_count > limits.max_contours {
        return Err(glyph.error(0, "contour count exceeds maxp.maxContours"));
    }
    let endpoints = glyph.range(
        10,
        contour_count * 2,
        "simple glyph contour endpoints are truncated",
    )?;
    let endpoints = endpoints.as_chunks::<2>().0;
    let mut point_count = 0;
    for (index, &endpoint) in endpoints.iter().enumerate() {
        let contour_end = usize::from(u16::from_be_bytes(endpoint)) + 1;
        if contour_end <= point_count {
            return Err(glyph.error(
                10 + index * 2,
                "simple glyph contour endpoints are not strictly increasing",
            ));
        }
        point_count = contour_end;
    }
    let instruction_offset = 10 + contour_count * 2;
    if point_count > limits.max_points {
        return Err(glyph.error(instruction_offset - 2, "point count exceeds maxp.maxPoints"));
    }

    let mut cursor = glyph.skip_instructions(instruction_offset, limits.max_instruction_bytes)?;
    let mut flags = Vec::with_capacity(point_count);
    while flags.len() < point_count {
        let flag = glyph.read::<u8>(cursor)?;
        cursor += 1;
        flags.push(flag);
        if flag & 0x08 != 0 {
            let repeats = usize::from(glyph.read::<u8>(cursor)?);
            if repeats > point_count - flags.len() {
                return Err(glyph.error(
                    cursor,
                    "simple glyph flag repeat exceeds the declared point count",
                ));
            }
            cursor += 1;
            flags.extend(std::iter::repeat_n(flag, repeats));
        }
    }

    let mut start = 0;
    for (index, &endpoint) in endpoints.iter().enumerate() {
        let end = usize::from(u16::from_be_bytes(endpoint)) + 1;
        // stb reads the next vertex when a contour starts off curve. It must belong to this
        // contour, and a final singleton would leave the native allocation entirely.
        if end == start + 1 && flags[start] & 1 == 0 {
            return Err(glyph.error(
                10 + index * 2,
                "a one-point contour must start on curve for stb_truetype",
            ));
        }
        start = end;
    }

    let (min_x, max_x) = validate_coordinate_stream(glyph, &mut cursor, &flags, 0x02, 0x10)?;
    let (min_y, max_y) = validate_coordinate_stream(glyph, &mut cursor, &flags, 0x04, 0x20)?;
    Ok(GlyphKind::Simple {
        points: point_count,
        contours: contour_count,
        bounds: Some(CoordinateBounds {
            min_x,
            max_x,
            min_y,
            max_y,
        }),
    })
}

fn validate_coordinate_stream(
    glyph: Glyph<'_>,
    cursor: &mut usize,
    flags: &[u8],
    short_mask: u8,
    same_mask: u8,
) -> Result<(i16, i16), FontError> {
    let mut coordinate = 0_i16;
    let mut min = i16::MAX;
    let mut max = i16::MIN;
    for &flag in flags {
        let delta = if flag & short_mask != 0 {
            let magnitude = i16::from(glyph.read::<u8>(*cursor)?);
            *cursor += 1;
            if flag & same_mask != 0 {
                magnitude
            } else {
                -magnitude
            }
        } else if flag & same_mask != 0 {
            0
        } else {
            let delta = glyph.read::<i16>(*cursor)?;
            *cursor += 2;
            delta
        };
        coordinate = i16::try_from(i32::from(coordinate) + i32::from(delta)).map_err(|_| {
            glyph.error(
                *cursor,
                "decoded coordinate is outside the signed 16-bit TrueType range",
            )
        })?;
        min = min.min(coordinate);
        max = max.max(coordinate);
    }
    Ok((min, max))
}

fn parse_composite_glyph(glyph: Glyph<'_>, limits: MaxpLimits) -> Result<GlyphKind, FontError> {
    const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
    const ARGS_ARE_XY_VALUES: u16 = 0x0002;
    const WE_HAVE_A_SCALE: u16 = 0x0008;
    const MORE_COMPONENTS: u16 = 0x0020;
    const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
    const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
    const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;
    const TRANSFORM_FLAGS: u16 = WE_HAVE_A_SCALE | WE_HAVE_AN_X_AND_Y_SCALE | WE_HAVE_A_TWO_BY_TWO;

    let mut cursor = 10;
    let mut components = Vec::new();
    let final_flags = loop {
        let component_offset = cursor;
        let flags = glyph.read::<u16>(cursor)?;
        let child = glyph.read::<u16>(cursor + 2)?;
        cursor += 4;
        if child >= limits.glyph_count {
            return Err(FontError::InvalidGlyphReference {
                glyph_id: glyph.id,
                referenced_glyph: child,
                glyph_count: limits.glyph_count,
            });
        }
        if flags & ARGS_ARE_XY_VALUES == 0 {
            return Err(glyph.error(
                component_offset,
                "stb_truetype does not support point-matching composite arguments",
            ));
        }
        if flags & WE_HAVE_INSTRUCTIONS != 0 && flags & MORE_COMPONENTS != 0 {
            return Err(glyph.error(
                component_offset,
                "WE_HAVE_INSTRUCTIONS may only appear on the final component",
            ));
        }

        let (dx, dy) = if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            let pair = (glyph.read::<i16>(cursor)?, glyph.read::<i16>(cursor + 2)?);
            cursor += 4;
            pair
        } else {
            let dx = i16::from(glyph.read::<i8>(cursor)?);
            let dy = i16::from(glyph.read::<i8>(cursor + 1)?);
            cursor += 2;
            (dx, dy)
        };
        let mut transform = CompositeTransform {
            dx,
            dy,
            ..CompositeTransform::default()
        };
        let mut read_scale = || -> Result<f32, FontError> {
            let value = glyph.read::<i16>(cursor)?;
            cursor += 2;
            Ok(f32::from(value) / 16_384.0)
        };
        match flags & TRANSFORM_FLAGS {
            0 => {}
            WE_HAVE_A_SCALE => {
                transform.xx = read_scale()?;
                transform.yy = transform.xx;
            }
            WE_HAVE_AN_X_AND_Y_SCALE => {
                transform.xx = read_scale()?;
                transform.yy = read_scale()?;
            }
            WE_HAVE_A_TWO_BY_TWO => {
                transform.xx = read_scale()?;
                transform.yx = read_scale()?;
                transform.xy = read_scale()?;
                transform.yy = read_scale()?;
            }
            _ => {
                return Err(glyph.error(
                    component_offset,
                    "composite component declares multiple transform encodings",
                ));
            }
        }
        components.push(CompositeComponent {
            glyph_id: child,
            transform,
            source_offset: glyph.offset + component_offset,
        });
        if components.len() > limits.max_component_elements.min(MAX_COMPONENTS_PER_GLYPH) {
            return Err(glyph.error(
                cursor,
                "component count exceeds maxp or the validated native-work limit",
            ));
        }
        if flags & MORE_COMPONENTS == 0 {
            break flags;
        }
    };

    if final_flags & WE_HAVE_INSTRUCTIONS != 0 {
        glyph.skip_instructions(cursor, limits.max_instruction_bytes)?;
    }
    Ok(GlyphKind::Composite { components })
}

impl CoordinateBounds {
    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            max_x: self.max_x.max(other.max_x),
            min_y: self.min_y.min(other.min_y),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn corners(self) -> [(f64, f64); 4] {
        [
            (f64::from(self.min_x), f64::from(self.min_y)),
            (f64::from(self.min_x), f64::from(self.max_y)),
            (f64::from(self.max_x), f64::from(self.min_y)),
            (f64::from(self.max_x), f64::from(self.max_y)),
        ]
    }
}

impl CompositeTransform {
    fn transform_bounds(self, bounds: CoordinateBounds) -> Option<CoordinateBounds> {
        if self.xx == 1.0 && self.yx == 0.0 && self.xy == 0.0 && self.yy == 1.0 {
            return checked_translated_bounds(bounds, self.dx, self.dy);
        }

        let x_scale = (self.xx * self.xx + self.yx * self.yx).sqrt();
        let y_scale = (self.xy * self.xy + self.yy * self.yy).sqrt();
        if !x_scale.is_finite() || !y_scale.is_finite() {
            return None;
        }

        let mut x_values = [0.0; 4];
        let mut y_values = [0.0; 4];
        for (index, (x, y)) in bounds.corners().into_iter().enumerate() {
            x_values[index] = f64::from(x_scale)
                * (f64::from(self.xx) * x + f64::from(self.xy) * y + f64::from(self.dx));
            y_values[index] = f64::from(y_scale)
                * (f64::from(self.yx) * x + f64::from(self.yy) * y + f64::from(self.dy));
        }

        let (min_x, max_x) = conservative_i16_bounds(x_values)?;
        let (min_y, max_y) = conservative_i16_bounds(y_values)?;
        Some(CoordinateBounds {
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }
}

fn checked_translated_bounds(
    bounds: CoordinateBounds,
    dx: i16,
    dy: i16,
) -> Option<CoordinateBounds> {
    let translate = |value: i16, offset: i16| -> Option<i16> {
        i32::from(value)
            .checked_add(i32::from(offset))
            .and_then(|value| i16::try_from(value).ok())
    };
    Some(CoordinateBounds {
        min_x: translate(bounds.min_x, dx)?,
        max_x: translate(bounds.max_x, dx)?,
        min_y: translate(bounds.min_y, dy)?,
        max_y: translate(bounds.max_y, dy)?,
    })
}

fn conservative_i16_bounds(values: [f64; 4]) -> Option<(i16, i16)> {
    const ROUNDING_MARGIN: f64 = 1.0;
    let min = values.into_iter().fold(f64::INFINITY, f64::min);
    let max = values.into_iter().fold(f64::NEG_INFINITY, f64::max);
    let min = min.floor() - ROUNDING_MARGIN;
    let max = max.ceil() + ROUNDING_MARGIN;
    if min < f64::from(i16::MIN) || max > f64::from(i16::MAX) {
        return None;
    }
    Some((min as i16, max as i16))
}

pub(super) fn expand_glyph(
    glyph_index: usize,
    glyphs: &[GlyphKind],
    states: &mut [u8],
    memo: &mut [Option<ExpandedGlyph>],
    recursion_depth: usize,
) -> Result<ExpandedGlyph, FontError> {
    if let Some(expanded) = memo[glyph_index] {
        return Ok(expanded);
    }
    if recursion_depth > MAX_COMPOSITE_DEPTH {
        return Err(FontError::CompositeDepth {
            glyph_id: glyph_index as u16,
            depth: recursion_depth,
            limit: MAX_COMPOSITE_DEPTH,
        });
    }
    if states[glyph_index] == 1 {
        return Err(FontError::CompositeCycle {
            glyph_id: glyph_index as u16,
            referenced_glyph: glyph_index as u16,
        });
    }
    states[glyph_index] = 1;

    let expanded = match &glyphs[glyph_index] {
        GlyphKind::Empty => ExpandedGlyph {
            references: 1,
            ..ExpandedGlyph::default()
        },
        GlyphKind::Simple {
            points,
            contours,
            bounds,
        } => ExpandedGlyph {
            points: *points,
            contours: *contours,
            references: 1,
            depth: 0,
            // stb also transforms the zeroed control coordinates of move and line vertices.
            bounds: bounds.map(|b| b.union(CoordinateBounds::default())),
        },
        GlyphKind::Composite { components } => {
            let mut expanded = ExpandedGlyph {
                references: 1,
                depth: 1,
                ..ExpandedGlyph::default()
            };
            for component in components {
                let component_index = usize::from(component.glyph_id);
                if states[component_index] == 1 {
                    return Err(FontError::CompositeCycle {
                        glyph_id: glyph_index as u16,
                        referenced_glyph: component.glyph_id,
                    });
                }
                let child =
                    expand_glyph(component_index, glyphs, states, memo, recursion_depth + 1)?;
                expanded.points =
                    checked_complexity_add(glyph_index, expanded.points, child.points)?;
                expanded.contours =
                    checked_complexity_add(glyph_index, expanded.contours, child.contours)?;
                expanded.references =
                    checked_complexity_add(glyph_index, expanded.references, child.references)?;
                expanded.depth = expanded.depth.max(child.depth + 1);
                if expanded.depth > MAX_COMPOSITE_DEPTH {
                    return Err(FontError::CompositeDepth {
                        glyph_id: glyph_index as u16,
                        depth: expanded.depth,
                        limit: MAX_COMPOSITE_DEPTH,
                    });
                }
                if let Some(child_bounds) = child.bounds {
                    let transformed = component
                        .transform
                        .transform_bounds(child_bounds)
                        .ok_or(FontError::InvalidGlyph {
                            glyph_id: glyph_index as u16,
                            offset: component.source_offset,
                            reason:
                                "composite transform produces coordinates outside the signed 16-bit range",
                        })?;
                    expanded.bounds = Some(match expanded.bounds {
                        Some(bounds) => bounds.union(transformed),
                        None => transformed,
                    });
                }
            }
            expanded
        }
    };

    states[glyph_index] = 2;
    memo[glyph_index] = Some(expanded);
    Ok(expanded)
}

fn checked_complexity_add(
    glyph_index: usize,
    left: usize,
    right: usize,
) -> Result<usize, FontError> {
    let complexity = left
        .checked_add(right)
        .ok_or(FontError::CompositeComplexity {
            glyph_id: glyph_index as u16,
            complexity: usize::MAX,
            limit: MAX_EXPANDED_GLYPH_COMPLEXITY,
        })?;
    if complexity > MAX_EXPANDED_GLYPH_COMPLEXITY {
        return Err(FontError::CompositeComplexity {
            glyph_id: glyph_index as u16,
            complexity,
            limit: MAX_EXPANDED_GLYPH_COMPLEXITY,
        });
    }
    Ok(complexity)
}
