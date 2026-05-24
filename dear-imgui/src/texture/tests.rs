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
    let mut texture = TextureData::new();

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
    let mut texture = TextureData::new();

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
    let mut texture = TextureData::new();
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
fn managed_texture_id_is_a_typed_texture_identity() {
    let mut texture = TextureData::new();
    unsafe {
        (*texture.as_raw_mut()).UniqueID = 42;
    }

    let id: ManagedTextureId = texture.unique_id();
    let snapshot_id: crate::render::snapshot::ManagedTextureId = id;

    assert_eq!(id, ManagedTextureId::from_raw(42));
    assert_eq!(id.raw(), 42);
    assert_eq!(snapshot_id, id);
}

#[test]
fn set_data_checks_byte_count_before_allocating_or_copying() {
    let mut texture = TextureData::new();
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

    let mut texture = TextureData::new();
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
