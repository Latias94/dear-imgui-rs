use super::id::validate_font_id_for_atlas;
use super::validation::RASTERIZER_MULTIPLY_MAX;
use super::*;

fn reconcile_with_retry(
    frame: crate::render::PendingFrame<'_>,
) -> crate::render::ReconciledFrame<'_> {
    let feedback = frame
        .texture_requests()
        .iter()
        .map(crate::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    frame
        .reconcile_texture_feedback(feedback)
        .expect("explicit retry outcomes must reconcile the frame")
}

mod mode;
mod shared;
mod source_validation;
mod texture;
