use thiserror::Error;

/// A renderer mode conflict encountered while acquiring a font-atlas capability.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum FontAtlasModeError {
    /// A managed renderer already owns this atlas.
    #[error("the font atlas is owned by a managed renderer")]
    ManagedRendererActive,
    /// The previous managed renderer has not committed texture release yet.
    #[error("the font atlas is waiting for the previous renderer texture release to be committed")]
    RendererReleasePending,
}

/// Failure to change the atlas-wide font loader.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum FontAtlasLoaderError {
    /// Dear ImGui has already retained one or more font sources.
    #[error(
        "the font atlas already contains {source_count} font source(s); set the loader before adding fonts"
    )]
    SourcesAlreadyAdded { source_count: usize },
}

/// Failure to add a font source to an atlas.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum FontLoadError {
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
    /// The source falls outside the validated stb_truetype subset.
    #[error(transparent)]
    StbTrueType(#[from] super::validated::StbTrueTypeFontError),
    /// The FreeType source does not have a supported SFNT header and table directory.
    #[error("expected a supported TrueType or OpenType SFNT face")]
    InvalidSfnt,
    /// The fixed native FreeType wrapper cannot consume the table's glyph output safely.
    #[error("the checked FreeType route does not accept {format} glyphs")]
    UnsupportedFreeTypeGlyphs { format: &'static str },
    /// The atlas-owned font data allocation failed.
    #[error("Dear ImGui could not allocate memory for the font source")]
    AllocationFailed,
    /// The native loader rejected the source.
    #[error("Dear ImGui's font loader rejected the source")]
    NativeRejected,
}
