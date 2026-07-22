use super::*;

#[test]
fn texture_id_try_as_usize_reports_overflow() {
    assert_eq!(TextureId::new(42).try_as_usize(), Some(42));

    if std::mem::size_of::<usize>() < std::mem::size_of::<u64>() {
        assert_eq!(TextureId::new(u64::MAX).try_as_usize(), None);
    }
}

#[test]
fn texture_create_rejects_invalid_sizes_and_status_before_ffi() {
    let mut texture = OwnedTextureData::new();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.create(TextureFormat::RGBA32, 0, 1);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.create(TextureFormat::RGBA32, i32::MAX as u32 + 1, 1);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.create(TextureFormat::RGBA32, i32::MAX as u32, 2);
        }))
        .is_err()
    );

    texture.create(TextureFormat::RGBA32, 1, 1);
    assert_eq!(texture.status(), TextureStatus::WantCreate);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.create(TextureFormat::RGBA32, 1, 1);
        }))
        .is_err()
    );
}

#[test]
fn texture_metadata_setters_are_destroyed_only_and_keep_bpp_in_sync() {
    let mut texture = OwnedTextureData::new();

    texture.set_width(4);
    texture.set_height(3);
    texture.set_format(TextureFormat::Alpha8);

    assert_eq!(texture.width(), 4);
    assert_eq!(texture.height(), 3);
    assert_eq!(texture.format(), TextureFormat::Alpha8);
    assert_eq!(texture.bytes_per_pixel(), 1);
    assert_eq!(texture.pitch(), 4);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.set_width(0);
        }))
        .is_err()
    );

    texture.create(TextureFormat::RGBA32, 1, 1);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.set_width(2);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.set_height(2);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.set_format(TextureFormat::Alpha8);
        }))
        .is_err()
    );
}

#[test]
fn unused_frames_is_a_checked_usize_count() {
    let mut texture = OwnedTextureData::new();
    unsafe {
        (*texture.as_raw_mut()).UnusedFrames = 7;
    }
    assert_eq!(texture.unused_frames(), 7);

    unsafe {
        (*texture.as_raw_mut()).UnusedFrames = -1;
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = texture.unused_frames();
        }))
        .is_err()
    );
}

#[test]
fn set_data_checks_byte_count_before_allocating_or_copying() {
    let mut texture = OwnedTextureData::new();
    unsafe {
        let raw = texture.as_raw_mut();
        (*raw).Format = sys::ImTextureFormat_RGBA32;
        (*raw).Width = i32::MAX;
        (*raw).Height = 2;
        (*raw).BytesPerPixel = 4;
    }

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.set_data(&[0; 4]);
        }))
        .is_err()
    );

    let mut texture = OwnedTextureData::new();
    unsafe {
        let raw = texture.as_raw_mut();
        (*raw).Format = sys::ImTextureFormat_RGBA32;
        (*raw).Width = 1;
        (*raw).Height = 1;
        (*raw).BytesPerPixel = 4;
        (*raw).Status = sys::ImTextureStatus_WantCreate;
    }

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            texture.set_data(&[1, 2, 3, 4]);
        }))
        .is_err()
    );
    assert!(unsafe { (*texture.as_raw()).Pixels.is_null() });
}

#[test]
fn texture_id_supports_lossless_handle_conversions() {
    let native = std::num::NonZeroU32::new(42).unwrap();
    let id = TextureId::from(native);

    assert_eq!(id, TextureId::from(42_u32));
    assert_eq!(id, TextureId::from(42_usize));
    assert_eq!(id.try_as_u32(), Some(42));
    assert_eq!(u32::try_from(id), Ok(42));
    assert_eq!(usize::try_from(id), Ok(42));
    assert_eq!(u64::from(id), 42);

    assert_eq!(TextureId::new(u64::MAX).try_as_u32(), None);
    assert!(u32::try_from(TextureId::new(u64::MAX)).is_err());
}

#[test]
fn initial_pixel_upload_preserves_the_create_request() {
    let mut texture = OwnedTextureData::new();
    texture.create(TextureFormat::RGBA32, 1, 1);
    assert_eq!(texture.status(), TextureStatus::WantCreate);

    texture.set_data(&[1, 2, 3, 4]);
    assert_eq!(texture.status(), TextureStatus::WantCreate);

    unsafe {
        // The test acts as the only renderer owner for this unregistered texture.
        texture.set_status(TextureStatus::OK);
    }
    texture.set_data(&[4, 3, 2, 1]);
    assert_eq!(texture.status(), TextureStatus::WantUpdates);
}

#[test]
fn full_updates_reject_unrepresentable_dimensions_before_copying() {
    let width = u16::MAX as u32 + 1;
    let mut texture = OwnedTextureData::new();
    texture.create(TextureFormat::Alpha8, width, 1);
    texture.set_data(&vec![1; width as usize]);
    assert_eq!(texture.status(), TextureStatus::WantCreate);

    unsafe {
        // The test acts as the only renderer owner for this unregistered texture.
        texture.set_status(TextureStatus::OK);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        texture.set_data(&vec![2; width as usize]);
    }));

    assert!(result.is_err());
    assert_eq!(texture.status(), TextureStatus::OK);
    assert!(texture.pixels().unwrap().iter().all(|pixel| *pixel == 1));
}
