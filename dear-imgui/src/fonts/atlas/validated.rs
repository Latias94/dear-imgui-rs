use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use ttf_parser::RawFace;
use ttf_parser::head::IndexToLocationFormat;

use crate::FontError;

mod glyph;
#[cfg(test)]
mod tests;

const TRUE_TYPE_SFNT_VERSION: u32 = 0x0001_0000;
const HEAD_MAGIC: u32 = 0x5f0f_3cf5;
const MAX_COMPOSITE_DEPTH: usize = 32;
const MAX_COMPONENTS_PER_GLYPH: usize = 4_096;
const MAX_EXPANDED_GLYPH_COMPLEXITY: usize = 1_000_000;
const MAX_VALIDATED_FONT_DATA_LEN: usize = 256 * 1024 * 1024;
const MAX_INITIAL_FILE_CAPACITY: usize = 8 * 1024 * 1024;

const CMAP: [u8; 4] = *b"cmap";
const GLYF: [u8; 4] = *b"glyf";
const HEAD: [u8; 4] = *b"head";
const HHEA: [u8; 4] = *b"hhea";
const HMTX: [u8; 4] = *b"hmtx";
const LOCA: [u8; 4] = *b"loca";
const MAXP: [u8; 4] = *b"maxp";

/// Owned font bytes proven safe for Dear ImGui's stb_truetype loader.
///
/// The proof deliberately accepts a narrower format than a general OpenType parser:
/// one standalone sfnt with TrueType `glyf` outlines and the exact format 4 or format
/// 12 character map that stb_truetype will select. Collections, CFF outlines, web-font
/// containers, borrowed bytes, and compressed inputs remain outside this safe type.
///
/// `ttf-parser` reads the SFNT directory and common table fields. Separate checks cover
/// the reads and composite glyph expansion of the pinned Dear ImGui loader, which calls
/// stb_truetype without a byte length or recursion limit.
#[derive(Clone)]
pub struct StbTrueTypeFontData {
    bytes: Arc<[u8]>,
}

impl StbTrueTypeFontData {
    /// Largest byte buffer accepted by the validated stb_truetype path.
    ///
    /// Larger fonts can still use the explicit unsafe raw-source constructors when the caller can
    /// prove the native loader contract independently.
    pub const MAX_BYTES: usize = MAX_VALIDATED_FONT_DATA_LEN;

    /// Validate owned bytes for the built-in stb_truetype loader.
    pub fn from_bytes(bytes: impl AsRef<[u8]> + Into<Arc<[u8]>>) -> Result<Self, FontError> {
        validate_font_data_length(bytes.as_ref().len())?;
        Self::from_length_checked_bytes(bytes.into())
    }

    /// Copy and validate a borrowed byte slice.
    ///
    /// The byte limit is checked before allocating the owned copy.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, FontError> {
        validate_font_data_length(bytes.len())?;
        Self::from_length_checked_bytes(Arc::<[u8]>::from(bytes))
    }

    /// Read and validate a standalone TrueType font file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, FontError> {
        let path = path.as_ref();
        let read_error = |source| FontError::Io {
            path: path.to_owned(),
            source,
        };
        let file = File::open(path).map_err(read_error)?;
        let metadata = file.metadata().map_err(read_error)?;
        let declared_length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        validate_font_data_length(declared_length)?;

        let mut bytes = Vec::with_capacity(declared_length.min(MAX_INITIAL_FILE_CAPACITY));
        file.take((MAX_VALIDATED_FONT_DATA_LEN as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(read_error)?;
        Self::from_bytes(bytes)
    }

    /// Borrow the validated font bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the validated byte length.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Return whether this font has no bytes.
    ///
    /// Successfully constructed values always return `false`.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn from_length_checked_bytes(bytes: Arc<[u8]>) -> Result<Self, FontError> {
        validate_font(&bytes)?;
        Ok(Self { bytes })
    }
}

impl AsRef<[u8]> for StbTrueTypeFontData {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Debug for StbTrueTypeFontData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StbTrueTypeFontData")
            .field("len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl TryFrom<Vec<u8>> for StbTrueTypeFontData {
    type Error = FontError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<Arc<[u8]>> for StbTrueTypeFontData {
    type Error = FontError;

    fn try_from(bytes: Arc<[u8]>) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

#[derive(Clone, Copy, Debug)]
struct Table<'a> {
    tag: [u8; 4],
    offset: usize,
    bytes: &'a [u8],
}

impl<'a> Table<'a> {
    fn header<const N: usize>(self) -> Result<&'a [u8; N], FontError> {
        self.bytes.first_chunk().ok_or_else(|| {
            self.invalid(
                self.bytes.len(),
                "table is shorter than the required fixed header",
            )
        })
    }

    fn invalid(self, relative_offset: usize, reason: &'static str) -> FontError {
        FontError::InvalidTable {
            tag: self.tag,
            offset: self.offset.saturating_add(relative_offset),
            reason,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MaxpLimits {
    glyph_count: u16,
    max_points: usize,
    max_contours: usize,
    max_composite_points: usize,
    max_composite_contours: usize,
    max_instruction_bytes: usize,
    max_component_elements: usize,
    max_component_depth: usize,
}

/// Checks the contracts of the pinned native parser.
///
/// The stb loader does not pass `FontDataSize` to `stbtt_InitFont`. It selects its own cmap
/// and walks the metrics, glyph locations and composite data on demand. Initialization looks
/// up `GPOS` and `kern` in the directory, but the ImGui loader never calls stb's kerning APIs.
pub(super) fn validate_font(data: &[u8]) -> Result<(), FontError> {
    validate_font_data_length(data.len())?;

    let signature = u32::from_be_bytes(field(data, 0).ok_or(FontError::Truncated {
        context: "sfnt header",
        offset: 0,
        needed: 4,
        available: data.len(),
    })?);
    if signature != TRUE_TYPE_SFNT_VERSION {
        return Err(FontError::UnsupportedContainer { signature });
    }

    let face = RawFace::parse(data, 0).map_err(|_| match field(data, 4) {
        Some(count) => FontError::Truncated {
            context: "sfnt table directory",
            offset: 0,
            needed: 12 + usize::from(u16::from_be_bytes(count)) * 16,
            available: data.len(),
        },
        None => FontError::Truncated {
            context: "sfnt header",
            offset: 4,
            needed: 2,
            available: data.len() - 4,
        },
    })?;
    let tables = checked_directory(face)?;
    let required = |tag| {
        let table = tables
            .get(&tag)
            .copied()
            .ok_or(FontError::MissingTable { tag })?;
        if table.offset == 0 {
            return Err(table.invalid(0, "stb_truetype treats offset zero as a missing table"));
        }
        Ok(table)
    };
    let cmap = required(CMAP)?;
    // A nonzero glyf offset pins stb to its TrueType path, including for hybrid fonts.
    let glyf = required(GLYF)?;
    let head = required(HEAD)?;
    let hhea = required(HHEA)?;
    let hmtx = required(HMTX)?;
    let loca = required(LOCA)?;
    let maxp = required(MAXP)?;

    let location_format = validate_head(head)?;
    let limits = validate_maxp(maxp)?;
    validate_horizontal_metrics(hhea, hmtx, limits.glyph_count)?;
    validate_cmap(cmap, limits.glyph_count)?;
    let locations = validate_loca(loca, glyf.bytes.len(), limits.glyph_count, location_format)?;
    glyph::validate_glyphs(glyf, &locations, limits)
}

fn checked_directory(face: RawFace<'_>) -> Result<BTreeMap<[u8; 4], Table<'_>>, FontError> {
    let count = usize::from(face.table_records.len());
    if count == 0 {
        return Err(FontError::InvalidDirectory {
            offset: 4,
            reason: "the font declares no tables",
        });
    }
    let entry_selector = count.ilog2() as u16;
    let search_range = 16_usize << entry_selector;
    if search_range > usize::from(u16::MAX) {
        return Err(FontError::InvalidDirectory {
            offset: 6,
            reason: "the table count cannot be represented by sfnt search fields",
        });
    }
    let range_shift = count * 16 - search_range;
    let search_fields = [search_range as u16, entry_selector, range_shift as u16];
    for (index, expected) in search_fields.into_iter().enumerate() {
        let offset = 6 + index * 2;
        if field(face.data, offset) != Some(expected.to_be_bytes()) {
            return Err(FontError::InvalidDirectory {
                offset,
                reason: "searchRange, entrySelector, or rangeShift disagrees with numTables",
            });
        }
    }

    // stb searches in directory order and ignores lengths. RawFace::table uses binary search,
    // so walk all records to accept unsorted directories and check every declared range and tag.
    let mut tables = BTreeMap::new();
    for record in face.table_records {
        let tag = record.tag.to_bytes();
        let offset = record.offset as usize;
        let end = offset.saturating_add(record.length as usize);
        let bytes = face
            .data
            .get(offset..end)
            .ok_or(FontError::TableOutOfBounds {
                tag,
                offset,
                end,
                data_len: face.data.len(),
            })?;
        if tables.insert(tag, Table { tag, offset, bytes }).is_some() {
            return Err(FontError::DuplicateTable { tag });
        }
    }
    Ok(tables)
}

fn validate_head(table: Table<'_>) -> Result<IndexToLocationFormat, FontError> {
    let header = table.header::<54>()?;
    if header[..4] != TRUE_TYPE_SFNT_VERSION.to_be_bytes() {
        return Err(table.invalid(0, "unsupported head table version"));
    }
    if header[12..16] != HEAD_MAGIC.to_be_bytes() {
        return Err(table.invalid(12, "invalid head table magic number"));
    }
    let head = ttf_parser::head::Table::parse(header).ok_or_else(|| {
        let units = u16::from_be_bytes([header[18], header[19]]);
        if (16..=16_384).contains(&units) {
            table.invalid(50, "indexToLocFormat must be 0 or 1")
        } else {
            table.invalid(18, "unitsPerEm is outside the TrueType range")
        }
    })?;
    let bbox = head.global_bbox;
    if bbox.x_min > bbox.x_max || bbox.y_min > bbox.y_max {
        return Err(table.invalid(36, "font bounding box minimum exceeds its maximum"));
    }
    if header[52..54] != [0, 0] {
        return Err(table.invalid(52, "glyphDataFormat must be zero"));
    }
    Ok(head.index_to_location_format)
}

fn validate_maxp(table: Table<'_>) -> Result<MaxpLimits, FontError> {
    let header = table.header::<32>()?;
    if header[..4] != TRUE_TYPE_SFNT_VERSION.to_be_bytes() {
        return Err(table.invalid(0, "glyf fonts require maxp version 1.0"));
    }
    let maxp = ttf_parser::maxp::Table::parse(header)
        .ok_or_else(|| table.invalid(4, "numGlyphs must be nonzero"))?;
    let words = header.as_chunks::<2>().0;
    let limit = |index: usize| usize::from(u16::from_be_bytes(words[index]));
    Ok(MaxpLimits {
        glyph_count: maxp.number_of_glyphs.get(),
        max_points: limit(3),
        max_contours: limit(4),
        max_composite_points: limit(5),
        max_composite_contours: limit(6),
        max_instruction_bytes: limit(13),
        max_component_elements: limit(14),
        max_component_depth: limit(15),
    })
}

fn validate_horizontal_metrics(
    hhea: Table<'_>,
    hmtx: Table<'_>,
    glyph_count: u16,
) -> Result<(), FontError> {
    let header = hhea.header::<36>()?;
    if header[..4] != TRUE_TYPE_SFNT_VERSION.to_be_bytes() {
        return Err(hhea.invalid(0, "unsupported hhea table version"));
    }
    let metrics = ttf_parser::hhea::Table::parse(header)
        .ok_or_else(|| hhea.invalid(0, "malformed horizontal header"))?;
    if metrics.ascender <= metrics.descender {
        return Err(hhea.invalid(
            4,
            "ascender must be greater than descender for stb pixel-height scaling",
        ));
    }
    if !(1..=glyph_count).contains(&metrics.number_of_metrics) {
        return Err(hhea.invalid(34, "numberOfHMetrics must be in 1..=maxp.numGlyphs"));
    }
    // stb reads every trailing side bearing. ttf-parser can omit the bearings on short data.
    let required_length = 4 * usize::from(metrics.number_of_metrics)
        + 2 * usize::from(glyph_count - metrics.number_of_metrics);
    if hmtx.bytes.len() < required_length {
        return Err(hmtx.invalid(
            hmtx.bytes.len(),
            "hmtx is shorter than hhea.numberOfHMetrics and maxp.numGlyphs require",
        ));
    }
    Ok(())
}

fn validate_loca(
    table: Table<'_>,
    glyf_length: usize,
    glyph_count: u16,
    format: IndexToLocationFormat,
) -> Result<Vec<usize>, FontError> {
    // Keep both offsets for every glyph, including empty glyphs and the final sentinel.
    // ttf-parser's glyph_range() returns None for empty and reversed ranges alike.
    let entry_size = match format {
        IndexToLocationFormat::Short => 2,
        IndexToLocationFormat::Long => 4,
    };
    let count = usize::from(glyph_count) + 1;
    if table.bytes.len() != count * entry_size {
        return Err(table.invalid(
            0,
            "loca length disagrees with maxp.numGlyphs and head.indexToLocFormat",
        ));
    }
    let mut locations = Vec::with_capacity(count);
    let mut previous = 0;
    for (index, entry) in table.bytes.chunks_exact(entry_size).enumerate() {
        let location = match format {
            IndexToLocationFormat::Short => {
                usize::from(u16::from_be_bytes([entry[0], entry[1]])) * 2
            }
            IndexToLocationFormat::Long => {
                u32::from_be_bytes([entry[0], entry[1], entry[2], entry[3]]) as usize
            }
        };
        if location < previous {
            return Err(table.invalid(index * entry_size, "loca offsets are not monotonic"));
        }
        if location > glyf_length {
            return Err(table.invalid(index * entry_size, "loca offset exceeds the glyf table"));
        }
        previous = location;
        locations.push(location);
    }
    Ok(locations)
}

fn field<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..)?.first_chunk().copied()
}

fn validate_font_data_length(length: usize) -> Result<(), FontError> {
    if length > MAX_VALIDATED_FONT_DATA_LEN {
        return Err(FontError::DataTooLarge {
            length,
            limit: MAX_VALIDATED_FONT_DATA_LEN,
        });
    }
    Ok(())
}

fn validate_cmap(table: Table<'_>, glyph_count: u16) -> Result<(), FontError> {
    let header = table.header::<4>()?;
    if header[..2] != [0, 0] {
        return Err(table.invalid(0, "cmap table version must be zero"));
    }
    let count = usize::from(u16::from_be_bytes([header[2], header[3]]));
    let records = table.bytes.get(4..4 + count * 8).ok_or_else(|| {
        invalid_cmap(
            table.offset + 2,
            "encoding records extend beyond the cmap table",
        )
    })?;

    // ttf-parser may skip unrecognized or malformed subtables. stb keeps the last matching
    // encoding record, so inspect the exact record it will use and bound every stored offset.
    let mut selected = None;
    for (index, &[p0, p1, e0, e1, o0, o1, o2, o3]) in records.as_chunks::<8>().0.iter().enumerate()
    {
        let platform = u16::from_be_bytes([p0, p1]);
        let encoding = u16::from_be_bytes([e0, e1]);
        let offset = u32::from_be_bytes([o0, o1, o2, o3]) as usize;
        let format = field(table.bytes, offset)
            .map(u16::from_be_bytes)
            .ok_or_else(|| {
                invalid_cmap(
                    table.offset + 8 + index * 8,
                    "an encoding record points outside the cmap table",
                )
            })?;
        if platform == 0 || (platform == 3 && matches!(encoding, 1 | 10)) {
            selected = Some((offset, format));
        }
    }

    let (offset, format) = selected.ok_or_else(|| {
        invalid_cmap(
            table.offset + 4,
            "stb_truetype recognizes no Unicode encoding record",
        )
    })?;
    let absolute = table.offset + offset;
    let data = &table.bytes[offset..];
    match format {
        4 => validate_format_4(data, absolute, glyph_count),
        12 => validate_format_12(data, absolute, glyph_count),
        _ => Err(FontError::UnsupportedCmapFormat {
            format,
            offset: absolute,
        }),
    }
}

fn validate_format_4(data: &[u8], base: usize, glyph_count: u16) -> Result<(), FontError> {
    let error = |offset, reason| invalid_cmap(base + offset, reason);
    let length = field(data, 2).map(u16::from_be_bytes).unwrap_or(0);
    let subtable = data
        .get(..usize::from(length))
        .filter(|bytes| bytes.len() >= 16)
        .ok_or_else(|| error(2, "format 4 declared length is truncated"))?;
    let words = subtable.as_chunks::<2>().0;
    let word = |index| u16::from_be_bytes(words[index]);
    let seg_count_x2 = word(3);
    if seg_count_x2 == 0 || seg_count_x2 % 2 != 0 {
        return Err(error(6, "format 4 segCountX2 must be positive and even"));
    }
    let count = usize::from(seg_count_x2 / 2);
    let glyph_id_array = 8 + count * 4;
    if words.len() < glyph_id_array {
        return Err(error(
            6,
            "format 4 segment arrays extend beyond the declared subtable",
        ));
    }
    let entry_selector = count.ilog2() as u16;
    let search_range = 2_u16 << entry_selector;
    if word(4) != search_range
        || word(5) != entry_selector
        || word(6) != seg_count_x2 - search_range
    {
        return Err(error(
            8,
            "format 4 binary-search fields disagree with segCountX2",
        ));
    }

    let end_codes = 7;
    let reserved_pad = end_codes + count;
    let start_codes = reserved_pad + 1;
    let id_deltas = start_codes + count;
    let id_range_offsets = id_deltas + count;
    if word(reserved_pad) != 0 {
        return Err(error(reserved_pad * 2, "format 4 reservedPad must be zero"));
    }

    let mut previous_end = None;
    for segment in 0..count {
        let end_code = word(end_codes + segment);
        let start_code = word(start_codes + segment);
        if start_code > end_code {
            return Err(error(
                (start_codes + segment) * 2,
                "format 4 segment start exceeds its end",
            ));
        }
        if previous_end.is_some_and(|previous| start_code <= previous) {
            return Err(error(
                (start_codes + segment) * 2,
                "format 4 segments overlap or are not strictly ordered",
            ));
        }
        previous_end = Some(end_code);

        let range_offset_index = id_range_offsets + segment;
        let range_offset = word(range_offset_index);
        if range_offset % 2 != 0 {
            return Err(error(
                range_offset_index * 2,
                "format 4 idRangeOffset must be two-byte aligned",
            ));
        }
        let invalid_glyph = if range_offset == 0 {
            let delta = word(id_deltas + segment);
            (start_code..=end_code).any(|codepoint| codepoint.wrapping_add(delta) >= glyph_count)
        } else {
            let start = range_offset_index + usize::from(range_offset / 2);
            let end = start + usize::from(end_code - start_code) + 1;
            let glyphs = words
                .get(start..end)
                .filter(|_| start >= glyph_id_array)
                .ok_or_else(|| {
                    error(
                        range_offset_index * 2,
                        "format 4 idRangeOffset points outside glyphIdArray",
                    )
                })?;
            // Unlike the OpenType mapping in ttf-parser, stb returns these values directly.
            glyphs
                .iter()
                .any(|word| u16::from_be_bytes(*word) >= glyph_count)
        };
        if invalid_glyph {
            return Err(error(
                range_offset_index * 2,
                "format 4 maps a codepoint beyond maxp.numGlyphs",
            ));
        }
    }

    if previous_end != Some(u16::MAX) {
        return Err(error(
            (end_codes + count - 1) * 2,
            "format 4 must end with the U+FFFF sentinel segment",
        ));
    }
    Ok(())
}

fn validate_format_12(data: &[u8], base: usize, glyph_count: u16) -> Result<(), FontError> {
    let error = |offset, reason| invalid_cmap(base + offset, reason);
    let header = data
        .first_chunk::<16>()
        .ok_or_else(|| error(0, "format 12 header is truncated"))?;
    let words = header.as_chunks::<4>().0;
    if header[2..4] != [0, 0] {
        return Err(error(2, "format 12 reserved field must be zero"));
    }
    let length = u32::from_be_bytes(words[1]) as usize;
    let count = u32::from_be_bytes(words[3]) as usize;
    let groups = data.get(16..length).ok_or_else(|| {
        error(
            4,
            "format 12 declared length disagrees with nGroups or is truncated",
        )
    })?;
    let (groups, remainder) = groups.as_chunks::<12>();
    if !remainder.is_empty() || groups.len() != count {
        return Err(error(
            4,
            "format 12 declared length disagrees with nGroups or is truncated",
        ));
    }

    let mut previous_end = None;
    for (index, group) in groups.iter().enumerate() {
        let offset = 16 + index * 12;
        let words = group.as_chunks::<4>().0;
        let start = u32::from_be_bytes(words[0]);
        let end = u32::from_be_bytes(words[1]);
        let start_glyph = u32::from_be_bytes(words[2]);
        if start > end {
            return Err(error(offset, "format 12 group start exceeds its end"));
        }
        if end > 0x10_ffff {
            return Err(error(
                offset + 4,
                "format 12 group exceeds the Unicode scalar range",
            ));
        }
        if previous_end.is_some_and(|previous| start <= previous) {
            return Err(error(
                offset,
                "format 12 groups overlap or are not strictly ordered",
            ));
        }
        previous_end = Some(end);

        let last_glyph = start_glyph
            .checked_add(end - start)
            .ok_or_else(|| error(offset + 8, "format 12 glyph range overflows u32"))?;
        if last_glyph >= u32::from(glyph_count) {
            return Err(error(
                offset + 8,
                "format 12 maps a codepoint beyond maxp.numGlyphs",
            ));
        }
    }
    Ok(())
}

fn invalid_cmap(offset: usize, reason: &'static str) -> FontError {
    FontError::InvalidCmap { offset, reason }
}
