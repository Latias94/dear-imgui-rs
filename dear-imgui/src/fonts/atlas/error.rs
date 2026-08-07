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
