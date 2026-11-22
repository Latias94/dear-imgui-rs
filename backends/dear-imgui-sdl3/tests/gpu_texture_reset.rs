#![cfg(feature = "sdlgpu3-renderer")]

use dear_imgui_rs::texture::{OwnedTextureData, TextureFormat};
use dear_imgui_rs::{Context, TextureStatus};
use dear_imgui_sdl3::{SdlGpu3InitInfo, SdlGpu3RendererBackend};
use sdl3::gpu::{Device, ShaderFormat};

#[test]
#[ignore = "requires a working SDL video driver and GPU device"]
fn recreating_device_objects_reuploads_same_size_textures() -> Result<(), Box<dyn std::error::Error>>
{
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let window = video
        .window("SDL GPU texture reset", 200, 120)
        .hidden()
        .build()?;
    let formats =
        ShaderFormat::SPIRV | ShaderFormat::DXIL | ShaderFormat::MSL | ShaderFormat::METALLIB;
    let device = Device::new(formats, false)?.with_window(&window)?;
    let mut context = Context::create();
    context.set_ini_filename::<std::path::PathBuf>(None)?;
    let init = SdlGpu3InitInfo::from_window(&device, &window);
    // The window and device outlive the renderer and its context, including on early return.
    let mut renderer = unsafe { SdlGpu3RendererBackend::init(&mut context, &window, init)? };
    let image = context.register_texture(OwnedTextureData::from_pixels(
        TextureFormat::RGBA32,
        8,
        8,
        &[255; 8 * 8 * 4],
    )?);

    for rebuild in [false, true] {
        if rebuild {
            renderer.create_device_objects(&mut context)?;
            assert!(context.with_texture(image, |tex| tex.texture_id().is_null())?);
        }
        renderer.new_frame(&mut context)?;
        let frame = context.begin_frame();
        frame.ui().text("Font atlas upload");
        frame.ui().image(image, [8.0, 8.0]);
        renderer.prepare(frame)?.skip_main();
        context.with_texture(image, |tex| {
            assert_eq!(tex.status(), TextureStatus::OK);
            assert!(!tex.texture_id().is_null());
        })?;
    }
    renderer.shutdown(&mut context)?;
    Ok(())
}
