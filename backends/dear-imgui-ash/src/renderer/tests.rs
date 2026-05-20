use super::texture::{TextureWriteback, texture_data_to_rgba_subrect};
use super::{TextureId, TextureStatus};
use dear_imgui_rs::texture::{TextureData, TextureFormat as ImFormat};

#[test]
fn texture_subrect_rgba32() {
    let mut tex = TextureData::new();
    tex.create(ImFormat::RGBA32, 2, 2);
    let pixels: [u8; 16] = [
        10, 20, 30, 40, // (0,0)
        50, 60, 70, 80, // (1,0)
        90, 100, 110, 120, // (0,1)
        130, 140, 150, 160, // (1,1)
    ];
    tex.set_data(&pixels);

    let out = texture_data_to_rgba_subrect(&tex, 1, 0, 1, 1).unwrap();
    assert_eq!(out, vec![50, 60, 70, 80]);
}

#[test]
fn texture_subrect_alpha8() {
    let mut tex = TextureData::new();
    tex.create(ImFormat::Alpha8, 2, 2);
    let alphas: [u8; 4] = [0, 64, 128, 255];
    tex.set_data(&alphas);

    let out = texture_data_to_rgba_subrect(&tex, 0, 1, 2, 1).unwrap();
    assert_eq!(
        out,
        vec![
            255, 255, 255, 128, //
            255, 255, 255, 255,
        ]
    );
}

#[test]
fn texture_writeback_created_sets_tex_id_and_status() {
    let mut tex = TextureData::new();
    tex.create(ImFormat::RGBA32, 1, 1);

    TextureWriteback {
        texture: tex.as_raw_mut(),
        tex_id: Some(TextureId::from(7u64)),
        status: TextureStatus::OK,
    }
    .apply();

    assert_eq!(tex.tex_id(), TextureId::from(7u64));
    assert_eq!(tex.status(), TextureStatus::OK);
}

#[test]
fn texture_writeback_destroyed_sets_destroy_next_frame_and_status() {
    let mut tex = TextureData::new();
    tex.create(ImFormat::RGBA32, 1, 1);

    TextureWriteback {
        texture: tex.as_raw_mut(),
        tex_id: None,
        status: TextureStatus::Destroyed,
    }
    .apply();

    assert_eq!(tex.status(), TextureStatus::Destroyed);
    unsafe {
        assert!((*tex.as_raw()).WantDestroyNextFrame);
    }
}
