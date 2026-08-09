use super::{
    MAX_COMPONENTS_PER_GLYPH, MAX_COMPOSITE_DEPTH, MAX_EXPANDED_GLYPH_COMPLEXITY, MaxpLimits,
    StbTrueTypeFontError, Table,
};

#[derive(Clone, Debug)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
struct AxisBounds {
    min: i16,
    max: i16,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CoordinateBounds {
    pub(super) min_x: i16,
    pub(super) max_x: i16,
    pub(super) min_y: i16,
    pub(super) max_y: i16,
}

#[derive(Clone, Copy, Debug)]
struct CoordinateStream {
    cursor: usize,
    bounds: Option<AxisBounds>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ExpandedGlyph {
    points: usize,
    contours: usize,
    references: usize,
    depth: usize,
    bounds: Option<CoordinateBounds>,
}

pub(super) fn validate_glyphs(
    data: &[u8],
    table: Table,
    locations: &[usize],
    limits: MaxpLimits,
) -> Result<(), StbTrueTypeFontError> {
    let glyf = table.bytes(data);
    let glyph_count = usize::from(limits.glyph_count);
    let mut glyphs = Vec::with_capacity(glyph_count);

    for glyph_index in 0..glyph_count {
        let start = locations[glyph_index];
        let end = locations[glyph_index + 1];
        let glyph_id = glyph_index as u16;
        if start == end {
            glyphs.push(GlyphKind::Empty);
            continue;
        }
        if end - start < 10 {
            return Err(invalid_glyph(
                table,
                glyph_id,
                start,
                "nonempty glyph is shorter than its ten-byte header",
            ));
        }
        let bytes = &glyf[start..end];
        let contour_count = glyph_i16(bytes, table, glyph_id, start, 0)?;
        let x_min = glyph_i16(bytes, table, glyph_id, start, 2)?;
        let y_min = glyph_i16(bytes, table, glyph_id, start, 4)?;
        let x_max = glyph_i16(bytes, table, glyph_id, start, 6)?;
        let y_max = glyph_i16(bytes, table, glyph_id, start, 8)?;
        if x_min > x_max || y_min > y_max {
            return Err(invalid_glyph(
                table,
                glyph_id,
                start + 2,
                "glyph bounding box minimum exceeds its maximum",
            ));
        }

        let kind = if contour_count > 0 {
            parse_simple_glyph(
                bytes,
                table,
                glyph_id,
                start,
                contour_count as usize,
                limits,
            )?
        } else if contour_count < 0 {
            parse_composite_glyph(bytes, table, glyph_id, start, limits)?
        } else {
            GlyphKind::Empty
        };
        glyphs.push(kind);
    }

    let mut states = vec![0_u8; glyph_count];
    let mut memo = vec![None; glyph_count];
    for glyph_index in 0..glyph_count {
        let expanded = expand_glyph(glyph_index, &glyphs, &mut states, &mut memo, 0)?;
        if matches!(glyphs[glyph_index], GlyphKind::Composite { .. }) {
            if expanded.points > limits.max_composite_points {
                return Err(invalid_glyph(
                    table,
                    glyph_index as u16,
                    locations[glyph_index],
                    "expanded point count exceeds maxp.maxCompositePoints",
                ));
            }
            if expanded.contours > limits.max_composite_contours {
                return Err(invalid_glyph(
                    table,
                    glyph_index as u16,
                    locations[glyph_index],
                    "expanded contour count exceeds maxp.maxCompositeContours",
                ));
            }
            if expanded.depth > limits.max_component_depth {
                return Err(invalid_glyph(
                    table,
                    glyph_index as u16,
                    locations[glyph_index],
                    "composite depth exceeds maxp.maxComponentDepth",
                ));
            }
        }
    }

    Ok(())
}

fn parse_simple_glyph(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    contour_count: usize,
    limits: MaxpLimits,
) -> Result<GlyphKind, StbTrueTypeFontError> {
    if contour_count > limits.max_contours {
        return Err(invalid_glyph(
            table,
            glyph_id,
            glyph_offset,
            "contour count exceeds maxp.maxContours",
        ));
    }
    let endpoints_end = contour_count
        .checked_mul(2)
        .and_then(|length| length.checked_add(10))
        .ok_or_else(|| {
            invalid_glyph(
                table,
                glyph_id,
                glyph_offset,
                "contour endpoint array overflows usize",
            )
        })?;
    let instruction_length_offset = endpoints_end;
    let flags_start = instruction_length_offset.checked_add(2).ok_or_else(|| {
        invalid_glyph(
            table,
            glyph_id,
            glyph_offset,
            "instruction length offset overflows usize",
        )
    })?;
    require_glyph_range(
        bytes,
        table,
        glyph_id,
        glyph_offset,
        instruction_length_offset,
        2,
        "simple glyph contour endpoints are truncated",
    )?;

    let mut previous_endpoint = None;
    let mut point_count = 0;
    for contour in 0..contour_count {
        let endpoint = usize::from(glyph_u16(
            bytes,
            table,
            glyph_id,
            glyph_offset,
            10 + contour * 2,
        )?);
        if previous_endpoint.is_some_and(|previous| endpoint <= previous) {
            return Err(invalid_glyph(
                table,
                glyph_id,
                glyph_offset + 10 + contour * 2,
                "simple glyph contour endpoints are not strictly increasing",
            ));
        }
        previous_endpoint = Some(endpoint);
        point_count = endpoint + 1;
    }
    if point_count > limits.max_points {
        return Err(invalid_glyph(
            table,
            glyph_id,
            glyph_offset + endpoints_end - 2,
            "point count exceeds maxp.maxPoints",
        ));
    }

    let instruction_length = usize::from(glyph_u16(
        bytes,
        table,
        glyph_id,
        glyph_offset,
        instruction_length_offset,
    )?);
    if instruction_length > limits.max_instruction_bytes {
        return Err(invalid_glyph(
            table,
            glyph_id,
            glyph_offset + instruction_length_offset,
            "instruction length exceeds maxp.maxSizeOfInstructions",
        ));
    }
    let mut cursor = flags_start.checked_add(instruction_length).ok_or_else(|| {
        invalid_glyph(
            table,
            glyph_id,
            glyph_offset + flags_start,
            "instruction range overflows usize",
        )
    })?;
    require_glyph_range(
        bytes,
        table,
        glyph_id,
        glyph_offset,
        flags_start,
        instruction_length,
        "simple glyph instructions are truncated",
    )?;

    let mut flags = Vec::with_capacity(point_count);
    while flags.len() < point_count {
        let flag = glyph_u8(bytes, table, glyph_id, glyph_offset, cursor)?;
        cursor += 1;
        flags.push(flag);
        if flag & 0x08 != 0 {
            let repeats = usize::from(glyph_u8(bytes, table, glyph_id, glyph_offset, cursor)?);
            cursor += 1;
            if repeats > point_count - flags.len() {
                return Err(invalid_glyph(
                    table,
                    glyph_id,
                    glyph_offset + cursor - 1,
                    "simple glyph flag repeat exceeds the declared point count",
                ));
            }
            flags.extend(std::iter::repeat_n(flag, repeats));
        }
    }

    let x_coordinates = validate_coordinate_stream(
        bytes,
        table,
        glyph_id,
        glyph_offset,
        cursor,
        &flags,
        0x02,
        0x10,
        "x-coordinate stream is truncated",
    )?;
    let y_coordinates = validate_coordinate_stream(
        bytes,
        table,
        glyph_id,
        glyph_offset,
        x_coordinates.cursor,
        &flags,
        0x04,
        0x20,
        "y-coordinate stream is truncated",
    )?;
    let bounds = match (x_coordinates.bounds, y_coordinates.bounds) {
        (Some(x), Some(y)) => Some(CoordinateBounds {
            min_x: x.min,
            max_x: x.max,
            min_y: y.min,
            max_y: y.max,
        }),
        (None, None) => None,
        _ => unreachable!("x and y coordinate streams must describe the same points"),
    };

    Ok(GlyphKind::Simple {
        points: point_count,
        contours: contour_count,
        bounds,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_coordinate_stream(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    mut cursor: usize,
    flags: &[u8],
    short_mask: u8,
    same_mask: u8,
    truncated_reason: &'static str,
) -> Result<CoordinateStream, StbTrueTypeFontError> {
    let mut coordinate = 0_i32;
    let mut min = i16::MAX;
    let mut max = i16::MIN;
    for &flag in flags {
        let delta = if flag & short_mask != 0 {
            let magnitude = i32::from(glyph_u8(bytes, table, glyph_id, glyph_offset, cursor)?);
            cursor += 1;
            if flag & same_mask != 0 {
                magnitude
            } else {
                -magnitude
            }
        } else if flag & same_mask != 0 {
            0
        } else {
            require_glyph_range(
                bytes,
                table,
                glyph_id,
                glyph_offset,
                cursor,
                2,
                truncated_reason,
            )?;
            let delta = i32::from(i16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]));
            cursor += 2;
            delta
        };
        coordinate = coordinate.checked_add(delta).ok_or_else(|| {
            invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor,
                "coordinate accumulation overflowed i32",
            )
        })?;
        let coordinate = i16::try_from(coordinate).map_err(|_| {
            invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor,
                "decoded coordinate is outside the signed 16-bit TrueType range",
            )
        })?;
        min = min.min(coordinate);
        max = max.max(coordinate);
    }
    Ok(CoordinateStream {
        cursor,
        bounds: (!flags.is_empty()).then_some(AxisBounds { min, max }),
    })
}

fn parse_composite_glyph(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    limits: MaxpLimits,
) -> Result<GlyphKind, StbTrueTypeFontError> {
    const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
    const ARGS_ARE_XY_VALUES: u16 = 0x0002;
    const WE_HAVE_A_SCALE: u16 = 0x0008;
    const MORE_COMPONENTS: u16 = 0x0020;
    const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
    const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
    const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;

    let mut cursor = 10;
    let mut components = Vec::new();
    let final_flags = loop {
        let component_source_offset = table
            .offset
            .saturating_add(glyph_offset)
            .saturating_add(cursor);
        require_glyph_range(
            bytes,
            table,
            glyph_id,
            glyph_offset,
            cursor,
            4,
            "composite component header is truncated",
        )?;
        let flags = glyph_u16(bytes, table, glyph_id, glyph_offset, cursor)?;
        let component = glyph_u16(bytes, table, glyph_id, glyph_offset, cursor + 2)?;
        cursor += 4;
        if component >= limits.glyph_count {
            return Err(StbTrueTypeFontError::InvalidGlyphReference {
                glyph_id,
                referenced_glyph: component,
                glyph_count: limits.glyph_count,
            });
        }
        if flags & ARGS_ARE_XY_VALUES == 0 {
            return Err(invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor - 4,
                "stb_truetype does not support point-matching composite arguments",
            ));
        }
        if flags & WE_HAVE_INSTRUCTIONS != 0 && flags & MORE_COMPONENTS != 0 {
            return Err(invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor - 4,
                "WE_HAVE_INSTRUCTIONS may only appear on the final component",
            ));
        }

        let arguments_are_words = flags & ARG_1_AND_2_ARE_WORDS != 0;
        let argument_bytes = if arguments_are_words { 4 } else { 2 };
        require_glyph_range(
            bytes,
            table,
            glyph_id,
            glyph_offset,
            cursor,
            argument_bytes,
            "composite component arguments are truncated",
        )?;
        let (dx, dy) = if arguments_are_words {
            (
                glyph_i16(bytes, table, glyph_id, glyph_offset, cursor)?,
                glyph_i16(bytes, table, glyph_id, glyph_offset, cursor + 2)?,
            )
        } else {
            (
                i16::from(i8::from_be_bytes([glyph_u8(
                    bytes,
                    table,
                    glyph_id,
                    glyph_offset,
                    cursor,
                )?])),
                i16::from(i8::from_be_bytes([glyph_u8(
                    bytes,
                    table,
                    glyph_id,
                    glyph_offset,
                    cursor + 1,
                )?])),
            )
        };
        cursor += argument_bytes;

        let transform_count = usize::from(flags & WE_HAVE_A_SCALE != 0)
            + usize::from(flags & WE_HAVE_AN_X_AND_Y_SCALE != 0)
            + usize::from(flags & WE_HAVE_A_TWO_BY_TWO != 0);
        if transform_count > 1 {
            return Err(invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor - argument_bytes - 4,
                "composite component declares multiple transform encodings",
            ));
        }
        let mut transform = CompositeTransform {
            dx,
            dy,
            ..CompositeTransform::default()
        };
        let transform_bytes = if flags & WE_HAVE_A_SCALE != 0 {
            let scale =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor)?) / 16_384.0;
            transform.xx = scale;
            transform.yy = scale;
            2
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            transform.xx =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor)?) / 16_384.0;
            transform.yy =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor + 2)?) / 16_384.0;
            4
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            transform.xx =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor)?) / 16_384.0;
            transform.yx =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor + 2)?) / 16_384.0;
            transform.xy =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor + 4)?) / 16_384.0;
            transform.yy =
                f32::from(glyph_i16(bytes, table, glyph_id, glyph_offset, cursor + 6)?) / 16_384.0;
            8
        } else {
            0
        };
        require_glyph_range(
            bytes,
            table,
            glyph_id,
            glyph_offset,
            cursor,
            transform_bytes,
            "composite transform is truncated",
        )?;
        cursor += transform_bytes;

        components.push(CompositeComponent {
            glyph_id: component,
            transform,
            source_offset: component_source_offset,
        });
        if components.len() > limits.max_component_elements
            || components.len() > MAX_COMPONENTS_PER_GLYPH
        {
            return Err(invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor,
                "component count exceeds maxp or the validated native-work limit",
            ));
        }
        if flags & MORE_COMPONENTS == 0 {
            break flags;
        }
    };

    if final_flags & WE_HAVE_INSTRUCTIONS != 0 {
        let instruction_length =
            usize::from(glyph_u16(bytes, table, glyph_id, glyph_offset, cursor)?);
        if instruction_length > limits.max_instruction_bytes {
            return Err(invalid_glyph(
                table,
                glyph_id,
                glyph_offset + cursor,
                "instruction length exceeds maxp.maxSizeOfInstructions",
            ));
        }
        cursor += 2;
        require_glyph_range(
            bytes,
            table,
            glyph_id,
            glyph_offset,
            cursor,
            instruction_length,
            "composite instructions are truncated",
        )?;
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
) -> Result<ExpandedGlyph, StbTrueTypeFontError> {
    if let Some(expanded) = memo[glyph_index] {
        return Ok(expanded);
    }
    if recursion_depth > MAX_COMPOSITE_DEPTH {
        return Err(StbTrueTypeFontError::CompositeDepth {
            glyph_id: glyph_index as u16,
            depth: recursion_depth,
            limit: MAX_COMPOSITE_DEPTH,
        });
    }
    if states[glyph_index] == 1 {
        return Err(StbTrueTypeFontError::CompositeCycle {
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
            bounds: *bounds,
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
                    return Err(StbTrueTypeFontError::CompositeCycle {
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
                    return Err(StbTrueTypeFontError::CompositeDepth {
                        glyph_id: glyph_index as u16,
                        depth: expanded.depth,
                        limit: MAX_COMPOSITE_DEPTH,
                    });
                }
                if let Some(child_bounds) = child.bounds {
                    let transformed = component
                        .transform
                        .transform_bounds(child_bounds)
                        .ok_or_else(|| StbTrueTypeFontError::InvalidGlyph {
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
) -> Result<usize, StbTrueTypeFontError> {
    let complexity = left
        .checked_add(right)
        .ok_or(StbTrueTypeFontError::CompositeComplexity {
            glyph_id: glyph_index as u16,
            complexity: usize::MAX,
            limit: MAX_EXPANDED_GLYPH_COMPLEXITY,
        })?;
    if complexity > MAX_EXPANDED_GLYPH_COMPLEXITY {
        return Err(StbTrueTypeFontError::CompositeComplexity {
            glyph_id: glyph_index as u16,
            complexity,
            limit: MAX_EXPANDED_GLYPH_COMPLEXITY,
        });
    }
    Ok(complexity)
}

fn invalid_glyph(
    table: Table,
    glyph_id: u16,
    relative_offset: usize,
    reason: &'static str,
) -> StbTrueTypeFontError {
    StbTrueTypeFontError::InvalidGlyph {
        glyph_id,
        offset: table.offset.saturating_add(relative_offset),
        reason,
    }
}

fn require_glyph_range(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    offset: usize,
    length: usize,
    reason: &'static str,
) -> Result<(), StbTrueTypeFontError> {
    if offset
        .checked_add(length)
        .is_none_or(|end| end > bytes.len())
    {
        return Err(invalid_glyph(
            table,
            glyph_id,
            glyph_offset.saturating_add(offset),
            reason,
        ));
    }
    Ok(())
}

fn glyph_u8(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    offset: usize,
) -> Result<u8, StbTrueTypeFontError> {
    bytes.get(offset).copied().ok_or_else(|| {
        invalid_glyph(
            table,
            glyph_id,
            glyph_offset.saturating_add(offset),
            "glyph byte stream is truncated",
        )
    })
}

fn glyph_u16(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    offset: usize,
) -> Result<u16, StbTrueTypeFontError> {
    require_glyph_range(
        bytes,
        table,
        glyph_id,
        glyph_offset,
        offset,
        2,
        "glyph 16-bit field is truncated",
    )?;
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

fn glyph_i16(
    bytes: &[u8],
    table: Table,
    glyph_id: u16,
    glyph_offset: usize,
    offset: usize,
) -> Result<i16, StbTrueTypeFontError> {
    glyph_u16(bytes, table, glyph_id, glyph_offset, offset).map(|value| value as i16)
}
