use crate::internal::{DataType, DataTypeKind};
use crate::sys;

use super::SliderFlags;

pub(super) fn validate_slider_flags(caller: &str, flags: SliderFlags) {
    let bits = flags.bits();
    assert!(
        bits & (sys::ImGuiSliderFlags_WrapAround as i32) == 0,
        "{caller} does not support ImGuiSliderFlags_WrapAround; use DragFlags::WRAP_AROUND with drag widgets"
    );
    let unsupported = bits & !SliderFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiSliderFlags bits: 0x{unsupported:X}"
    );
}

pub(super) fn validate_slider_range<Data: DataTypeKind>(caller: &str, min: &Data, max: &Data) {
    match Data::KIND {
        DataType::I8 | DataType::U8 | DataType::I16 | DataType::U16 => {}
        DataType::I32 => {
            let min = unsafe { *(min as *const Data as *const i32) };
            let max = unsafe { *(max as *const Data as *const i32) };
            let lower = i32::MIN / 2;
            let upper = i32::MAX / 2;
            assert!(
                (lower..=upper).contains(&min) && (lower..=upper).contains(&max),
                "{caller} i32/isize range endpoints must stay within i32::MIN/2..=i32::MAX/2"
            );
        }
        DataType::U32 => {
            let min = unsafe { *(min as *const Data as *const u32) };
            let max = unsafe { *(max as *const Data as *const u32) };
            let upper = u32::MAX / 2;
            assert!(
                min <= upper && max <= upper,
                "{caller} u32/usize range endpoints must be <= u32::MAX/2"
            );
        }
        DataType::I64 => {
            let min = unsafe { *(min as *const Data as *const i64) };
            let max = unsafe { *(max as *const Data as *const i64) };
            let lower = i64::MIN / 2;
            let upper = i64::MAX / 2;
            assert!(
                (lower..=upper).contains(&min) && (lower..=upper).contains(&max),
                "{caller} i64/isize range endpoints must stay within i64::MIN/2..=i64::MAX/2"
            );
        }
        DataType::U64 => {
            let min = unsafe { *(min as *const Data as *const u64) };
            let max = unsafe { *(max as *const Data as *const u64) };
            let upper = u64::MAX / 2;
            assert!(
                min <= upper && max <= upper,
                "{caller} u64/usize range endpoints must be <= u64::MAX/2"
            );
        }
        DataType::F32 => {
            let min = unsafe { *(min as *const Data as *const f32) };
            let max = unsafe { *(max as *const Data as *const f32) };
            assert!(
                min.is_finite()
                    && max.is_finite()
                    && (-f32::MAX / 2.0..=f32::MAX / 2.0).contains(&min)
                    && (-f32::MAX / 2.0..=f32::MAX / 2.0).contains(&max),
                "{caller} f32 range endpoints must be finite and stay within -f32::MAX/2..=f32::MAX/2"
            );
        }
        DataType::F64 => {
            let min = unsafe { *(min as *const Data as *const f64) };
            let max = unsafe { *(max as *const Data as *const f64) };
            assert!(
                min.is_finite()
                    && max.is_finite()
                    && (-f64::MAX / 2.0..=f64::MAX / 2.0).contains(&min)
                    && (-f64::MAX / 2.0..=f64::MAX / 2.0).contains(&max),
                "{caller} f64 range endpoints must be finite and stay within -f64::MAX/2..=f64::MAX/2"
            );
        }
    }
}

pub(super) fn validate_slider_preconditions<Data: DataTypeKind>(
    caller: &str,
    min: &Data,
    max: &Data,
    flags: SliderFlags,
) {
    validate_slider_flags(caller, flags);
    validate_slider_range(caller, min, max);
}
