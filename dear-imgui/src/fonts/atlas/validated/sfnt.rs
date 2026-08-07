use std::collections::BTreeMap;

use super::read::{
    largest_power_of_two, read_u16, read_u32, require_range, require_table_length, table_i16,
    table_u16, table_u32,
};
use super::{HEAD_MAGIC, MaxpLimits, StbTrueTypeFontError, TRUE_TYPE_SFNT_VERSION, Table};

pub(super) fn parse_table_directory(
    data: &[u8],
) -> Result<BTreeMap<[u8; 4], Table>, StbTrueTypeFontError> {
    let table_count = usize::from(read_u16(data, 4, "sfnt header")?);
    if table_count == 0 {
        return Err(StbTrueTypeFontError::InvalidDirectory {
            offset: 4,
            reason: "the font declares no tables",
        });
    }

    let directory_length = table_count
        .checked_mul(16)
        .and_then(|length| length.checked_add(12))
        .ok_or(StbTrueTypeFontError::InvalidDirectory {
            offset: 4,
            reason: "the table count overflows the directory length",
        })?;
    require_range(data, 0, directory_length, "sfnt table directory")?;

    let greatest_power = largest_power_of_two(table_count);
    let expected_search_range =
        greatest_power
            .checked_mul(16)
            .ok_or(StbTrueTypeFontError::InvalidDirectory {
                offset: 6,
                reason: "the table count cannot be represented by sfnt search fields",
            })?;
    if expected_search_range > usize::from(u16::MAX) {
        return Err(StbTrueTypeFontError::InvalidDirectory {
            offset: 6,
            reason: "the table count cannot be represented by sfnt search fields",
        });
    }
    let expected_entry_selector = greatest_power.trailing_zeros() as usize;
    let expected_range_shift = table_count * 16 - expected_search_range;
    if usize::from(read_u16(data, 6, "sfnt header")?) != expected_search_range
        || usize::from(read_u16(data, 8, "sfnt header")?) != expected_entry_selector
        || usize::from(read_u16(data, 10, "sfnt header")?) != expected_range_shift
    {
        return Err(StbTrueTypeFontError::InvalidDirectory {
            offset: 6,
            reason: "searchRange, entrySelector, or rangeShift disagrees with numTables",
        });
    }

    let mut tables = BTreeMap::new();
    for index in 0..table_count {
        let record_offset = 12 + index * 16;
        let tag = [
            data[record_offset],
            data[record_offset + 1],
            data[record_offset + 2],
            data[record_offset + 3],
        ];
        let offset = usize_from_u32(
            read_u32(data, record_offset + 8, "sfnt table record")?,
            record_offset + 8,
            "the table offset does not fit the target address space",
        )?;
        let length = usize_from_u32(
            read_u32(data, record_offset + 12, "sfnt table record")?,
            record_offset + 12,
            "the table length does not fit the target address space",
        )?;
        let end = offset
            .checked_add(length)
            .ok_or(StbTrueTypeFontError::TableOutOfBounds {
                tag,
                offset,
                end: usize::MAX,
                data_len: data.len(),
            })?;
        if end > data.len() {
            return Err(StbTrueTypeFontError::TableOutOfBounds {
                tag,
                offset,
                end,
                data_len: data.len(),
            });
        }
        if tables
            .insert(
                tag,
                Table {
                    tag,
                    offset,
                    length,
                },
            )
            .is_some()
        {
            return Err(StbTrueTypeFontError::DuplicateTable { tag });
        }
    }

    Ok(tables)
}

pub(super) fn required_table(
    tables: &BTreeMap<[u8; 4], Table>,
    tag: [u8; 4],
) -> Result<Table, StbTrueTypeFontError> {
    let table = tables
        .get(&tag)
        .copied()
        .ok_or(StbTrueTypeFontError::MissingTable { tag })?;
    if table.offset == 0 {
        return Err(table.invalid(
            0,
            "stb_truetype uses offset zero as the missing-table sentinel",
        ));
    }
    Ok(table)
}

pub(super) fn validate_head(data: &[u8], table: Table) -> Result<i16, StbTrueTypeFontError> {
    let bytes = table.bytes(data);
    require_table_length(table, 54)?;
    if table_u32(bytes, table, 0)? != TRUE_TYPE_SFNT_VERSION {
        return Err(table.invalid(0, "unsupported head table version"));
    }
    if table_u32(bytes, table, 12)? != HEAD_MAGIC {
        return Err(table.invalid(12, "invalid head table magic number"));
    }
    let units_per_em = table_u16(bytes, table, 18)?;
    if !(16..=16_384).contains(&units_per_em) {
        return Err(table.invalid(18, "unitsPerEm is outside the TrueType range"));
    }
    let x_min = table_i16(bytes, table, 36)?;
    let y_min = table_i16(bytes, table, 38)?;
    let x_max = table_i16(bytes, table, 40)?;
    let y_max = table_i16(bytes, table, 42)?;
    if x_min > x_max || y_min > y_max {
        return Err(table.invalid(36, "font bounding box minimum exceeds its maximum"));
    }
    let index_to_loc_format = table_i16(bytes, table, 50)?;
    if !matches!(index_to_loc_format, 0 | 1) {
        return Err(table.invalid(50, "indexToLocFormat must be 0 or 1"));
    }
    if table_i16(bytes, table, 52)? != 0 {
        return Err(table.invalid(52, "glyphDataFormat must be zero"));
    }
    Ok(index_to_loc_format)
}

pub(super) fn validate_maxp(data: &[u8], table: Table) -> Result<MaxpLimits, StbTrueTypeFontError> {
    let bytes = table.bytes(data);
    require_table_length(table, 32)?;
    if table_u32(bytes, table, 0)? != TRUE_TYPE_SFNT_VERSION {
        return Err(table.invalid(0, "glyf fonts require maxp version 1.0"));
    }
    let glyph_count = table_u16(bytes, table, 4)?;
    if glyph_count == 0 {
        return Err(table.invalid(4, "numGlyphs must be nonzero"));
    }

    Ok(MaxpLimits {
        glyph_count,
        max_points: usize::from(table_u16(bytes, table, 6)?),
        max_contours: usize::from(table_u16(bytes, table, 8)?),
        max_composite_points: usize::from(table_u16(bytes, table, 10)?),
        max_composite_contours: usize::from(table_u16(bytes, table, 12)?),
        max_instruction_bytes: usize::from(table_u16(bytes, table, 26)?),
        max_component_elements: usize::from(table_u16(bytes, table, 28)?),
        max_component_depth: usize::from(table_u16(bytes, table, 30)?),
    })
}

pub(super) fn validate_horizontal_metrics(
    data: &[u8],
    hhea: Table,
    hmtx: Table,
    glyph_count: u16,
) -> Result<(), StbTrueTypeFontError> {
    let hhea_bytes = hhea.bytes(data);
    require_table_length(hhea, 36)?;
    if table_u32(hhea_bytes, hhea, 0)? != TRUE_TYPE_SFNT_VERSION {
        return Err(hhea.invalid(0, "unsupported hhea table version"));
    }
    let ascender = table_i16(hhea_bytes, hhea, 4)?;
    let descender = table_i16(hhea_bytes, hhea, 6)?;
    if ascender <= descender {
        return Err(hhea.invalid(
            4,
            "ascender must be greater than descender for stb pixel-height scaling",
        ));
    }
    let metric_count = usize::from(table_u16(hhea_bytes, hhea, 34)?);
    if metric_count == 0 || metric_count > usize::from(glyph_count) {
        return Err(hhea.invalid(34, "numberOfHMetrics must be in 1..=maxp.numGlyphs"));
    }
    let side_bearing_count = usize::from(glyph_count) - metric_count;
    let required_hmtx_length = metric_count
        .checked_mul(4)
        .and_then(|length| {
            side_bearing_count
                .checked_mul(2)
                .and_then(|bearings| length.checked_add(bearings))
        })
        .ok_or_else(|| hmtx.invalid(0, "horizontal metric length overflowed usize"))?;
    if hmtx.length < required_hmtx_length {
        return Err(hmtx.invalid(
            hmtx.length,
            "hmtx is shorter than hhea.numberOfHMetrics and maxp.numGlyphs require",
        ));
    }
    Ok(())
}

pub(super) fn validate_loca(
    data: &[u8],
    table: Table,
    glyf_length: usize,
    glyph_count: u16,
    index_to_loc_format: i16,
) -> Result<Vec<usize>, StbTrueTypeFontError> {
    let bytes = table.bytes(data);
    let entry_size = if index_to_loc_format == 0 { 2 } else { 4 };
    let entry_count = usize::from(glyph_count) + 1;
    let required_length = entry_count
        .checked_mul(entry_size)
        .ok_or_else(|| table.invalid(0, "loca length overflowed usize"))?;
    if table.length != required_length {
        return Err(table.invalid(
            0,
            "loca length disagrees with maxp.numGlyphs and head.indexToLocFormat",
        ));
    }

    let mut locations = Vec::with_capacity(entry_count);
    let mut previous = 0;
    for index in 0..entry_count {
        let relative_offset = index * entry_size;
        let location = if entry_size == 2 {
            usize::from(table_u16(bytes, table, relative_offset)?) * 2
        } else {
            usize_from_table_u32(
                table_u32(bytes, table, relative_offset)?,
                table,
                relative_offset,
            )?
        };
        if location < previous {
            return Err(table.invalid(relative_offset, "loca offsets are not monotonic"));
        }
        if location > glyf_length {
            return Err(table.invalid(relative_offset, "loca offset exceeds the glyf table"));
        }
        previous = location;
        locations.push(location);
    }

    Ok(locations)
}

fn usize_from_u32(
    value: u32,
    offset: usize,
    reason: &'static str,
) -> Result<usize, StbTrueTypeFontError> {
    usize::try_from(value).map_err(|_| StbTrueTypeFontError::InvalidDirectory { offset, reason })
}

fn usize_from_table_u32(
    value: u32,
    table: Table,
    relative_offset: usize,
) -> Result<usize, StbTrueTypeFontError> {
    usize::try_from(value)
        .map_err(|_| table.invalid(relative_offset, "32-bit value does not fit usize"))
}
