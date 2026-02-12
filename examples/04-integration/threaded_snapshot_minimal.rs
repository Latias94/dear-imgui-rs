//! Minimal threaded snapshot example (no actual GPU renderer).
//!
//! This demonstrates how to:
//! - build UI on the main thread
//! - create a `Send + Sync` `FrameSnapshot`
//! - send it to a "render thread"
//! - return `TextureFeedback` and apply it on the UI thread
//!
//! Run:
//! `cargo run -p dear-imgui-examples --bin threaded_snapshot_minimal`

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use dear_imgui_rs::BackendFlags;
use dear_imgui_rs::Context;
use dear_imgui_rs::render::snapshot::{
    FrameSnapshot, ManagedTextureId, SnapshotOptions, TextureBinding, TextureFeedback, TextureOp,
};
use dear_imgui_rs::texture::{TextureFormat, TextureId, TextureStatus};

fn main() {
    let mut ctx = Context::create();
    ctx.set_ini_filename(None::<String>).unwrap();

    // Minimal IO setup (headless).
    ctx.io_mut().set_display_size([800.0, 600.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    let flags = ctx.io().backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES;
    ctx.io_mut().set_backend_flags(flags);

    // Create and register a managed texture so `DrawData::textures()` has something to request.
    let mut managed_tex = dear_imgui_rs::texture::TextureData::new();
    managed_tex.create(TextureFormat::RGBA32, 2, 2);
    managed_tex.set_data(&[
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 255, 255,
    ]);
    managed_tex.set_status(TextureStatus::WantCreate);
    ctx.register_user_texture(&mut *managed_tex);

    let (snapshot_tx, snapshot_rx) = mpsc::channel::<FrameSnapshot>();
    let (feedback_tx, feedback_rx) = mpsc::channel::<Vec<TextureFeedback>>();

    let render_thread = thread::spawn(move || render_thread_main(snapshot_rx, feedback_tx));

    let mut pending_feedback: Vec<TextureFeedback> = Vec::new();

    for frame_idx in 0..2 {
        // Apply feedback from the previous frame.
        if !pending_feedback.is_empty() {
            let applied = ctx
                .platform_io_mut()
                .apply_texture_feedback(&pending_feedback);
            println!("[ui] applied {applied} feedback items");
            pending_feedback.clear();
        }

        // Frame 1: request a partial update (simulated).
        if frame_idx == 1 {
            managed_tex.set_data(&[
                0, 0, 0, 255, 255, 0, 255, 255, //
                0, 255, 255, 255, 255, 255, 0, 255,
            ]);
        }

        let ui = ctx.frame();
        ui.window("Threaded Snapshot")
            .size([360.0, 120.0], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Frame: {frame_idx}"));
                ui.text("This example does not render to a GPU.");
                ui.image(&mut *managed_tex, [64.0, 64.0]);
            });

        let draw_data = ctx.render();
        let snapshot = FrameSnapshot::from_draw_data(draw_data, SnapshotOptions::default())
            .expect("snapshot build failed");

        snapshot_tx.send(snapshot).unwrap();
        pending_feedback = feedback_rx.recv().unwrap();
    }

    drop(snapshot_tx);
    let _ = render_thread.join();
}

fn render_thread_main(
    snapshot_rx: mpsc::Receiver<FrameSnapshot>,
    feedback_tx: mpsc::Sender<Vec<TextureFeedback>>,
) {
    let mut next_tex_id: u64 = 1;
    let mut managed_map: HashMap<ManagedTextureId, TextureId> = HashMap::new();

    while let Ok(snapshot) = snapshot_rx.recv() {
        let mut feedback = Vec::new();

        for req in &snapshot.texture_requests {
            match &req.op {
                TextureOp::Create { .. } => {
                    let tex_id = TextureId::new(next_tex_id);
                    next_tex_id += 1;
                    managed_map.insert(req.id, tex_id);
                    feedback.push(TextureFeedback {
                        id: req.id,
                        status: TextureStatus::OK,
                        tex_id: Some(tex_id),
                    });
                }
                TextureOp::Update { .. } => {
                    feedback.push(TextureFeedback {
                        id: req.id,
                        status: TextureStatus::OK,
                        tex_id: None,
                    });
                }
                TextureOp::Destroy => {
                    managed_map.remove(&req.id);
                    feedback.push(TextureFeedback {
                        id: req.id,
                        status: TextureStatus::Destroyed,
                        tex_id: None,
                    });
                }
            }
        }

        let mut elements = 0usize;
        let mut legacy = 0usize;
        let mut managed = 0usize;

        for dl in &snapshot.draw.draw_lists {
            for cmd in &dl.commands {
                let dear_imgui_rs::render::snapshot::DrawCmdSnapshot::Elements { texture, .. } =
                    cmd
                else {
                    continue;
                };
                elements += 1;
                match *texture {
                    TextureBinding::Legacy(_) => legacy += 1,
                    TextureBinding::Managed(id) => {
                        managed += 1;
                        let _resolved = managed_map.get(&id).copied().unwrap_or(TextureId::null());
                    }
                }
            }
        }

        println!(
            "[render] commands(elements={elements}, legacy={legacy}, managed={managed}), managed_textures={}",
            managed_map.len()
        );

        feedback_tx.send(feedback).unwrap();
    }
}
