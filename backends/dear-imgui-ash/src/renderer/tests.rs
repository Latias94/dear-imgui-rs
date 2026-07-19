use super::texture::{apply_rgba_rect, texture_data_to_rgba_subrect, texture_upload_to_rgba};
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

#[test]
fn managed_update_composes_a_complete_replacement_image() {
    let mut rgba = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, // row 0
        13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, // row 1
    ];
    let update = [101, 102, 103, 104, 105, 106, 107, 108];

    assert!(apply_rgba_rect(&mut rgba, 3, 2, 1, 0, 1, 2, &update));
    assert_eq!(
        rgba,
        vec![
            1, 2, 3, 4, 101, 102, 103, 104, 9, 10, 11, 12, // row 0
            13, 14, 15, 16, 105, 106, 107, 108, 21, 22, 23, 24, // row 1
        ]
    );
}

#[test]
fn managed_update_rejects_out_of_bounds_without_mutating_shadow() {
    let mut rgba = vec![1_u8; 3 * 2 * 4];
    let original = rgba.clone();

    assert!(!apply_rgba_rect(
        &mut rgba,
        3,
        2,
        2,
        1,
        2,
        1,
        &[2_u8; 2 * 4]
    ));
    assert_eq!(rgba, original);
}
