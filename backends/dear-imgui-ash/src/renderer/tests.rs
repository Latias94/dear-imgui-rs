use super::texture::{texture_data_to_rgba_subrect, texture_upload_to_rgba};
use dear_imgui_rs::texture::{OwnedTextureData, TextureFormat as ImFormat};

#[test]
fn texture_subrect_rgba32() {
    let mut tex = OwnedTextureData::new();
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
    let mut tex = OwnedTextureData::new();
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
fn request_upload_rgba32_honors_row_pitch() {
    let pixels = [
        10, 20, 30, 40, 50, 60, 70, 80, 1, 2, 3, 4, // row 0 plus padding
        90, 100, 110, 120, 130, 140, 150, 160, 5, 6, 7, 8, // row 1 plus padding
    ];

    let out = texture_upload_to_rgba(ImFormat::RGBA32, 2, 2, 12, &pixels).unwrap();
    assert_eq!(
        out,
        vec![
            10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
        ]
    );
}

#[test]
fn request_upload_alpha8_honors_row_pitch() {
    let pixels = [10, 20, 99, 30, 40, 88];

    let out = texture_upload_to_rgba(ImFormat::Alpha8, 2, 2, 3, &pixels).unwrap();
    assert_eq!(
        out,
        vec![
            255, 255, 255, 10, 255, 255, 255, 20, 255, 255, 255, 30, 255, 255, 255, 40,
        ]
    );
}
