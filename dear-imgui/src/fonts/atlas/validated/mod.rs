use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

mod cmap;
mod glyph;
mod read;
mod sfnt;
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
/// Validation checks every native read that stb_truetype performs for the supported
/// tables. It also rejects recursive or excessively expanded composite glyphs before
/// the bytes can cross FFI, because stb_truetype itself has no input length or recursion
/// boundary.
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
    pub fn from_bytes(
        bytes: impl AsRef<[u8]> + Into<Arc<[u8]>>,
    ) -> Result<Self, StbTrueTypeFontError> {
        validate_font_data_length(bytes.as_ref().len())?;
        Self::from_length_checked_bytes(bytes.into())
    }

    /// Copy and validate a borrowed byte slice.
    ///
    /// The byte limit is checked before allocating the owned copy.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, StbTrueTypeFontError> {
        validate_font_data_length(bytes.len())?;
        Self::from_length_checked_bytes(Arc::<[u8]>::from(bytes))
    }

    /// Read and validate a standalone TrueType font file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, StbTrueTypeFontLoadError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| StbTrueTypeFontLoadError::Io {
            path: path.to_owned(),
            source,
        })?;
        let metadata = file
            .metadata()
            .map_err(|source| StbTrueTypeFontLoadError::Io {
                path: path.to_owned(),
                source,
            })?;
        let declared_length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        validate_font_data_length(declared_length).map_err(StbTrueTypeFontLoadError::Validation)?;

        let mut bytes = Vec::with_capacity(declared_length.min(MAX_INITIAL_FILE_CAPACITY));
        file.take((MAX_VALIDATED_FONT_DATA_LEN as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| StbTrueTypeFontLoadError::Io {
                path: path.to_owned(),
                source,
            })?;
        Self::from_bytes(bytes).map_err(StbTrueTypeFontLoadError::Validation)
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

    fn from_length_checked_bytes(bytes: Arc<[u8]>) -> Result<Self, StbTrueTypeFontError> {
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
    type Error = StbTrueTypeFontError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<Arc<[u8]>> for StbTrueTypeFontData {
    type Error = StbTrueTypeFontError;

    fn try_from(bytes: Arc<[u8]>) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

/// A structural validation failure for [`StbTrueTypeFontData`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum StbTrueTypeFontError {
    /// The safe validator does not admit this byte length.
    #[error("font data length {length} exceeds the validated stb_truetype limit {limit}")]
    DataTooLarge { length: usize, limit: usize },
    /// The input is not a standalone TrueType sfnt.
    #[error("unsupported font container signature {signature:#010x}; expected 0x00010000")]
    UnsupportedContainer { signature: u32 },
    /// A fixed-size structure extends past the available bytes.
    #[error(
        "truncated {context} at byte {offset}: need {needed} bytes but only {available} remain"
    )]
    Truncated {
        context: &'static str,
        offset: usize,
        needed: usize,
        available: usize,
    },
    /// The sfnt table directory is internally inconsistent.
    #[error("invalid sfnt table directory at byte {offset}: {reason}")]
    InvalidDirectory { offset: usize, reason: &'static str },
    /// The same table tag appears more than once.
    #[error("duplicate sfnt table tag {tag:?}")]
    DuplicateTable { tag: [u8; 4] },
    /// A table needed by stb_truetype is absent.
    #[error("missing required sfnt table {tag:?}")]
    MissingTable { tag: [u8; 4] },
    /// A declared table range is outside the owned byte buffer.
    #[error("sfnt table {tag:?} range {offset}..{end} is outside font data of length {data_len}")]
    TableOutOfBounds {
        tag: [u8; 4],
        offset: usize,
        end: usize,
        data_len: usize,
    },
    /// A required table is malformed or disagrees with another required table.
    #[error("invalid sfnt table {tag:?} at byte {offset}: {reason}")]
    InvalidTable {
        tag: [u8; 4],
        offset: usize,
        reason: &'static str,
    },
    /// stb_truetype would select a character map outside the proven subset.
    #[error(
        "stb_truetype selected unsupported cmap format {format} at byte {offset}; only formats 4 and 12 are accepted"
    )]
    UnsupportedCmapFormat { format: u16, offset: usize },
    /// The cmap selected by stb_truetype is malformed.
    #[error("invalid stb-selected cmap at byte {offset}: {reason}")]
    InvalidCmap { offset: usize, reason: &'static str },
    /// A glyph record is malformed.
    #[error("invalid glyf record {glyph_id} at byte {offset}: {reason}")]
    InvalidGlyph {
        glyph_id: u16,
        offset: usize,
        reason: &'static str,
    },
    /// A composite glyph refers beyond `maxp.numGlyphs`.
    #[error(
        "composite glyph {glyph_id} refers to glyph {referenced_glyph}, but the font declares only {glyph_count} glyphs"
    )]
    InvalidGlyphReference {
        glyph_id: u16,
        referenced_glyph: u16,
        glyph_count: u16,
    },
    /// Composite glyph references contain a cycle.
    #[error("composite glyph cycle from glyph {glyph_id} to glyph {referenced_glyph}")]
    CompositeCycle {
        glyph_id: u16,
        referenced_glyph: u16,
    },
    /// Composite nesting exceeds the safe native recursion budget.
    #[error("composite glyph {glyph_id} has depth {depth}, exceeding the validated limit {limit}")]
    CompositeDepth {
        glyph_id: u16,
        depth: usize,
        limit: usize,
    },
    /// Recursive expansion would require unreasonable native work or allocation.
    #[error(
        "composite glyph {glyph_id} expands to complexity {complexity}, exceeding the validated limit {limit}"
    )]
    CompositeComplexity {
        glyph_id: u16,
        complexity: usize,
        limit: usize,
    },
}

/// A file-system or validation failure from [`StbTrueTypeFontData::from_file`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StbTrueTypeFontLoadError {
    /// The font file could not be read completely.
    #[error("failed to read TrueType font file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file was read but is outside the safe stb_truetype subset.
    #[error(transparent)]
    Validation(#[from] StbTrueTypeFontError),
}

#[derive(Clone, Copy, Debug)]
struct Table {
    tag: [u8; 4],
    offset: usize,
    length: usize,
}

impl Table {
    fn end(self) -> usize {
        self.offset + self.length
    }

    fn bytes(self, data: &[u8]) -> &[u8] {
        &data[self.offset..self.end()]
    }

    fn invalid(self, relative_offset: usize, reason: &'static str) -> StbTrueTypeFontError {
        StbTrueTypeFontError::InvalidTable {
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

fn validate_font(data: &[u8]) -> Result<(), StbTrueTypeFontError> {
    validate_font_data_length(data.len())?;

    let signature = read::read_u32(data, 0, "sfnt header")?;
    if signature != TRUE_TYPE_SFNT_VERSION {
        return Err(StbTrueTypeFontError::UnsupportedContainer { signature });
    }

    let tables = sfnt::parse_table_directory(data)?;
    let cmap = sfnt::required_table(&tables, CMAP)?;
    let glyf = sfnt::required_table(&tables, GLYF)?;
    let head = sfnt::required_table(&tables, HEAD)?;
    let hhea = sfnt::required_table(&tables, HHEA)?;
    let hmtx = sfnt::required_table(&tables, HMTX)?;
    let loca = sfnt::required_table(&tables, LOCA)?;
    let maxp = sfnt::required_table(&tables, MAXP)?;

    let index_to_loc_format = sfnt::validate_head(data, head)?;
    let maxp_limits = sfnt::validate_maxp(data, maxp)?;
    sfnt::validate_horizontal_metrics(data, hhea, hmtx, maxp_limits.glyph_count)?;
    cmap::validate_cmap(data, cmap, maxp_limits.glyph_count)?;
    let locations = sfnt::validate_loca(
        data,
        loca,
        glyf.length,
        maxp_limits.glyph_count,
        index_to_loc_format,
    )?;
    glyph::validate_glyphs(data, glyf, &locations, maxp_limits)?;

    Ok(())
}

fn validate_font_data_length(length: usize) -> Result<(), StbTrueTypeFontError> {
    if length > MAX_VALIDATED_FONT_DATA_LEN {
        return Err(StbTrueTypeFontError::DataTooLarge {
            length,
            limit: MAX_VALIDATED_FONT_DATA_LEN,
        });
    }
    Ok(())
}
