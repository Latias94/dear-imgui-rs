use std::ops::Deref;

use super::core::FontAtlas;
use super::error::FontAtlasModeError;
use super::state::{claim_font_atlas_legacy_renderer, release_font_atlas_legacy_renderer};

/// Legacy renderer capability for a font atlas.
///
/// The common [`FontAtlas`] view is intentionally limited to font configuration and managed
/// texture state. Acquire this capability before using a renderer-owned CPU atlas texture or
/// calling [`LegacyFontAtlas::build`]. Acquiring it claims legacy renderer mode immediately, so a
/// managed renderer conflict is reported at this call instead of being deferred to the next
/// frame.
#[must_use = "dropping this capability releases its active lease but preserves legacy renderer mode until the atlas is fully cleared"]
#[derive(Debug)]
pub struct LegacyFontAtlas<'atlas> {
    pub(super) atlas: &'atlas FontAtlas,
}

impl<'atlas> LegacyFontAtlas<'atlas> {
    /// Borrow the common font-configuration capability.
    pub fn atlas(&self) -> &'atlas FontAtlas {
        self.atlas
    }
}

impl Deref for LegacyFontAtlas<'_> {
    type Target = FontAtlas;

    fn deref(&self) -> &Self::Target {
        self.atlas
    }
}

impl Drop for LegacyFontAtlas<'_> {
    fn drop(&mut self) {
        release_font_atlas_legacy_renderer(self.atlas.raw());
    }
}

pub(super) fn claim_legacy_renderer(
    atlas: &FontAtlas,
) -> Result<LegacyFontAtlas<'_>, FontAtlasModeError> {
    claim_font_atlas_legacy_renderer(atlas.raw())?;
    Ok(LegacyFontAtlas { atlas })
}
