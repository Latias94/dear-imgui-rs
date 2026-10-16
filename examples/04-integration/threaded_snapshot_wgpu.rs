//! Bounded UI-to-render-thread handoff with a real offscreen WGPU executor.
//!
//! Run `cargo run -p dear-imgui-examples --bin threaded_snapshot_wgpu`.
//! The worker records UI and an engine producer separately, submits producer before UI, and
//! returns CPU recording evidence. This headless example does not request native presentation.

use std::{sync::mpsc, thread};

use dear_imgui_rs::{Color, Context};
use dear_imgui_wgpu::{
    FramebufferExtent, GammaMode, WgpuInitInfo, WgpuSnapshotExecutor, WgpuSnapshotRenderer,
    WgpuSnapshotReport, WgpuSnapshotSubmission, WgpuSnapshotTarget, wgpu,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let mut context = Context::create();
    context.set_ini_filename(None::<String>)?;
    context.io_mut().set_display_size([64.0, 32.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        &mut context,
    )?;
    executor.set_gamma_mode(GammaMode::Linear);
    let target = executor.set_target(context.main_viewport().id(), None)?;
    let scene = color_texture(
        &device,
        "engine scene",
        [1, 1],
        wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let scene_view = scene.create_view(&Default::default());
    let scene_binding = executor.register_external_texture(&scene_view)?;

    // Capacity one supplies backpressure; neither Context nor the UI consumer crosses threads.
    let (frames_tx, frames_rx) = mpsc::sync_channel::<WgpuSnapshotSubmission>(1);
    let (results_tx, results_rx) = mpsc::sync_channel::<Result<WgpuSnapshotReport, String>>(1);
    let worker = thread::spawn(move || {
        render_worker(
            executor, device, queue, scene_view, target, frames_rx, results_tx,
        )
    });
    for _ in 0..3 {
        let frame = context.begin_frame();
        frame.ui().get_foreground_draw_list().add_image(
            scene_binding.texture_id(),
            [0.0, 0.0],
            [64.0, 32.0],
            [0.0, 0.0],
            [1.0, 1.0],
            Color::WHITE,
        );
        let snapshot = frame.render_snapshot(owner.renderer_consumer()?)?;
        frames_tx.send(WgpuSnapshotSubmission::new(snapshot, [target]))?;
        let report = results_rx.recv()?.map_err(std::io::Error::other)?;
        context.poll_snapshot_completions()?;
        println!(
            "processed epoch {}: {} recorded target(s)",
            report.epoch.sequence(),
            report
                .targets
                .iter()
                .filter(|target| target.draw.recorded)
                .count()
        );
    }
    drop(frames_tx);
    let mut executor = worker.join().map_err(|_| "render worker panicked")?;
    owner.shutdown(&mut context, &mut executor)?;
    Ok(())
}

fn color_texture(
    device: &wgpu::Device,
    label: &str,
    size: [u32; 2],
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: usage | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn render_worker(
    mut executor: WgpuSnapshotExecutor,
    device: wgpu::Device,
    queue: wgpu::Queue,
    scene_view: wgpu::TextureView,
    target: WgpuSnapshotTarget,
    frames: mpsc::Receiver<WgpuSnapshotSubmission>,
    results: mpsc::SyncSender<Result<WgpuSnapshotReport, String>>,
) -> WgpuSnapshotExecutor {
    let output = color_texture(
        &device,
        "offscreen UI",
        [64, 32],
        wgpu::TextureUsages::empty(),
    );
    let output_view = output.create_view(&Default::default());
    let mut last_submission = None;
    while let Ok(submission) = frames.recv() {
        let result = (|| -> Result<WgpuSnapshotReport, String> {
            let mut prepared = executor
                .prepare(submission)
                .map_err(|error| error.to_string())?;
            let mut ui_commands = device.create_command_encoder(&Default::default());
            {
                let attachments = [Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })];
                let mut pass = ui_commands.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &attachments,
                    ..Default::default()
                });
                // Finish even on recording failure so its detailed cause, report, and feedback survive.
                let _ = prepared.record(target, &mut pass, FramebufferExtent::new(64, 32));
            }
            let report = prepared.finish().map_err(|error| error.to_string())?;
            let mut producer = device.create_command_encoder(&Default::default());
            {
                let attachments = [Some(wgpu::RenderPassColorAttachment {
                    view: &scene_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: wgpu::StoreOp::Store,
                    },
                })];
                let _pass = producer.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &attachments,
                    ..Default::default()
                });
            }
            last_submission = Some(queue.submit([producer.finish(), ui_commands.finish()]));
            Ok(report)
        })();
        if results.send(result).is_err() {
            break;
        }
    }
    // This wait belongs to final GPU teardown, not the CPU snapshot completion protocol.
    if let Some(submission) = last_submission {
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(10)),
        });
    }
    executor
}
