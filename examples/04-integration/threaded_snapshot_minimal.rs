//! Minimal threaded snapshot example (no actual GPU renderer).
//!
//! This demonstrates how to:
//! - build UI on the main thread
//! - create a `Send + Sync` `FrameSnapshot`
//! - send it to a "render thread"
//! - commit request-bound feedback through the snapshot's Context channel
//!
//! Run:
//! `cargo run -p dear-imgui-examples --bin threaded_snapshot_minimal`

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use dear_imgui_rs::BackendFlags;
use dear_imgui_rs::Context;
use dear_imgui_rs::render::snapshot::{
    FrameSnapshot, SnapshotTextureId, TextureBinding, TextureOp,
};
use dear_imgui_rs::texture::{TextureFormat, TextureId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::create();
    ctx.set_ini_filename(None::<String>).unwrap();

    // Minimal IO setup (headless).
    ctx.io_mut().set_display_size([800.0, 600.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    let flags = ctx.io().backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES;
    ctx.io_mut().set_backend_flags(flags);

    // Create and register a managed texture so the renderer receives owned upload requests.
    let managed_tex = dear_imgui_rs::texture::OwnedTextureData::from_pixels(
        TextureFormat::RGBA32,
        2,
        2,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )?;
    let managed_tex = ctx.register_texture(managed_tex);
    let consumer = ctx
        .create_detached_renderer_consumer()
        .expect("the detached renderer consumer should attach");

    let (snapshot_tx, snapshot_rx) = mpsc::channel::<FrameSnapshot>();
    let (completion_tx, completion_rx) = mpsc::channel::<()>();

    let render_thread = thread::spawn(move || render_thread_main(snapshot_rx, completion_tx));

    for frame_idx in 0..3 {
        // Frame 1: request a full replacement (simulated).
        if frame_idx == 1 {
            ctx.try_with_texture_mut(managed_tex, |mut texture| {
                texture.replace_pixels(&[
                    0, 0, 0, 255, 255, 0, 255, 255, //
                    0, 255, 255, 255, 255, 255, 0, 255,
                ])
            })?;
        } else if frame_idx == 2 {
            ctx.remove_texture(managed_tex)
                .expect("the final frame should begin texture retirement");
        }

        let frame = ctx.begin_frame();
        let ui = frame.ui();
        ui.window("Threaded Snapshot")
            .size([360.0, 120.0], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Frame: {frame_idx}"));
                ui.text("This example does not render to a GPU.");
                if frame_idx < 2 {
                    ui.image(managed_tex, [64.0, 64.0]);
                } else {
                    ui.text("The managed texture is retiring.");
                }
            });

        let snapshot = frame
            .render_snapshot(&consumer)
            .expect("snapshot build failed");
        snapshot_tx.send(snapshot).unwrap();
        completion_rx.recv().unwrap();
        let progress = ctx
            .poll_snapshot_completions()
            .expect("renderer completion should match its Context epoch");
        println!(
            "[ui] completed through epoch {}, applied {} feedback item(s)",
            progress.watermark(),
            progress.feedback_applied()
        );
    }

    drop(snapshot_tx);
    let _ = render_thread.join();
    drop(consumer);
    ctx.poll_snapshot_completions().unwrap();
    Ok(())
}

fn render_thread_main(snapshot_rx: mpsc::Receiver<FrameSnapshot>, completion_tx: mpsc::Sender<()>) {
    let mut next_tex_id: u64 = 1;
    let mut managed_map: HashMap<SnapshotTextureId, TextureId> = HashMap::new();

    while let Ok(snapshot) = snapshot_rx.recv() {
        let mut feedback = Vec::new();

        for request in snapshot.texture_requests() {
            match request.operation() {
                TextureOp::Create { .. } => {
                    let tex_id = *managed_map.entry(request.texture()).or_insert_with(|| {
                        let tex_id = TextureId::new(next_tex_id);
                        next_tex_id += 1;
                        tex_id
                    });
                    feedback.push(request.uploaded(tex_id).unwrap());
                }
                TextureOp::Update { .. } => {
                    if let Some(tex_id) = managed_map.get(&request.texture()).copied() {
                        feedback.push(request.uploaded(tex_id).unwrap());
                    } else {
                        feedback.push(request.retry());
                    }
                }
                TextureOp::Destroy => {
                    managed_map.remove(&request.texture());
                    feedback.push(request.destroyed().unwrap());
                }
            }
        }

        let mut elements = 0usize;
        let mut legacy = 0usize;
        let mut managed = 0usize;

        for dl in &snapshot.draw_data().draw_lists {
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

        snapshot.commit(feedback).unwrap();
        completion_tx.send(()).unwrap();
    }
}
