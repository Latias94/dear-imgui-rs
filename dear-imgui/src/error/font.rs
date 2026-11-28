use std::io;
use std::path::PathBuf;

use thiserror::Error;

use super::ContextError;

/// A failure during font atlas configuration, font loading, or font data validation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FontError {
    /// A font operation could not acquire the Dear ImGui thread lease.
    #[error("could not acquire Dear ImGui for a font operation: {0}")]
    Context(#[from] ContextError),

    /// Dear ImGui did not provide its built-in stb_truetype loader.
    #[error("Dear ImGui returned a null stb_truetype loader")]
    BuiltInLoaderUnavailable,

    /// The font file could not be read completely.
    #[error("failed to read TrueType font file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// A managed renderer already owns this atlas.
    #[error("the font atlas is owned by a managed renderer")]
    ManagedRendererActive,
    /// The previous managed renderer has not committed texture release yet.
    #[error("the font atlas is waiting for the previous renderer texture release to be committed")]
    RendererReleasePending,

    /// Dear ImGui has already retained one or more font sources.
    #[error(
        "the font atlas already contains {source_count} font source(s); set the loader before adding fonts"
    )]
    SourcesAlreadyAdded { source_count: usize },

    /// No source was provided.
    #[error("at least one font source is required")]
    NoSources,
    /// The atlas is not using a supported built-in loader for checked bytes.
    #[error("checked font bytes require a supported built-in atlas font loader")]
    UnsupportedAtlasLoader,
    /// A checked source attempted to bypass the atlas's loader selection.
    #[error("checked font bytes cannot override the font loader")]
    LoaderOverride,
    /// The selected loader does not support this face index for the input.
    #[error("font face index {index} is not supported for this input and loader")]
    InvalidFontIndex { index: u32 },
    /// The source is too small or its size exceeds Dear ImGui's native size type.
    #[error("TTF/OTF data must contain more than 100 bytes and fit in i32, got {length} bytes")]
    InvalidDataLength { length: usize },
    /// The FreeType source does not have a supported SFNT header and table directory.
    #[error("expected a supported TrueType or OpenType SFNT face")]
    InvalidSfnt,
    /// The fixed native FreeType wrapper cannot consume the table's glyph output safely.
    #[error("the checked FreeType route does not accept {format} glyphs")]
    UnsupportedFreeTypeGlyphs { format: &'static str },
    /// Dear ImGui could not allocate a font resource.
    #[error("Dear ImGui could not allocate memory for {resource}")]
    AllocationFailed { resource: &'static str },
    /// The native loader rejected the source.
    #[error("Dear ImGui's font loader rejected the source")]
    NativeRejected,

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
