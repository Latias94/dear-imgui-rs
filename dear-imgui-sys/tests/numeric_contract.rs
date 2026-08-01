#![cfg(not(target_arch = "wasm32"))]

use std::ffi::{CStr, CString, c_void};

use dear_imgui_sys as sys;

fn format_value<T>(data_type: sys::ImGuiDataType, value: &T, format: &str) -> String {
    format_value_with_capacity(data_type, value, format, 1024).1
}

fn format_value_with_capacity<T>(
    data_type: sys::ImGuiDataType,
    value: &T,
    format: &str,
    capacity: usize,
) -> (i32, String) {
    assert!(
        capacity > 0,
        "the test helper requires room for a terminator"
    );
    let format = CString::new(format).expect("test format must not contain NUL bytes");
    let mut output = vec![0_i8; capacity];
    let written = unsafe {
        sys::igDataTypeFormatString(
            output.as_mut_ptr(),
            output.len().try_into().unwrap(),
            data_type,
            std::ptr::from_ref(value).cast::<c_void>(),
            format.as_ptr(),
        )
    };
    assert!(written >= 0);
    let output = unsafe { CStr::from_ptr(output.as_ptr()) }
        .to_str()
        .expect("formatted output must remain UTF-8")
        .to_owned();
    (written, output)
}

fn apply_value<T>(data_type: sys::ImGuiDataType, value: &mut T, input: &str, format: &str) -> bool {
    let input = CString::new(input).expect("test input must not contain NUL bytes");
    let format = CString::new(format).expect("test format must not contain NUL bytes");
    unsafe {
        sys::igDataTypeApplyFromText(
            input.as_ptr(),
            data_type,
            std::ptr::from_mut(value).cast::<c_void>(),
            format.as_ptr(),
            std::ptr::null_mut(),
        )
    }
}

#[test]
fn formatting_keeps_utf8_decoration_outside_printf() {
    let decorated = format_value(sys::ImGuiDataType_Double, &12.5_f64, "温度 %.2f °C");
    assert_eq!(decorated, "温度 12.50 °C");

    let mut parsed = 0.0_f64;
    assert!(apply_value(
        sys::ImGuiDataType_Double,
        &mut parsed,
        &decorated,
        "温度 %.2f °C"
    ));
    assert_eq!(parsed, 12.5);

    assert_eq!(
        format_value(sys::ImGuiDataType_S32, &7_i32, "load %% complete"),
        "load % complete"
    );
}

#[test]
fn numeric_suffixes_round_trip_without_becoming_part_of_the_value() {
    let mut decimal = 0_i32;
    let decimal_text = format_value(sys::ImGuiDataType_S32, &5_i32, "%d123");
    assert_eq!(decimal_text, "5123");
    assert!(apply_value(
        sys::ImGuiDataType_S32,
        &mut decimal,
        &decimal_text,
        "%d123"
    ));
    assert_eq!(decimal, 5);

    let mut hexadecimal = 0_u32;
    let hexadecimal_text = format_value(sys::ImGuiDataType_U32, &10_u32, "%xF");
    assert_eq!(hexadecimal_text, "aF");
    assert!(apply_value(
        sys::ImGuiDataType_U32,
        &mut hexadecimal,
        &hexadecimal_text,
        "%xF"
    ));
    assert_eq!(hexadecimal, 10);

    let mut floating = 0.0_f32;
    let floating_text = format_value(sys::ImGuiDataType_Float, &1.0_f32, "%fe3");
    assert_eq!(floating_text, "1.000000e3");
    assert!(apply_value(
        sys::ImGuiDataType_Float,
        &mut floating,
        &floating_text,
        "%fe3"
    ));
    assert_eq!(floating, 1.0);
}

#[test]
fn parsing_clamps_overflow_and_rejects_invalid_values_without_writing() {
    let mut signed = 0_i8;
    assert!(apply_value(
        sys::ImGuiDataType_S8,
        &mut signed,
        "1000",
        "%d"
    ));
    assert_eq!(signed, i8::MAX);

    let mut unsigned = 7_u8;
    assert!(!apply_value(
        sys::ImGuiDataType_U8,
        &mut unsigned,
        "-1",
        "%u"
    ));
    assert_eq!(unsigned, 7);

    let mut not_a_number = 3.0_f32;
    assert!(!apply_value(
        sys::ImGuiDataType_Float,
        &mut not_a_number,
        "nan",
        "%f"
    ));
    assert_eq!(not_a_number, 3.0);

    let mut infinity = 0.0_f32;
    assert!(apply_value(
        sys::ImGuiDataType_Float,
        &mut infinity,
        "inf",
        "%f"
    ));
    assert_eq!(infinity, f32::MAX);
}

#[test]
fn every_supported_numeric_carrier_formats_and_parses_with_its_exact_type() {
    macro_rules! assert_round_trip {
        ($data_type:expr, $value_type:ty, $value:expr, $format:expr, $expected:expr) => {{
            let value: $value_type = $value;
            let text = format_value($data_type, &value, $format);
            assert_eq!(text, $expected);
            let mut parsed = <$value_type>::default();
            assert!(apply_value($data_type, &mut parsed, &text, $format));
            assert_eq!(parsed, value);
        }};
    }

    assert_round_trip!(sys::ImGuiDataType_S8, i8, -12, "%d", "-12");
    assert_round_trip!(sys::ImGuiDataType_U8, u8, 250, "%u", "250");
    assert_round_trip!(sys::ImGuiDataType_S16, i16, -1234, "%d", "-1234");
    assert_round_trip!(sys::ImGuiDataType_U16, u16, 60_000, "%u", "60000");
    assert_round_trip!(
        sys::ImGuiDataType_S32,
        i32,
        -2_000_000_000,
        "%d",
        "-2000000000"
    );
    assert_round_trip!(
        sys::ImGuiDataType_U32,
        u32,
        4_000_000_000,
        "%u",
        "4000000000"
    );

    #[cfg(target_env = "msvc")]
    let (signed_64_format, unsigned_64_format) = ("%I64d", "%I64u");
    #[cfg(not(target_env = "msvc"))]
    let (signed_64_format, unsigned_64_format) = ("%lld", "%llu");

    assert_round_trip!(
        sys::ImGuiDataType_S64,
        i64,
        i64::MIN,
        signed_64_format,
        "-9223372036854775808"
    );
    assert_round_trip!(
        sys::ImGuiDataType_U64,
        u64,
        u64::MAX,
        unsigned_64_format,
        "18446744073709551615"
    );
    assert_round_trip!(sys::ImGuiDataType_Float, f32, 1.5, "%.1f", "1.5");
    assert_round_trip!(sys::ImGuiDataType_Double, f64, -2.25, "%.2f", "-2.25");
}

#[test]
fn numeric_buffers_handle_the_safe_contract_limits_and_fail_closed_beyond_them() {
    let maximum_fixed = format_value(sys::ImGuiDataType_Double, &f64::MAX, "%.99f");
    assert_eq!(maximum_fixed.len(), 409);
    assert!(maximum_fixed.ends_with(&format!(".{:0<99}", "")));

    let (written, truncated) =
        format_value_with_capacity(sys::ImGuiDataType_S32, &123_i32, "prefix %d suffix", 8);
    assert_eq!(written, 7);
    assert_eq!(truncated, "prefix ");

    let mut accepted = 0_u64;
    assert!(apply_value(
        sys::ImGuiDataType_U64,
        &mut accepted,
        &"9".repeat(511),
        "%llu"
    ));
    assert_eq!(accepted, u64::MAX);

    let mut rejected = 7_u64;
    assert!(!apply_value(
        sys::ImGuiDataType_U64,
        &mut rejected,
        &"9".repeat(512),
        "%llu"
    ));
    assert_eq!(rejected, 7);
}
