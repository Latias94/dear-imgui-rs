//! Thread-safe rendering snapshot.
//!
//! This module provides `Send + Sync` data structures which capture everything a renderer backend
//! needs to render a frame, without retaining any pointers into ImGui-owned memory.

use crate::render::draw_data::{DrawData, DrawIdx, DrawList, DrawVert};
use crate::sys;
use crate::texture::{TextureFormat, TextureId, TextureRect, TextureStatus};
use thiserror::Error;

/// A stable identifier for ImGui-managed textures (`ImTextureData.UniqueID`).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ManagedTextureId(pub i32);

/// How a draw command binds its texture.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TextureBinding {
    /// Legacy texture binding (plain handle).
    Legacy(TextureId),
    /// ImGui-managed texture binding, resolved by `ManagedTextureId`.
    Managed(ManagedTextureId),
}

/// A thread-safe snapshot of everything needed to render a frame.
#[derive(Clone, Debug)]
pub struct FrameSnapshot {
    pub draw: DrawDataSnapshot,
    pub texture_requests: Vec<TextureRequest>,
}

/// Options controlling snapshot behavior.
#[derive(Copy, Clone, Debug)]
pub struct SnapshotOptions {
    pub user_callback_policy: UserCallbackPolicy,
    pub capture_texture_requests: bool,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            user_callback_policy: UserCallbackPolicy::Error,
            capture_texture_requests: true,
        }
    }
}

/// Policy for `ImDrawCmd::UserCallback` commands when snapshotting.
#[derive(Copy, Clone, Debug, Default)]
pub enum UserCallbackPolicy {
    /// Return an error when encountering a user callback (default).
    #[default]
    Error,
    /// Drop callback commands from the snapshot.
    Drop,
}

/// Errors that can occur when building a snapshot.
#[derive(Error, Debug)]
pub enum SnapshotError {
    #[error("user callback commands are not supported in the snapshot path")]
    UserCallbackUnsupported,

    #[error("managed texture {id:?} has status {status:?} but no pixel buffer is available")]
    TexturePixelsMissing {
        id: ManagedTextureId,
        status: TextureStatus,
    },

    #[error(
        "managed texture {id:?} has invalid dimensions/format (width={width}, height={height}, bpp={bpp})"
    )]
    TextureInvalidLayout {
        id: ManagedTextureId,
        width: i32,
        height: i32,
        bpp: i32,
    },
}

/// Thread-safe draw data snapshot.
#[derive(Clone, Debug)]
pub struct DrawDataSnapshot {
    pub display_pos: [f32; 2],
    pub display_size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
    pub draw_lists: Vec<DrawListSnapshot>,
}

/// Thread-safe draw list snapshot.
#[derive(Clone, Debug)]
pub struct DrawListSnapshot {
    pub vtx: Vec<DrawVert>,
    pub idx: Vec<DrawIdx>,
    pub commands: Vec<DrawCmdSnapshot>,
}

/// Thread-safe draw command snapshot.
#[derive(Clone, Debug)]
pub enum DrawCmdSnapshot {
    Elements {
        count: usize,
        clip_rect: [f32; 4],
        texture: TextureBinding,
        vtx_offset: usize,
        idx_offset: usize,
    },
    ResetRenderState,
}

/// A thread-safe managed texture request (ImGui 1.92+).
#[derive(Clone, Debug)]
pub struct TextureRequest {
    pub id: ManagedTextureId,
    pub op: TextureOp,
}

/// Feedback produced by the renderer thread, to be applied on the UI thread.
#[derive(Copy, Clone, Debug)]
pub struct TextureFeedback {
    pub id: ManagedTextureId,
    pub status: TextureStatus,
    pub tex_id: Option<TextureId>,
}

/// A managed texture operation requested by ImGui.
#[derive(Clone, Debug)]
pub enum TextureOp {
    Create {
        format: TextureFormat,
        width: i32,
        height: i32,
        row_pitch: i32,
        pixels: Vec<u8>,
    },
    Update {
        format: TextureFormat,
        width: i32,
        height: i32,
        rects: Vec<TextureUploadRect>,
    },
    Destroy,
}

/// A tightly-packed pixel upload for a sub-rectangle of a managed texture.
#[derive(Clone, Debug)]
pub struct TextureUploadRect {
    pub rect: TextureRect,
    pub row_pitch: i32,
    pub data: Vec<u8>,
}

impl FrameSnapshot {
    /// Build a `FrameSnapshot` from ImGui draw data.
    ///
    /// This must be called on the UI thread while the `Context` is current.
    pub fn from_draw_data(
        draw_data: &DrawData,
        options: SnapshotOptions,
    ) -> Result<Self, SnapshotError> {
        let draw = snapshot_draw_data(draw_data, options)?;
        let texture_requests = if options.capture_texture_requests {
            snapshot_texture_requests(draw_data)?
        } else {
            Vec::new()
        };
        Ok(Self {
            draw,
            texture_requests,
        })
    }
}

fn snapshot_draw_data(
    draw_data: &DrawData,
    options: SnapshotOptions,
) -> Result<DrawDataSnapshot, SnapshotError> {
    let mut draw_lists = Vec::with_capacity(draw_data.draw_lists_count());
    for draw_list in draw_data.draw_lists() {
        draw_lists.push(snapshot_draw_list(draw_list, options)?);
    }

    Ok(DrawDataSnapshot {
        display_pos: draw_data.display_pos(),
        display_size: draw_data.display_size(),
        framebuffer_scale: draw_data.framebuffer_scale(),
        draw_lists,
    })
}

fn snapshot_draw_list(
    draw_list: &DrawList,
    options: SnapshotOptions,
) -> Result<DrawListSnapshot, SnapshotError> {
    let vtx = draw_list.vtx_buffer().to_vec();
    let idx = draw_list.idx_buffer().to_vec();

    let mut commands = Vec::new();
    for cmd in unsafe { draw_list.cmd_buffer() } {
        if let Some(cb) = cmd.UserCallback {
            if cb as usize == usize::MAX {
                commands.push(DrawCmdSnapshot::ResetRenderState);
                continue;
            }

            match options.user_callback_policy {
                UserCallbackPolicy::Error => return Err(SnapshotError::UserCallbackUnsupported),
                UserCallbackPolicy::Drop => continue,
            }
        }

        let texture = snapshot_texture_binding(cmd.TexRef);
        commands.push(DrawCmdSnapshot::Elements {
            count: cmd.ElemCount as usize,
            clip_rect: [
                cmd.ClipRect.x,
                cmd.ClipRect.y,
                cmd.ClipRect.z,
                cmd.ClipRect.w,
            ],
            texture,
            vtx_offset: cmd.VtxOffset as usize,
            idx_offset: cmd.IdxOffset as usize,
        });
    }

    Ok(DrawListSnapshot { vtx, idx, commands })
}

fn snapshot_texture_binding(tex_ref: sys::ImTextureRef) -> TextureBinding {
    if tex_ref._TexID != 0 {
        return TextureBinding::Legacy(TextureId::from(tex_ref._TexID as u64));
    }

    if !tex_ref._TexData.is_null() {
        let unique_id = unsafe { (*tex_ref._TexData).UniqueID };
        return TextureBinding::Managed(ManagedTextureId(unique_id));
    }

    TextureBinding::Legacy(TextureId::null())
}

fn snapshot_texture_requests(draw_data: &DrawData) -> Result<Vec<TextureRequest>, SnapshotError> {
    let mut out = Vec::new();

    for tex in draw_data.textures() {
        let status = tex.status();
        if status == TextureStatus::OK || status == TextureStatus::Destroyed {
            continue;
        }

        let id = ManagedTextureId(tex.unique_id());
        let width = tex.width();
        let height = tex.height();
        let bpp = tex.bytes_per_pixel();
        if width <= 0 || height <= 0 || bpp <= 0 {
            return Err(SnapshotError::TextureInvalidLayout {
                id,
                width,
                height,
                bpp,
            });
        }
        let format = tex.format();

        match status {
            TextureStatus::WantCreate => {
                let pixels = tex
                    .pixels()
                    .ok_or(SnapshotError::TexturePixelsMissing { id, status })?;
                let expected = usize::try_from(width)
                    .ok()
                    .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
                    .and_then(|px| usize::try_from(bpp).ok().and_then(|b| px.checked_mul(b)));
                let Some(expected) = expected else {
                    return Err(SnapshotError::TextureInvalidLayout {
                        id,
                        width,
                        height,
                        bpp,
                    });
                };
                if pixels.len() < expected {
                    return Err(SnapshotError::TextureInvalidLayout {
                        id,
                        width,
                        height,
                        bpp,
                    });
                }
                out.push(TextureRequest {
                    id,
                    op: TextureOp::Create {
                        format,
                        width,
                        height,
                        row_pitch: width.saturating_mul(bpp),
                        pixels: pixels[..expected].to_vec(),
                    },
                });
            }
            TextureStatus::WantUpdates => {
                let pixels = tex
                    .pixels()
                    .ok_or(SnapshotError::TexturePixelsMissing { id, status })?;
                let expected = usize::try_from(width)
                    .ok()
                    .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
                    .and_then(|px| usize::try_from(bpp).ok().and_then(|b| px.checked_mul(b)));
                let Some(expected) = expected else {
                    return Err(SnapshotError::TextureInvalidLayout {
                        id,
                        width,
                        height,
                        bpp,
                    });
                };
                if pixels.len() < expected {
                    return Err(SnapshotError::TextureInvalidLayout {
                        id,
                        width,
                        height,
                        bpp,
                    });
                }

                let mut rects: Vec<TextureRect> = tex.updates().collect();
                if rects.is_empty() {
                    let r = tex.update_rect();
                    if r.w != 0 && r.h != 0 {
                        rects.push(r);
                    } else {
                        rects.push(TextureRect {
                            x: 0,
                            y: 0,
                            w: width.min(u16::MAX as i32) as u16,
                            h: height.min(u16::MAX as i32) as u16,
                        });
                    }
                }

                let rects = rects
                    .into_iter()
                    .filter_map(|r| copy_texture_rect(pixels, width, height, bpp, r))
                    .collect::<Vec<_>>();

                out.push(TextureRequest {
                    id,
                    op: TextureOp::Update {
                        format,
                        width,
                        height,
                        rects,
                    },
                });
            }
            TextureStatus::WantDestroy => {
                out.push(TextureRequest {
                    id,
                    op: TextureOp::Destroy,
                });
            }
            TextureStatus::OK | TextureStatus::Destroyed => {}
        }
    }

    Ok(out)
}

fn copy_texture_rect(
    pixels: &[u8],
    width: i32,
    height: i32,
    bpp: i32,
    rect: TextureRect,
) -> Option<TextureUploadRect> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let bpp = usize::try_from(bpp).ok()?;
    if width == 0 || height == 0 || bpp == 0 {
        return None;
    }

    let x = usize::from(rect.x);
    let y = usize::from(rect.y);
    let w = usize::from(rect.w);
    let h = usize::from(rect.h);
    if w == 0 || h == 0 {
        return None;
    }

    let x_end = x.saturating_add(w).min(width);
    let y_end = y.saturating_add(h).min(height);
    if x >= x_end || y >= y_end {
        return None;
    }

    let rect_w = x_end - x;
    let rect_h = y_end - y;

    let full_row_pitch = width.checked_mul(bpp)?;
    let rect_row_pitch = rect_w.checked_mul(bpp)?;
    let needed_size = rect_row_pitch.checked_mul(rect_h)?;

    let mut out = vec![0u8; needed_size];
    for row in 0..rect_h {
        let src_row = y.checked_add(row)?;
        let src_off = src_row
            .checked_mul(full_row_pitch)?
            .checked_add(x.checked_mul(bpp)?)?;
        let dst_off = row.checked_mul(rect_row_pitch)?;
        let src_end = src_off.checked_add(rect_row_pitch)?;
        let dst_end = dst_off.checked_add(rect_row_pitch)?;
        if src_end > pixels.len() || dst_end > out.len() {
            return None;
        }
        out[dst_off..dst_end].copy_from_slice(&pixels[src_off..src_end]);
    }

    Some(TextureUploadRect {
        rect: TextureRect {
            x: rect.x,
            y: rect.y,
            w: rect_w.min(u16::MAX as usize) as u16,
            h: rect_h.min(u16::MAX as usize) as u16,
        },
        row_pitch: rect_row_pitch.min(i32::MAX as usize) as i32,
        data: out,
    })
}
