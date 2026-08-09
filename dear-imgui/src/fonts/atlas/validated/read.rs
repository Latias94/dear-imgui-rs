use super::{StbTrueTypeFontError, Table};

pub(super) fn require_table_length(
    table: Table,
    required: usize,
) -> Result<(), StbTrueTypeFontError> {
    if table.length < required {
        return Err(table.invalid(
            table.length,
            "table is shorter than the required fixed header",
        ));
    }
    Ok(())
}

pub(super) fn table_u16(
    bytes: &[u8],
    table: Table,
    offset: usize,
) -> Result<u16, StbTrueTypeFontError> {
    if offset.checked_add(2).is_none_or(|end| end > bytes.len()) {
        return Err(table.invalid(offset, "16-bit field extends beyond the table"));
    }
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

pub(super) fn table_i16(
    bytes: &[u8],
    table: Table,
    offset: usize,
) -> Result<i16, StbTrueTypeFontError> {
    table_u16(bytes, table, offset).map(|value| value as i16)
}

pub(super) fn table_u32(
    bytes: &[u8],
    table: Table,
    offset: usize,
) -> Result<u32, StbTrueTypeFontError> {
    if offset.checked_add(4).is_none_or(|end| end > bytes.len()) {
        return Err(table.invalid(offset, "32-bit field extends beyond the table"));
    }
    Ok(u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

pub(super) fn read_u16(
    data: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u16, StbTrueTypeFontError> {
    require_range(data, offset, 2, context)?;
    Ok(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

pub(super) fn read_u32(
    data: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u32, StbTrueTypeFontError> {
    require_range(data, offset, 4, context)?;
    Ok(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

pub(super) fn require_range(
    data: &[u8],
    offset: usize,
    needed: usize,
    context: &'static str,
) -> Result<(), StbTrueTypeFontError> {
    if offset
        .checked_add(needed)
        .is_none_or(|end| end > data.len())
    {
        return Err(StbTrueTypeFontError::Truncated {
            context,
            offset,
            needed,
            available: data.len().saturating_sub(offset),
        });
    }
    Ok(())
}

pub(super) fn largest_power_of_two(value: usize) -> usize {
    1_usize << (usize::BITS - 1 - value.leading_zeros())
}
