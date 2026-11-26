use super::*;

const PROGGY: &[u8] = include_bytes!(
    "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
);

#[test]
fn checked_bytes_are_copied_for_deferred_glyph_access() {
    let context = crate::Context::create();
    let atlas = context.font_atlas();
    let bytes = PROGGY.to_vec();

    let font = atlas
        .try_add_font(&[FontSource::ttf_bytes(&bytes)])
        .expect("the built-in atlas loader should accept the test font");
    let source = unsafe { &*(*atlas.raw()).Sources.Data };
    assert!(source.FontDataOwnedByAtlas);
    assert_ne!(source.FontData.cast::<u8>().cast_const(), bytes.as_ptr());
    drop(bytes);

    atlas.try_claim_legacy_renderer().unwrap().build();
    assert_eq!(font.reference_size(), Some(13.0));
    assert!(font.is_glyph_in_font('A'));
}

#[test]
fn checked_bytes_keep_the_stb_proof_when_atlas_selects_stb() {
    let context = crate::Context::create();
    let atlas = context.font_atlas();
    let stb = FontLoader::stb_truetype();
    atlas.set_font_loader(stb).unwrap();
    let raw = atlas.raw();

    let invalid_font = [0; 128];
    assert!(matches!(
        atlas.try_add_font(&[
            FontSource::default_font(),
            FontSource::ttf_bytes(&invalid_font),
        ]),
        Err(FontLoadError::StbTrueType(_))
    ));
    assert_eq!(unsafe { (*raw).Fonts.Size }, 0);
    assert_eq!(unsafe { (*raw).Sources.Size }, 0);
    assert_eq!(
        atlas.try_add_font(&[FontSource::ttf_bytes(PROGGY).font_index(1)]),
        Err(FontLoadError::InvalidFontIndex { index: 1 })
    );

    let font = atlas
        .try_add_font(&[FontSource::ttf_bytes(PROGGY)])
        .unwrap();
    assert_eq!(unsafe { (*(*raw).Sources.Data).FontLoader }, stb.as_ptr());
    assert!(font.is_glyph_in_font('A'));
}

#[test]
fn checked_bytes_cannot_override_or_use_a_custom_loader() {
    let _guard = crate::test_support::imgui_context_guard();
    let stb = FontLoader::stb_truetype();
    let custom = unsafe { *stb.as_ptr() };
    let context = crate::Context::create();
    let override_config = FontConfig::new().font_loader(stb);
    assert_eq!(
        context
            .font_atlas()
            .try_add_font(&[FontSource::ttf_bytes(PROGGY).with_config(override_config)]),
        Err(FontLoadError::LoaderOverride)
    );

    let atlas = context.font_atlas();
    unsafe { sys::ImFontAtlas_SetFontLoader(atlas.raw(), &custom) };
    let result = atlas.try_add_font(&[FontSource::ttf_bytes(PROGGY)]);
    unsafe { sys::ImFontAtlas_SetFontLoader(atlas.raw(), stb.as_ptr()) };
    assert_eq!(result, Err(FontLoadError::UnsupportedAtlasLoader));
}

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
#[test]
fn checked_bytes_load_the_requested_freetype_collection_face() {
    const ROBOTO: &[u8] = include_bytes!(
        "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
    );

    let context = crate::Context::create();
    let atlas = context.font_atlas();
    let raw = atlas.raw();
    let collection = make_collection([PROGGY, ROBOTO]);
    assert_eq!(
        atlas.try_add_font(&[FontSource::ttf_bytes(&collection).font_index(2)]),
        Err(FontLoadError::InvalidFontIndex { index: 2 })
    );
    assert_eq!(
        atlas.try_add_font(&[FontSource::ttf_bytes(&collection).font_index(1 << 16)]),
        Err(FontLoadError::InvalidFontIndex { index: 1 << 16 })
    );
    assert_eq!(unsafe { (*raw).Sources.Size }, 0);

    let first = atlas
        .try_add_font(&[FontSource::ttf_bytes(&collection)])
        .unwrap();
    let second = atlas
        .try_add_font(&[FontSource::ttf_bytes(&collection).font_index(1)])
        .unwrap();
    let source = unsafe { &*(*raw).Sources.Data.add(1) };
    assert!(source.FontLoader.is_null());
    assert_eq!(source.FontNo, 1);
    drop(collection);

    assert!(first.is_glyph_in_font('A'));
    assert!(!first.is_glyph_in_font('Ж'));
    assert!(second.is_glyph_in_font('Ж'));
}

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
#[test]
fn checked_bytes_report_native_freetype_rejection() {
    let context = crate::Context::create();
    let atlas = context.font_atlas();
    let mut no_tables = [0; 128];
    no_tables[..4].copy_from_slice(b"\0\x01\0\0");

    assert_eq!(
        atlas.try_add_font(&[FontSource::ttf_bytes(&no_tables)]),
        Err(FontLoadError::NativeRejected)
    );
    assert_eq!(unsafe { (*atlas.raw()).Fonts.Size }, 0);
    assert_eq!(unsafe { (*atlas.raw()).Sources.Size }, 0);
    assert!(atlas.try_add_font(&[FontSource::ttf_bytes(PROGGY)]).is_ok());
}

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
#[test]
fn checked_bytes_reject_explicit_merge_into_an_implicit_freetype_face() {
    let context = crate::Context::create();
    let atlas = context.font_atlas();
    let source = FontSource::ttf_bytes(PROGGY);
    let merge = FontSource::ttf_bytes(PROGGY).with_config(FontConfig::new().size_pixels(16.0));

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.try_add_font(&[source, merge]);
        }))
        .is_err()
    );
    assert_eq!(unsafe { (*atlas.raw()).Sources.Size }, 0);
}

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
fn make_collection(fonts: [&[u8]; 2]) -> Vec<u8> {
    let mut collection = b"ttcf\0\x01\0\0\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
    for (index, font) in fonts.into_iter().enumerate() {
        let offset = collection.len().next_multiple_of(4);
        collection.resize(offset, 0);
        collection.extend_from_slice(font);
        let offset_record = 12 + index * 4;
        collection[offset_record..offset_record + 4]
            .copy_from_slice(&(offset as u32).to_be_bytes());

        let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
        let directory = &mut collection[offset + 12..offset + 12 + table_count * 16];
        for record in directory.as_chunks_mut::<16>().0 {
            let table_offset = u32::from_be_bytes(record[8..12].try_into().unwrap());
            record[8..12].copy_from_slice(&(table_offset + offset as u32).to_be_bytes());
        }
    }
    collection
}
