use super::read::{largest_power_of_two, require_table_length, table_u16};
use super::{StbTrueTypeFontError, Table};

pub(super) fn validate_cmap(
    data: &[u8],
    table: Table,
    glyph_count: u16,
) -> Result<(), StbTrueTypeFontError> {
    let bytes = table.bytes(data);
    require_table_length(table, 4)?;
    if table_u16(bytes, table, 0)? != 0 {
        return Err(table.invalid(0, "cmap table version must be zero"));
    }
    let record_count = usize::from(table_u16(bytes, table, 2)?);
    let records_end = record_count
        .checked_mul(8)
        .and_then(|length| length.checked_add(4))
        .ok_or_else(|| table.invalid(2, "cmap encoding-record count overflowed usize"))?;
    if records_end > bytes.len() {
        return Err(StbTrueTypeFontError::InvalidCmap {
            offset: table.offset + 2,
            reason: "encoding records extend beyond the cmap table",
        });
    }

    let mut selected = None;
    for index in 0..record_count {
        let record_offset = 4 + index * 8;
        let platform = cmap_u16(bytes, table, record_offset)?;
        let encoding = cmap_u16(bytes, table, record_offset + 2)?;
        let subtable_offset = usize_from_cmap_u32(
            cmap_u32(bytes, table, record_offset + 4)?,
            table.offset + record_offset + 4,
        )?;
        if subtable_offset
            .checked_add(2)
            .is_none_or(|end| end > bytes.len())
        {
            return Err(StbTrueTypeFontError::InvalidCmap {
                offset: table.offset + record_offset + 4,
                reason: "an encoding record points outside the cmap table",
            });
        }
        if platform == 0 || (platform == 3 && matches!(encoding, 1 | 10)) {
            // stb_truetype intentionally keeps the last recognized record.
            selected = Some(subtable_offset);
        }
    }

    let subtable_offset = selected.ok_or(StbTrueTypeFontError::InvalidCmap {
        offset: table.offset + 4,
        reason: "stb_truetype recognizes no Unicode encoding record",
    })?;
    let format = cmap_u16(bytes, table, subtable_offset)?;
    match format {
        4 => validate_cmap_format_4(bytes, table, subtable_offset, glyph_count),
        12 => validate_cmap_format_12(bytes, table, subtable_offset, glyph_count),
        _ => Err(StbTrueTypeFontError::UnsupportedCmapFormat {
            format,
            offset: table.offset + subtable_offset,
        }),
    }
}

fn validate_cmap_format_4(
    cmap: &[u8],
    table: Table,
    offset: usize,
    glyph_count: u16,
) -> Result<(), StbTrueTypeFontError> {
    let declared_length = usize::from(cmap_u16(cmap, table, offset + 2)?);
    let end = offset
        .checked_add(declared_length)
        .ok_or(StbTrueTypeFontError::InvalidCmap {
            offset: table.offset + offset + 2,
            reason: "format 4 length overflowed usize",
        })?;
    if declared_length < 16 || end > cmap.len() {
        return Err(StbTrueTypeFontError::InvalidCmap {
            offset: table.offset + offset + 2,
            reason: "format 4 declared length is truncated",
        });
    }
    let subtable = &cmap[offset..end];
    let seg_count_x2 = usize::from(cmap_subtable_u16(subtable, table, offset, 6)?);
    if seg_count_x2 == 0 || seg_count_x2 % 2 != 0 {
        return Err(invalid_cmap_at(
            table,
            offset + 6,
            "format 4 segCountX2 must be positive and even",
        ));
    }
    let seg_count = seg_count_x2 / 2;
    let arrays_end = seg_count
        .checked_mul(8)
        .and_then(|length| length.checked_add(16))
        .ok_or_else(|| {
            invalid_cmap_at(table, offset + 6, "format 4 segment arrays overflow usize")
        })?;
    if arrays_end > subtable.len() {
        return Err(invalid_cmap_at(
            table,
            offset + 6,
            "format 4 segment arrays extend beyond the declared subtable",
        ));
    }

    let greatest_power = largest_power_of_two(seg_count);
    let expected_search_range = greatest_power * 2;
    let expected_entry_selector = greatest_power.trailing_zeros() as usize;
    let expected_range_shift = seg_count_x2 - expected_search_range;
    if usize::from(cmap_subtable_u16(subtable, table, offset, 8)?) != expected_search_range
        || usize::from(cmap_subtable_u16(subtable, table, offset, 10)?) != expected_entry_selector
        || usize::from(cmap_subtable_u16(subtable, table, offset, 12)?) != expected_range_shift
    {
        return Err(invalid_cmap_at(
            table,
            offset + 8,
            "format 4 binary-search fields disagree with segCountX2",
        ));
    }

    let end_codes = 14;
    let reserved_pad = end_codes + seg_count * 2;
    let start_codes = reserved_pad + 2;
    let id_deltas = start_codes + seg_count * 2;
    let id_range_offsets = id_deltas + seg_count * 2;
    let glyph_id_array = id_range_offsets + seg_count * 2;
    if cmap_subtable_u16(subtable, table, offset, reserved_pad)? != 0 {
        return Err(invalid_cmap_at(
            table,
            offset + reserved_pad,
            "format 4 reservedPad must be zero",
        ));
    }

    let mut previous_end = None;
    for segment in 0..seg_count {
        let end_code = cmap_subtable_u16(subtable, table, offset, end_codes + segment * 2)?;
        let start_code = cmap_subtable_u16(subtable, table, offset, start_codes + segment * 2)?;
        if start_code > end_code {
            return Err(invalid_cmap_at(
                table,
                offset + start_codes + segment * 2,
                "format 4 segment start exceeds its end",
            ));
        }
        if previous_end.is_some_and(|previous| start_code <= previous) {
            return Err(invalid_cmap_at(
                table,
                offset + start_codes + segment * 2,
                "format 4 segments overlap or are not strictly ordered",
            ));
        }
        previous_end = Some(end_code);

        let delta = cmap_subtable_i16(subtable, table, offset, id_deltas + segment * 2)?;
        let range_offset = usize::from(cmap_subtable_u16(
            subtable,
            table,
            offset,
            id_range_offsets + segment * 2,
        )?);
        if range_offset % 2 != 0 {
            return Err(invalid_cmap_at(
                table,
                offset + id_range_offsets + segment * 2,
                "format 4 idRangeOffset must be two-byte aligned",
            ));
        }

        for codepoint in u32::from(start_code)..=u32::from(end_code) {
            let glyph = if range_offset == 0 {
                ((codepoint as i64 + i64::from(delta)) & 0xffff) as u16
            } else {
                let codepoint_index = (codepoint - u32::from(start_code)) as usize;
                let target = id_range_offsets
                    .checked_add(segment * 2)
                    .and_then(|target| target.checked_add(range_offset))
                    .and_then(|target| target.checked_add(codepoint_index * 2))
                    .ok_or_else(|| {
                        invalid_cmap_at(
                            table,
                            offset + id_range_offsets + segment * 2,
                            "format 4 glyph index address overflowed usize",
                        )
                    })?;
                if target < glyph_id_array || target + 2 > subtable.len() {
                    return Err(invalid_cmap_at(
                        table,
                        offset + id_range_offsets + segment * 2,
                        "format 4 idRangeOffset points outside glyphIdArray",
                    ));
                }
                cmap_subtable_u16(subtable, table, offset, target)?
            };
            if glyph >= glyph_count && glyph != 0 {
                return Err(invalid_cmap_at(
                    table,
                    offset + id_range_offsets + segment * 2,
                    "format 4 maps a codepoint beyond maxp.numGlyphs",
                ));
            }
        }
    }

    if previous_end != Some(u16::MAX) {
        return Err(invalid_cmap_at(
            table,
            offset + end_codes + (seg_count - 1) * 2,
            "format 4 must end with the U+FFFF sentinel segment",
        ));
    }

    Ok(())
}

fn validate_cmap_format_12(
    cmap: &[u8],
    table: Table,
    offset: usize,
    glyph_count: u16,
) -> Result<(), StbTrueTypeFontError> {
    if offset.checked_add(16).is_none_or(|end| end > cmap.len()) {
        return Err(invalid_cmap_at(
            table,
            offset,
            "format 12 header is truncated",
        ));
    }
    if cmap_u16(cmap, table, offset + 2)? != 0 {
        return Err(invalid_cmap_at(
            table,
            offset + 2,
            "format 12 reserved field must be zero",
        ));
    }
    let declared_length = usize_from_cmap_u32(
        cmap_u32(cmap, table, offset + 4)?,
        table.offset + offset + 4,
    )?;
    let group_count = usize_from_cmap_u32(
        cmap_u32(cmap, table, offset + 12)?,
        table.offset + offset + 12,
    )?;
    let expected_length = group_count
        .checked_mul(12)
        .and_then(|length| length.checked_add(16))
        .ok_or_else(|| {
            invalid_cmap_at(table, offset + 12, "format 12 group array overflows usize")
        })?;
    if declared_length != expected_length
        || offset
            .checked_add(declared_length)
            .is_none_or(|end| end > cmap.len())
    {
        return Err(invalid_cmap_at(
            table,
            offset + 4,
            "format 12 declared length disagrees with nGroups or is truncated",
        ));
    }

    let mut previous_end = None;
    for group in 0..group_count {
        let group_offset = offset + 16 + group * 12;
        let start = cmap_u32(cmap, table, group_offset)?;
        let end = cmap_u32(cmap, table, group_offset + 4)?;
        let start_glyph = cmap_u32(cmap, table, group_offset + 8)?;
        if start > end {
            return Err(invalid_cmap_at(
                table,
                group_offset,
                "format 12 group start exceeds its end",
            ));
        }
        if end > 0x10_ffff {
            return Err(invalid_cmap_at(
                table,
                group_offset + 4,
                "format 12 group exceeds the Unicode scalar range",
            ));
        }
        if previous_end.is_some_and(|previous| start <= previous) {
            return Err(invalid_cmap_at(
                table,
                group_offset,
                "format 12 groups overlap or are not strictly ordered",
            ));
        }
        previous_end = Some(end);

        let last_glyph = start_glyph.checked_add(end - start).ok_or_else(|| {
            invalid_cmap_at(
                table,
                group_offset + 8,
                "format 12 glyph range overflows u32",
            )
        })?;
        if last_glyph >= u32::from(glyph_count) && last_glyph != 0 {
            return Err(invalid_cmap_at(
                table,
                group_offset + 8,
                "format 12 maps a codepoint beyond maxp.numGlyphs",
            ));
        }
    }

    Ok(())
}

fn invalid_cmap_at(
    table: Table,
    relative_offset: usize,
    reason: &'static str,
) -> StbTrueTypeFontError {
    StbTrueTypeFontError::InvalidCmap {
        offset: table.offset.saturating_add(relative_offset),
        reason,
    }
}

fn cmap_u16(bytes: &[u8], table: Table, offset: usize) -> Result<u16, StbTrueTypeFontError> {
    if offset.checked_add(2).is_none_or(|end| end > bytes.len()) {
        return Err(invalid_cmap_at(
            table,
            offset,
            "16-bit field extends beyond the cmap table",
        ));
    }
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

fn cmap_u32(bytes: &[u8], table: Table, offset: usize) -> Result<u32, StbTrueTypeFontError> {
    if offset.checked_add(4).is_none_or(|end| end > bytes.len()) {
        return Err(invalid_cmap_at(
            table,
            offset,
            "32-bit field extends beyond the cmap table",
        ));
    }
    Ok(u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn cmap_subtable_u16(
    bytes: &[u8],
    table: Table,
    subtable_offset: usize,
    offset: usize,
) -> Result<u16, StbTrueTypeFontError> {
    if offset.checked_add(2).is_none_or(|end| end > bytes.len()) {
        return Err(invalid_cmap_at(
            table,
            subtable_offset.saturating_add(offset),
            "16-bit field extends beyond the declared cmap subtable",
        ));
    }
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

fn cmap_subtable_i16(
    bytes: &[u8],
    table: Table,
    subtable_offset: usize,
    offset: usize,
) -> Result<i16, StbTrueTypeFontError> {
    cmap_subtable_u16(bytes, table, subtable_offset, offset).map(|value| value as i16)
}

fn usize_from_cmap_u32(value: u32, absolute_offset: usize) -> Result<usize, StbTrueTypeFontError> {
    usize::try_from(value).map_err(|_| StbTrueTypeFontError::InvalidCmap {
        offset: absolute_offset,
        reason: "32-bit cmap value does not fit usize",
    })
}
