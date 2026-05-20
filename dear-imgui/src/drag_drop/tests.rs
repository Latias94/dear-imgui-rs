use super::flags::{
    DragDropSourceFlags, DragDropTargetFlags, validate_drag_drop_source_flags,
    validate_drag_drop_target_flags,
};
use super::payload::{DragDropPayload, TypedPayload, decode_typed_payload, make_typed_payload};
use super::validation::{MAX_PAYLOAD_TYPE_LEN, validate_payload_submission};
use std::{any, ffi};

fn payload_bytes<T: Copy + 'static>(value: T) -> Vec<u8> {
    let payload = make_typed_payload(value);
    let size = std::mem::size_of::<TypedPayload<T>>();
    let mut out = vec![0u8; size];
    unsafe {
        std::ptr::copy_nonoverlapping(
            std::ptr::from_ref(&payload).cast::<u8>(),
            out.as_mut_ptr(),
            size,
        );
    }
    out
}

#[test]
fn typed_payload_bytes_are_deterministic() {
    // If we accidentally leak uninitialized padding bytes, these can become nondeterministic.
    assert_eq!(payload_bytes(7u8), payload_bytes(7u8));
    assert_eq!(payload_bytes(0x1122_3344u32), payload_bytes(0x1122_3344u32));
}

#[test]
fn typed_payload_can_be_read_unaligned() {
    let bytes = payload_bytes(7u8);
    let mut buf = vec![0u8; 1 + bytes.len()];
    buf[1..].copy_from_slice(&bytes);
    let ptr = unsafe { buf.as_ptr().add(1) } as *const TypedPayload<u8>;
    let decoded = unsafe { std::ptr::read_unaligned(ptr) };
    assert_eq!(decoded.type_id, any::TypeId::of::<u8>());
    assert_eq!(decoded.data, 7u8);
}

#[test]
fn payload_submission_rejects_imgui_assert_conditions_before_ffi() {
    let byte = 1u8;
    let ptr = std::ptr::from_ref(&byte).cast::<ffi::c_void>();
    let long_name = "x".repeat(MAX_PAYLOAD_TYPE_LEN + 1);

    assert!(
        std::panic::catch_unwind(|| {
            validate_payload_submission(
                &long_name,
                ptr,
                1,
                "payload_submission_rejects_imgui_assert_conditions_before_ffi",
            );
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_payload_submission(
                "payload",
                std::ptr::null(),
                1,
                "payload_submission_rejects_imgui_assert_conditions_before_ffi",
            );
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_payload_submission(
                "payload",
                ptr,
                0,
                "payload_submission_rejects_imgui_assert_conditions_before_ffi",
            );
        })
        .is_err()
    );
}

#[test]
fn drag_drop_flag_domain_validation_rejects_wrong_domain_bits() {
    validate_drag_drop_source_flags("test", DragDropSourceFlags::all());
    validate_drag_drop_target_flags("test", DragDropTargetFlags::all());

    let target_bit_in_source =
        DragDropSourceFlags::from_bits_retain(DragDropTargetFlags::BEFORE_DELIVERY.bits());
    assert!(
        std::panic::catch_unwind(|| {
            validate_drag_drop_source_flags("test", target_bit_in_source);
        })
        .is_err()
    );

    let source_bit_in_target =
        DragDropTargetFlags::from_bits_retain(DragDropSourceFlags::NO_PREVIEW_TOOLTIP.bits());
    assert!(
        std::panic::catch_unwind(|| {
            validate_drag_drop_target_flags("test", source_bit_in_target);
        })
        .is_err()
    );

    let known_bits = DragDropSourceFlags::all().bits() | DragDropTargetFlags::all().bits();
    let unknown_bit = (0..u32::BITS)
        .map(|bit| 1u32 << bit)
        .find(|bit| known_bits & bit == 0)
        .expect("Dear ImGui drag-drop flags should not consume every u32 bit");

    assert!(
        std::panic::catch_unwind(|| {
            validate_drag_drop_source_flags(
                "test",
                DragDropSourceFlags::from_bits_retain(unknown_bit),
            );
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_drag_drop_target_flags(
                "test",
                DragDropTargetFlags::from_bits_retain(unknown_bit),
            );
        })
        .is_err()
    );
}

#[test]
fn typed_accept_rejects_trailing_payload_bytes() {
    let bytes = payload_bytes(7u8);
    let mut buf = bytes.clone();
    buf.push(0);

    let payload = DragDropPayload {
        data: buf.as_ptr().cast::<ffi::c_void>(),
        size: buf.len(),
        preview: false,
        delivery: false,
    };

    assert_ne!(payload.size, std::mem::size_of::<TypedPayload<u8>>());
    assert!(decode_typed_payload::<u8>(payload).is_err());
}
