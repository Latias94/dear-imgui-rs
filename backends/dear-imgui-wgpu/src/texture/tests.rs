use super::result::mark_texture_destroyed;
use super::*;
use dear_imgui_rs::texture::{TextureData, TextureFormat as ImFormat, TextureRect};

#[test]
fn texture_update_result_apply_to_sets_status_and_id() {
    let mut tex = TextureData::new();

    // Created -> sets TexID and OK
    TextureUpdateResult::Created {
        texture_id: TextureId::from(42u64),
    }
    .apply_to(&mut tex);
    assert_eq!(tex.status(), TextureStatus::OK);
    assert_eq!(tex.tex_id().id(), 42);

    // Updated -> only status OK
    TextureUpdateResult::Updated.apply_to(&mut tex);
    assert_eq!(tex.status(), TextureStatus::OK);
    assert_eq!(tex.tex_id().id(), 42);

    // Destroyed -> status Destroyed
    // `apply_to` owns the ImTextureData status writeback, including Dear ImGui's
    // WantDestroyNextFrame precondition.
    TextureUpdateResult::Destroyed.apply_to(&mut tex);
    assert_eq!(tex.status(), TextureStatus::Destroyed);
    unsafe {
        assert!((*tex.as_raw()).WantDestroyNextFrame);
    }

    // Failed -> also marks Destroyed
    // In the general case (not a requested destroy), SetStatus(Destroyed) translates to WantCreate.
    unsafe {
        (*tex.as_raw_mut()).WantDestroyNextFrame = false;
    }
    tex.create(ImFormat::RGBA32, 1, 1);
    TextureUpdateResult::Failed.apply_to(&mut tex);
    assert_eq!(tex.status(), TextureStatus::WantCreate);

    // NoAction -> leaves state unchanged
    TextureUpdateResult::NoAction.apply_to(&mut tex);
    assert_eq!(tex.status(), TextureStatus::WantCreate);
}

#[test]
fn mark_texture_destroyed_sets_destroy_next_frame_and_status() {
    let mut tex = TextureData::new();
    tex.create(ImFormat::RGBA32, 1, 1);

    mark_texture_destroyed(&mut tex);
    assert_eq!(tex.status(), TextureStatus::Destroyed);
    unsafe {
        assert!((*tex.as_raw()).WantDestroyNextFrame);
    }
}

#[test]
fn convert_subrect_to_rgba_rgba32_full_rect() {
    let mut tex = TextureData::new();
    let width = 2;
    let height = 2;
    tex.create(ImFormat::RGBA32, width, height);

    // 2x2 RGBA pixels: row-major
    let pixels: [u8; 16] = [
        10, 20, 30, 40, // (0,0)
        50, 60, 70, 80, // (1,0)
        90, 100, 110, 120, // (0,1)
        130, 140, 150, 160, // (1,1)
    ];
    tex.set_data(&pixels);

    let rect = TextureRect {
        x: 0,
        y: 0,
        w: width as u16,
        h: height as u16,
    };

    let out = WgpuTextureManager::convert_subrect_to_rgba(&tex, rect).expect("expected data");
    assert_eq!(out, pixels);
}

#[test]
fn convert_subrect_to_rgba_alpha8_full_rect() {
    let mut tex = TextureData::new();
    let width = 2;
    let height = 2;
    tex.create(ImFormat::Alpha8, width, height);

    // 2x2 alpha-only pixels
    let alphas: [u8; 4] = [0, 64, 128, 255];
    tex.set_data(&alphas);

    let rect = TextureRect {
        x: 0,
        y: 0,
        w: width as u16,
        h: height as u16,
    };

    let out = WgpuTextureManager::convert_subrect_to_rgba(&tex, rect).expect("expected data");
    // Each alpha should expand to [255,255,255,a]
    assert_eq!(
        out,
        vec![
            255, 255, 255, 0, // a=0
            255, 255, 255, 64, // a=64
            255, 255, 255, 128, // a=128
            255, 255, 255, 255, // a=255
        ]
    );
}

#[test]
fn convert_subrect_to_rgba_out_of_bounds_returns_none() {
    let mut tex = TextureData::new();
    tex.create(ImFormat::RGBA32, 2, 2);
    let rect = TextureRect {
        x: 10,
        y: 10,
        w: 1,
        h: 1,
    };
    assert!(WgpuTextureManager::convert_subrect_to_rgba(&tex, rect).is_none());
}
