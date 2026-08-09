use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;

use super::glyph::{
    CompositeComponent, CompositeTransform, CoordinateBounds, GlyphKind, expand_glyph,
};
use super::{
    MAX_COMPOSITE_DEPTH, MAX_EXPANDED_GLYPH_COMPLEXITY, StbTrueTypeFontData, StbTrueTypeFontError,
    StbTrueTypeFontLoadError, validate_font_data_length,
};

const PROGGY: &[u8] = include_bytes!(
    "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
);
const ROBOTO: &[u8] = include_bytes!(
    "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
);

#[test]
fn accepts_bundled_simple_format_4_font() {
    let font = StbTrueTypeFontData::from_slice(PROGGY).unwrap();
    assert_eq!(font.as_bytes(), PROGGY);
    assert!(!font.is_empty());
}

#[test]
fn accepts_bundled_composite_format_12_font() {
    let font = StbTrueTypeFontData::from_slice(ROBOTO).unwrap();
    assert_eq!(font.len(), ROBOTO.len());
}

#[test]
fn accepts_other_bundled_imgui_truetype_fonts() {
    let fonts: &[&[u8]] = &[
        include_bytes!(
            "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Cousine-Regular.ttf"
        ),
        include_bytes!(
            "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/DroidSans.ttf"
        ),
        include_bytes!(
            "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Karla-Regular.ttf"
        ),
        include_bytes!(
            "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyTiny.ttf"
        ),
    ];
    for font in fonts {
        StbTrueTypeFontData::from_slice(font).unwrap();
    }
}

#[test]
fn owns_shared_font_bytes() {
    let bytes: Arc<[u8]> = Arc::from(PROGGY);
    let font = StbTrueTypeFontData::from_bytes(Arc::clone(&bytes)).unwrap();
    drop(bytes);
    assert_eq!(font.as_bytes(), PROGGY);
}

#[test]
fn reads_and_validates_a_file() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
    );
    let font = StbTrueTypeFontData::from_file(path).unwrap();
    assert_eq!(font.as_bytes(), ROBOTO);
}

#[test]
fn distinguishes_file_io_from_validation_errors() {
    let error =
        StbTrueTypeFontData::from_file("this/path/must/not/exist/dear-imgui-font-proof.ttf")
            .unwrap_err();
    assert!(matches!(error, StbTrueTypeFontLoadError::Io { .. }));
}

#[test]
fn reports_validation_for_a_readable_invalid_file() {
    let path = temporary_font_path("invalid");
    fs::write(&path, b"not a TrueType font").unwrap();
    let result = StbTrueTypeFontData::from_file(&path);
    fs::remove_file(&path).unwrap();

    assert!(matches!(
        result,
        Err(StbTrueTypeFontLoadError::Validation(_))
    ));
}

#[test]
fn rejects_oversized_file_metadata_before_reading_the_body() {
    let path = temporary_font_path("oversized");
    let file = File::create(&path).unwrap();
    file.set_len(StbTrueTypeFontData::MAX_BYTES as u64 + 1)
        .unwrap();
    drop(file);

    let result = StbTrueTypeFontData::from_file(&path);
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        result,
        Err(StbTrueTypeFontLoadError::Validation(
            StbTrueTypeFontError::DataTooLarge { length, limit }
        )) if length == StbTrueTypeFontData::MAX_BYTES + 1
            && limit == StbTrueTypeFontData::MAX_BYTES
    ));
}

#[test]
fn rejects_oversized_owned_input_before_validation() {
    assert!(matches!(
        validate_font_data_length(StbTrueTypeFontData::MAX_BYTES + 1),
        Err(StbTrueTypeFontError::DataTooLarge { length, limit })
            if length == StbTrueTypeFontData::MAX_BYTES + 1
                && limit == StbTrueTypeFontData::MAX_BYTES
    ));
}

#[test]
fn rejects_truncated_sfnt_directory() {
    let error = StbTrueTypeFontData::from_slice(&PROGGY[..11]).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::Truncated { .. }));
}

#[test]
fn rejects_non_truetype_containers() {
    let mut font = PROGGY.to_vec();
    font[..4].copy_from_slice(b"OTTO");
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::UnsupportedContainer { .. }
    ));
}

#[test]
fn rejects_directory_count_overflow_or_truncation() {
    let mut font = PROGGY.to_vec();
    font[4..6].copy_from_slice(&u16::MAX.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::Truncated { .. } | StbTrueTypeFontError::InvalidDirectory { .. }
    ));
}

#[test]
fn rejects_duplicate_table_tags() {
    let mut font = PROGGY.to_vec();
    let first_tag: [u8; 4] = font[12..16].try_into().unwrap();
    font[28..32].copy_from_slice(&first_tag);
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::DuplicateTable { .. }));
}

#[test]
fn rejects_out_of_bounds_table_ranges() {
    let mut font = PROGGY.to_vec();
    font[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::TableOutOfBounds { .. }
    ));
}

#[test]
fn rejects_zero_or_negative_stb_pixel_height_denominator() {
    let mut font = PROGGY.to_vec();
    let hhea = table(&font, b"hhea");
    let ascender = [font[hhea.0 + 4], font[hhea.0 + 5]];
    font[hhea.0 + 6..hhea.0 + 8].copy_from_slice(&ascender);

    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::InvalidTable { tag, .. } if tag == *b"hhea"
    ));
}

#[test]
fn rejects_the_exact_unsupported_cmap_selected_by_stb() {
    let mut font = PROGGY.to_vec();
    let cmap = table(&font, b"cmap");
    let cmap_bytes = font[cmap.0..cmap.0 + cmap.1].to_vec();
    let records = u16::from_be_bytes([cmap_bytes[2], cmap_bytes[3]]) as usize;
    let mac_format_zero_offset = (0..records)
        .map(|index| 4 + index * 8)
        .find_map(|record| {
            let platform = u16::from_be_bytes([cmap_bytes[record], cmap_bytes[record + 1]]);
            let subtable = u32::from_be_bytes([
                cmap_bytes[record + 4],
                cmap_bytes[record + 5],
                cmap_bytes[record + 6],
                cmap_bytes[record + 7],
            ]) as usize;
            (platform == 1
                && u16::from_be_bytes([cmap_bytes[subtable], cmap_bytes[subtable + 1]]) == 0)
                .then_some(subtable as u32)
        })
        .unwrap();
    let last_recognized = (0..records)
        .map(|index| 4 + index * 8)
        .rfind(|&record| {
            let platform = u16::from_be_bytes([cmap_bytes[record], cmap_bytes[record + 1]]);
            let encoding = u16::from_be_bytes([cmap_bytes[record + 2], cmap_bytes[record + 3]]);
            platform == 0 || (platform == 3 && matches!(encoding, 1 | 10))
        })
        .unwrap();
    font[cmap.0 + last_recognized + 4..cmap.0 + last_recognized + 8]
        .copy_from_slice(&mac_format_zero_offset.to_be_bytes());

    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::UnsupportedCmapFormat { format: 0, .. }
    ));
}

#[test]
fn rejects_malformed_format_4_search_fields() {
    let mut font = PROGGY.to_vec();
    let cmap = table(&font, b"cmap");
    let selected = selected_cmap_offset(&font[cmap.0..cmap.0 + cmap.1]);
    font[cmap.0 + selected + 8..cmap.0 + selected + 10].copy_from_slice(&0_u16.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::InvalidCmap { .. }));
}

#[test]
fn rejects_malformed_format_12_groups() {
    let mut font = ROBOTO.to_vec();
    let cmap = table(&font, b"cmap");
    let selected = selected_cmap_offset(&font[cmap.0..cmap.0 + cmap.1]);
    assert_eq!(
        u16::from_be_bytes([font[cmap.0 + selected], font[cmap.0 + selected + 1]]),
        12
    );
    let first_group = cmap.0 + selected + 16;
    font[first_group..first_group + 4].copy_from_slice(&0x11_0000_u32.to_be_bytes());
    font[first_group + 4..first_group + 8].copy_from_slice(&0x11_0000_u32.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::InvalidCmap { .. }));
}

#[test]
fn rejects_truncated_simple_glyph_records() {
    let mut font = PROGGY.to_vec();
    let head = table(&font, b"head");
    let loca = table(&font, b"loca");
    let format = i16::from_be_bytes([font[head.0 + 50], font[head.0 + 51]]);
    assert_eq!(format, 0);
    let first_end = u16::from_be_bytes([font[loca.0 + 2], font[loca.0 + 3]]);
    assert!(first_end > 1);
    font[loca.0 + 2..loca.0 + 4].copy_from_slice(&1_u16.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::InvalidGlyph { .. }));
}

#[test]
fn rejects_composite_glyph_cycles() {
    let mut font = ROBOTO.to_vec();
    let (glyph_id, component_offset, _) = first_composite_component(&font);
    font[component_offset..component_offset + 2].copy_from_slice(&glyph_id.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::CompositeCycle { .. }));
}

#[test]
fn rejects_out_of_range_composite_references() {
    let mut font = ROBOTO.to_vec();
    let (_, component_offset, glyph_count) = first_composite_component(&font);
    font[component_offset..component_offset + 2].copy_from_slice(&glyph_count.to_be_bytes());
    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::InvalidGlyphReference { .. }
    ));
}

#[test]
fn rejects_composites_above_the_declared_maxp_component_limit() {
    let mut font = ROBOTO.to_vec();
    let maxp = table(&font, b"maxp");
    font[maxp.0 + 28..maxp.0 + 30].copy_from_slice(&0_u16.to_be_bytes());

    let error = StbTrueTypeFontData::try_from(font).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::InvalidGlyph { .. }));
}

#[test]
fn rejects_composite_chains_above_the_recursion_limit() {
    let glyph_count = MAX_COMPOSITE_DEPTH + 2;
    let mut glyphs = (0..glyph_count - 1)
        .map(|index| GlyphKind::Composite {
            components: vec![CompositeComponent {
                glyph_id: (index + 1) as u16,
                transform: CompositeTransform::default(),
                source_offset: index,
            }],
        })
        .collect::<Vec<_>>();
    glyphs.push(simple_glyph_with_bounds(CoordinateBounds {
        min_x: 0,
        max_x: 1,
        min_y: 0,
        max_y: 1,
    }));

    let error = expand_glyph(
        0,
        &glyphs,
        &mut vec![0; glyph_count],
        &mut vec![None; glyph_count],
        0,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::CompositeDepth { limit, .. }
            if limit == MAX_COMPOSITE_DEPTH
    ));
}

#[test]
fn rejects_shared_composite_dags_above_the_expansion_limit() {
    let repeated_components = MAX_EXPANDED_GLYPH_COMPLEXITY / usize::from(u16::MAX) + 1;
    let glyphs = vec![
        GlyphKind::Simple {
            points: usize::from(u16::MAX),
            contours: 1,
            bounds: None,
        },
        GlyphKind::Composite {
            components: (0..repeated_components)
                .map(|index| CompositeComponent {
                    glyph_id: 0,
                    transform: CompositeTransform::default(),
                    source_offset: index,
                })
                .collect(),
        },
    ];

    let error = expand_glyph(1, &glyphs, &mut [0; 2], &mut [None; 2], 0).unwrap_err();
    assert!(matches!(
        error,
        StbTrueTypeFontError::CompositeComplexity { limit, .. }
            if limit == MAX_EXPANDED_GLYPH_COMPLEXITY
    ));
}

#[test]
fn rejects_composite_translation_outside_native_short_coordinates() {
    let glyphs = vec![
        simple_glyph_with_bounds(CoordinateBounds {
            min_x: i16::MAX,
            max_x: i16::MAX,
            min_y: 0,
            max_y: 0,
        }),
        GlyphKind::Composite {
            components: vec![CompositeComponent {
                glyph_id: 0,
                transform: CompositeTransform {
                    dx: 1,
                    ..CompositeTransform::default()
                },
                source_offset: 10,
            }],
        },
    ];

    let error = expand_glyph(1, &glyphs, &mut [0; 2], &mut [None; 2], 0).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::InvalidGlyph { .. }));
}

#[test]
fn rejects_nested_composite_matrix_overflow() {
    let scale = CompositeTransform {
        xx: 2.0,
        yy: 2.0,
        ..CompositeTransform::default()
    };
    let glyphs = vec![
        simple_glyph_with_bounds(CoordinateBounds {
            min_x: 10_000,
            max_x: 10_000,
            min_y: 10_000,
            max_y: 10_000,
        }),
        GlyphKind::Composite {
            components: vec![CompositeComponent {
                glyph_id: 0,
                transform: scale,
                source_offset: 20,
            }],
        },
        GlyphKind::Composite {
            components: vec![CompositeComponent {
                glyph_id: 1,
                transform: scale,
                source_offset: 30,
            }],
        },
    ];

    let error = expand_glyph(2, &glyphs, &mut [0; 3], &mut [None; 3], 0).unwrap_err();
    assert!(matches!(error, StbTrueTypeFontError::InvalidGlyph { .. }));
}

fn simple_glyph_with_bounds(bounds: CoordinateBounds) -> GlyphKind {
    GlyphKind::Simple {
        points: 1,
        contours: 1,
        bounds: Some(bounds),
    }
}

fn temporary_font_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dear-imgui-stb-validation-{label}-{}.ttf",
        std::process::id()
    ))
}

fn table(font: &[u8], tag: &[u8; 4]) -> (usize, usize) {
    let count = u16::from_be_bytes([font[4], font[5]]) as usize;
    (0..count)
        .map(|index| 12 + index * 16)
        .find_map(|record| {
            (&font[record..record + 4] == tag).then(|| {
                let offset = u32::from_be_bytes([
                    font[record + 8],
                    font[record + 9],
                    font[record + 10],
                    font[record + 11],
                ]) as usize;
                let length = u32::from_be_bytes([
                    font[record + 12],
                    font[record + 13],
                    font[record + 14],
                    font[record + 15],
                ]) as usize;
                (offset, length)
            })
        })
        .unwrap()
}

fn selected_cmap_offset(cmap: &[u8]) -> usize {
    let count = u16::from_be_bytes([cmap[2], cmap[3]]) as usize;
    (0..count)
        .map(|index| 4 + index * 8)
        .filter(|&record| {
            let platform = u16::from_be_bytes([cmap[record], cmap[record + 1]]);
            let encoding = u16::from_be_bytes([cmap[record + 2], cmap[record + 3]]);
            platform == 0 || (platform == 3 && matches!(encoding, 1 | 10))
        })
        .map(|record| {
            u32::from_be_bytes([
                cmap[record + 4],
                cmap[record + 5],
                cmap[record + 6],
                cmap[record + 7],
            ]) as usize
        })
        .next_back()
        .unwrap()
}

fn first_composite_component(font: &[u8]) -> (u16, usize, u16) {
    let head = table(font, b"head");
    let loca = table(font, b"loca");
    let glyf = table(font, b"glyf");
    let maxp = table(font, b"maxp");
    let glyph_count = u16::from_be_bytes([font[maxp.0 + 4], font[maxp.0 + 5]]);
    let format = i16::from_be_bytes([font[head.0 + 50], font[head.0 + 51]]);

    for glyph_id in 0..glyph_count {
        let location = |index: u16| -> usize {
            if format == 0 {
                let entry = loca.0 + usize::from(index) * 2;
                usize::from(u16::from_be_bytes([font[entry], font[entry + 1]])) * 2
            } else {
                let entry = loca.0 + usize::from(index) * 4;
                u32::from_be_bytes([
                    font[entry],
                    font[entry + 1],
                    font[entry + 2],
                    font[entry + 3],
                ]) as usize
            }
        };
        let start = location(glyph_id);
        let end = location(glyph_id + 1);
        if end > start {
            let contours = i16::from_be_bytes([font[glyf.0 + start], font[glyf.0 + start + 1]]);
            if contours < 0 {
                return (glyph_id, glyf.0 + start + 12, glyph_count);
            }
        }
    }
    panic!("Roboto test asset must contain a composite glyph")
}
