use super::*;

#[derive(Debug, Eq, PartialEq)]
struct TextureState {
    pixels_ptr: usize,
    pixels: Option<Vec<u8>>,
    status: TextureStatus,
    used_rect: TextureRect,
    update_rect: TextureRect,
    updates: Vec<TextureRect>,
}

fn texture_state(texture: &TextureData) -> TextureState {
    TextureState {
        pixels_ptr: unsafe { (*texture.as_raw()).Pixels as usize },
        pixels: texture.pixels().map(<[u8]>::to_vec),
        status: texture.status(),
        used_rect: texture.used_rect(),
        update_rect: texture.update_rect(),
        updates: texture.updates().collect(),
    }
}

fn region(x: u32, y: u32, width: u32, height: u32) -> TextureRegion {
    TextureRegion::new(x, y, width, height).unwrap()
}

#[test]
fn texture_id_try_as_usize_reports_overflow() {
    assert_eq!(TextureId::new(42).try_as_usize(), Some(42));

    if std::mem::size_of::<usize>() < std::mem::size_of::<u64>() {
        assert_eq!(TextureId::new(u64::MAX).try_as_usize(), None);
    }
}

#[test]
fn from_pixels_rejects_invalid_dimensions_and_allocation_sizes() {
    assert_eq!(
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, 0, 1, &[]).err(),
        Some(TextureDataError::InvalidDimensions {
            width: 0,
            height: 1,
        })
    );
    assert_eq!(
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, i32::MAX as u32 + 1, 1, &[],).err(),
        Some(TextureDataError::WidthOutOfRange(i32::MAX as u32 + 1))
    );
    assert_eq!(
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, i32::MAX as u32, 2, &[]).err(),
        Some(TextureDataError::ByteSizeOutOfRange {
            width: i32::MAX as u32,
            height: 2,
            bytes_per_pixel: 4,
        })
    );
}

#[test]
fn from_pixels_initializes_metadata_and_pitch() {
    let texture = OwnedTextureData::from_pixels(TextureFormat::Alpha8, 4, 3, &[0; 12]).unwrap();
    assert_eq!(texture.width(), 4);
    assert_eq!(texture.height(), 3);
    assert_eq!(texture.format(), TextureFormat::Alpha8);
    assert_eq!(texture.bytes_per_pixel(), 1);
    assert_eq!(texture.pitch(), 4);
}

#[test]
fn unused_frames_is_a_checked_usize_count() {
    let mut texture = OwnedTextureData::empty();
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
fn replace_pixels_rejects_invalid_layout_before_allocating_or_copying() {
    let mut texture = OwnedTextureData::empty();
    unsafe {
        let raw = texture.as_raw_mut();
        (*raw).Format = sys::ImTextureFormat_RGBA32;
        (*raw).Width = i32::MAX;
        (*raw).Height = 2;
        (*raw).BytesPerPixel = 4;
    }

    let before = texture_state(&texture);
    assert_eq!(
        texture.replace_pixels(&[0; 4]),
        Err(TextureDataError::ByteSizeOutOfRange {
            width: i32::MAX as u32,
            height: 2,
            bytes_per_pixel: 4,
        })
    );
    assert_eq!(texture_state(&texture), before);

    let mut texture = OwnedTextureData::empty();
    unsafe {
        let raw = texture.as_raw_mut();
        (*raw).Format = sys::ImTextureFormat_RGBA32;
        (*raw).Width = 1;
        (*raw).Height = 1;
        (*raw).BytesPerPixel = 4;
        (*raw).Status = sys::ImTextureStatus_WantCreate;
    }

    let before = texture_state(&texture);
    assert_eq!(
        texture.replace_pixels(&[1, 2, 3, 4]),
        Err(TextureDataError::MissingPixelStorage(
            TextureStatus::WantCreate
        ))
    );
    assert_eq!(texture_state(&texture), before);
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
    let mut texture =
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
    assert_eq!(texture.status(), TextureStatus::WantCreate);

    texture.replace_pixels(&[4, 3, 2, 1]).unwrap();
    assert_eq!(texture.status(), TextureStatus::WantCreate);

    unsafe {
        // The test acts as the only renderer owner for this unregistered texture.
        texture.set_status(TextureStatus::OK);
    }
    texture.replace_pixels(&[1, 2, 3, 4]).unwrap();
    assert_eq!(texture.status(), TextureStatus::WantUpdates);
    assert_eq!(
        texture.updates().collect::<Vec<_>>(),
        vec![TextureRect {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        }]
    );
}

#[test]
fn full_updates_reject_unrepresentable_dimensions_before_copying() {
    let width = u16::MAX as u32 + 1;
    let mut texture =
        OwnedTextureData::from_pixels(TextureFormat::Alpha8, width, 1, &vec![1; width as usize])
            .unwrap();
    assert_eq!(texture.status(), TextureStatus::WantCreate);

    unsafe {
        // The test acts as the only renderer owner for this unregistered texture.
        texture.set_status(TextureStatus::OK);
    }
    let before = texture_state(&texture);
    let result = texture.replace_pixels(&vec![2; width as usize]);

    assert_eq!(
        result,
        Err(TextureDataError::FullUpdateRectOutOfRange { width, height: 1 })
    );
    assert_eq!(texture_state(&texture), before);
}

#[test]
fn from_pixels_requires_an_exact_payload() {
    for actual in [0, 1023, 1025] {
        assert_eq!(
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 16, 16, &vec![0; actual])
                .err()
                .unwrap(),
            TextureDataError::ByteLengthMismatch {
                expected: 1024,
                actual,
            }
        );
    }

    let texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 16, 16, &[7; 1024]).unwrap();
    assert_eq!(texture.pixels(), Some([7; 1024].as_slice()));
    assert_eq!(texture.status(), TextureStatus::WantCreate);
}

#[test]
fn replace_pixels_is_exact_and_failure_atomic() {
    let mut texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 2, 2, &[1; 16]).unwrap();
    unsafe {
        texture.set_status(TextureStatus::OK);
    }

    for actual in [0, 15, 17] {
        let before = texture_state(&texture);
        assert_eq!(
            texture.replace_pixels(&vec![2; actual]),
            Err(TextureDataError::ByteLengthMismatch {
                expected: 16,
                actual,
            })
        );
        assert_eq!(texture_state(&texture), before);
    }

    texture.replace_pixels(&[3; 16]).unwrap();
    assert_eq!(texture.pixels(), Some([3; 16].as_slice()));
    assert_eq!(texture.status(), TextureStatus::WantUpdates);
}

#[test]
fn replace_pixels_rejects_destroy_transitions_atomically() {
    let mut texture =
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
    unsafe {
        (*texture.as_raw_mut()).WantDestroyNextFrame = true;
        texture.set_status(TextureStatus::WantDestroy);
    }
    let before = texture_state(&texture);
    assert_eq!(
        texture.replace_pixels(&[4, 3, 2, 1]),
        Err(TextureDataError::InvalidStatus(TextureStatus::WantDestroy))
    );
    assert_eq!(texture_state(&texture), before);

    unsafe {
        texture.set_status(TextureStatus::Destroyed);
    }
    let before = texture_state(&texture);
    assert_eq!(
        texture.replace_pixels(&[4, 3, 2, 1]),
        Err(TextureDataError::InvalidStatus(TextureStatus::Destroyed))
    );
    assert_eq!(texture_state(&texture), before);
}

#[test]
fn full_replacement_queues_a_full_rect_even_when_old_updates_remain() {
    let mut texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 2, 2, &[0; 16]).unwrap();
    unsafe {
        texture.set_status(TextureStatus::OK);
    }
    texture
        .update_subresource(TextureSubresource::new(
            region(0, 0, 1, 1),
            4,
            &[1, 2, 3, 4],
        ))
        .unwrap();
    unsafe {
        // Renderer feedback does not clear native Updates until the next NewFrame.
        texture.set_status(TextureStatus::OK);
    }

    texture.replace_pixels(&[9; 16]).unwrap();

    assert_eq!(texture.status(), TextureStatus::WantUpdates);
    assert_eq!(
        texture.updates().collect::<Vec<_>>(),
        vec![
            TextureRect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
            TextureRect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
        ]
    );
}

#[test]
fn subresource_updates_validate_every_boundary_before_mutation() {
    let mut texture = OwnedTextureData::from_pixels(TextureFormat::Alpha8, 4, 4, &[0; 16]).unwrap();
    unsafe {
        texture.set_status(TextureStatus::OK);
    }

    assert_eq!(
        TextureRegion::new(0, 0, 0, 1),
        Err(TextureDataError::InvalidRegionDimensions {
            width: 0,
            height: 1,
        })
    );

    let failures = [
        (
            region(3, 0, 2, 1),
            2,
            vec![1, 2],
            TextureDataError::UpdateRegionOutOfBounds {
                region: region(3, 0, 2, 1),
                width: 4,
                height: 4,
            },
        ),
        (
            region(0, 0, 2, 2),
            1,
            vec![1, 2],
            TextureDataError::RowPitchTooSmall {
                minimum: 2,
                actual: 1,
            },
        ),
        (
            region(0, 0, 2, 2),
            3,
            vec![1, 2, 3, 4],
            TextureDataError::ByteLengthMismatch {
                expected: 5,
                actual: 4,
            },
        ),
        (
            region(0, 0, 2, 2),
            3,
            vec![1, 2, 3, 4, 5, 6],
            TextureDataError::ByteLengthMismatch {
                expected: 5,
                actual: 6,
            },
        ),
    ];

    for (rect, row_pitch, pixels, expected) in failures {
        let before = texture_state(&texture);
        assert_eq!(
            texture.update_subresource(TextureSubresource::new(rect, row_pitch, &pixels,)),
            Err(expected)
        );
        assert_eq!(texture_state(&texture), before);
    }

    let rect = region(1, 1, 2, 2);
    texture
        .update_subresource(TextureSubresource::new(rect, 3, &[1, 2, 99, 3, 4]))
        .unwrap();

    assert_eq!(texture.status(), TextureStatus::WantUpdates);
    assert_eq!(
        texture.updates().collect::<Vec<_>>(),
        vec![TextureRect {
            x: 1,
            y: 1,
            w: 2,
            h: 2,
        }]
    );
    assert_eq!(
        texture.pixels(),
        Some([0, 0, 0, 0, 0, 1, 2, 0, 0, 3, 4, 0, 0, 0, 0, 0].as_slice())
    );
}

#[test]
fn subresource_update_rejects_overflow_and_destroy_transitions_atomically() {
    let mut texture = OwnedTextureData::from_pixels(TextureFormat::Alpha8, 2, 2, &[0; 4]).unwrap();
    unsafe {
        texture.set_status(TextureStatus::OK);
    }
    let rect = region(0, 0, 1, 2);
    let before = texture_state(&texture);
    assert_eq!(
        texture.update_subresource(TextureSubresource::new(rect, usize::MAX, &[])),
        Err(TextureDataError::PayloadSizeOutOfRange {
            row_pitch: usize::MAX,
            height: 2,
        })
    );
    assert_eq!(texture_state(&texture), before);

    unsafe {
        (*texture.as_raw_mut()).WantDestroyNextFrame = true;
        texture.set_status(TextureStatus::WantDestroy);
    }
    let before = texture_state(&texture);
    assert_eq!(
        texture.update_subresource(TextureSubresource::new(rect, 1, &[1, 2])),
        Err(TextureDataError::InvalidStatus(TextureStatus::WantDestroy))
    );
    assert_eq!(texture_state(&texture), before);
}

#[test]
fn subresource_update_during_initial_creation_changes_pixels_without_queueing_update() {
    let mut texture = OwnedTextureData::from_pixels(TextureFormat::Alpha8, 4, 2, &[0; 8]).unwrap();
    let before_update_rect = texture.update_rect();
    let before_used_rect = texture.used_rect();

    texture
        .update_subresource(TextureSubresource::new(
            region(1, 0, 2, 2),
            3,
            &[1, 2, 99, 3, 4],
        ))
        .unwrap();

    assert_eq!(texture.status(), TextureStatus::WantCreate);
    assert_eq!(texture.update_rect(), before_update_rect);
    assert_eq!(texture.used_rect(), before_used_rect);
    assert!(texture.updates().next().is_none());
    assert_eq!(texture.pixels(), Some([0, 1, 2, 0, 0, 3, 4, 0].as_slice()));
}

#[test]
fn live_subresource_updates_enforce_native_rectangle_boundaries_atomically() {
    let width = u16::MAX as u32 + 1;
    let mut texture =
        OwnedTextureData::from_pixels(TextureFormat::Alpha8, width, 1, &vec![0; width as usize])
            .unwrap();
    unsafe {
        texture.set_status(TextureStatus::OK);
    }

    let representable = region(0, 0, u16::MAX as u32, 1);
    texture
        .update_subresource(TextureSubresource::new(
            representable,
            u16::MAX as usize,
            &vec![1; u16::MAX as usize],
        ))
        .unwrap();

    let unrepresentable = region(0, 0, width, 1);
    let before = texture_state(&texture);
    assert_eq!(
        texture.update_subresource(TextureSubresource::new(
            unrepresentable,
            width as usize,
            &vec![2; width as usize],
        )),
        Err(TextureDataError::UpdateRegionNotRepresentable(
            unrepresentable
        ))
    );
    assert_eq!(texture_state(&texture), before);

    let overflowing = TextureRegion::new(u32::MAX, 0, 1, 1).unwrap();
    assert_eq!(
        texture.update_subresource(TextureSubresource::new(overflowing, 1, &[3])),
        Err(TextureDataError::UpdateRegionOutOfBounds {
            region: overflowing,
            width,
            height: 1,
        })
    );
    assert_eq!(texture_state(&texture), before);
}

#[test]
fn live_subresource_update_accepts_the_native_endpoint_when_bounding_boxes_fit() {
    let width = u16::MAX as u32 + 1;
    let mut texture =
        OwnedTextureData::from_pixels(TextureFormat::Alpha8, width, 1, &vec![0; width as usize])
            .unwrap();
    unsafe {
        let raw = texture.as_raw_mut();
        (*raw).UsedRect = sys::ImTextureRect {
            x: u16::MAX,
            y: 0,
            w: 0,
            h: 0,
        };
        (*raw).UpdateRect = sys::ImTextureRect {
            x: u16::MAX,
            y: 0,
            w: 0,
            h: 0,
        };
        texture.set_status(TextureStatus::OK);
    }

    let last_pixel = region(u16::MAX as u32, 0, 1, 1);
    texture
        .update_subresource(TextureSubresource::new(last_pixel, 1, &[9]))
        .unwrap();

    assert_eq!(texture.status(), TextureStatus::WantUpdates);
    assert_eq!(texture.pixels_at(u16::MAX as u32, 0).unwrap()[0], 9);
    assert_eq!(
        texture.updates().collect::<Vec<_>>(),
        vec![TextureRect {
            x: u16::MAX,
            y: 0,
            w: 1,
            h: 1,
        }]
    );
}
