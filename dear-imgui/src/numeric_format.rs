use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::Range;
use std::str::FromStr;
use thiserror::Error;

use crate::internal::{DataType, DataTypeKind};

const MAX_FORMAT_BYTES: usize = 4096;
const MAX_DIRECTIVE_BYTES: usize = 30;
const MAX_FIELD_WIDTH: u32 = 31;
const MAX_PRECISION: u32 = 99;

/// A validated C-style format for one Dear ImGui numeric value.
///
/// The value type is part of the format type, so a format validated for `f32`
/// cannot be passed to an integer widget. Borrowed strings remain zero-copy,
/// while owned strings remain stable after validation.
///
/// ```compile_fail
/// use dear_imgui_rs::{Context, NumericFormat};
///
/// let mut context = Context::create();
/// let ui = context.frame();
/// let integer_format = NumericFormat::<u32>::new("%u").unwrap();
/// let _ = ui
///     .slider_config("value", 0.0_f32, 1.0_f32)
///     .display_format(integer_format);
/// ```
///
/// ```compile_fail
/// use dear_imgui_rs::Context;
///
/// let mut context = Context::create();
/// let ui = context.frame();
/// let _ = ui
///     .slider_config("value", 0.0_f32, 1.0_f32)
///     .display_format("%.2f");
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct NumericFormat<'a, T> {
    storage: Cow<'a, str>,
    marker: PhantomData<fn() -> T>,
}

impl<'a, T> NumericFormat<'a, T>
where
    T: DataTypeKind,
{
    /// Validates and retains a numeric format.
    ///
    /// Safe formats may contain ordinary UTF-8 decoration, escaped percent
    /// signs (`%%`), and at most one conversion matching `T`. The portable `ll` and MSVC
    /// `I64` modifiers are normalized for the compilation target. Width is
    /// limited to 31, precision to 99, and the complete UTF-8 string to 4096
    /// bytes so downstream native parsing remains bounded.
    ///
    /// ```
    /// use dear_imgui_rs::NumericFormat;
    ///
    /// let percent = NumericFormat::<f32>::new("%.1f%%")?;
    /// assert_eq!(percent.as_str(), "%.1f%%");
    /// assert!(NumericFormat::<f32>::new("%s").is_err());
    /// # Ok::<(), dear_imgui_rs::NumericFormatError>(())
    /// ```
    pub fn new(storage: impl Into<Cow<'a, str>>) -> Result<Self, NumericFormatError> {
        let storage = storage.into();
        validate_format_length(&storage)?;
        let storage = normalize_wide_integer_length::<T>(storage);
        validate_numeric_format::<T>(&storage)?;
        Ok(Self {
            storage,
            marker: PhantomData,
        })
    }

    /// Creates a numeric format without validation.
    ///
    /// # Safety
    ///
    /// The format must be valid for the exact C variadic argument represented
    /// by `T::KIND`. It must consume at most one value argument, must not use
    /// dynamic width or precision, positional arguments, `%n`, or any other
    /// conversion that consumes a different argument type. Width must not exceed
    /// 31, precision must not exceed 99, flags must be valid for the conversion,
    /// and the complete string must not exceed 4096 bytes. Violating this contract
    /// can cause undefined behavior, including arbitrary memory writes.
    pub unsafe fn new_unchecked(storage: impl Into<Cow<'a, str>>) -> Self {
        Self {
            storage: storage.into(),
            marker: PhantomData,
        }
    }

    /// Returns the validated, target-normalized format string.
    pub fn as_str(&self) -> &str {
        &self.storage
    }

    /// Borrows this format without revalidating it.
    pub fn borrowed(&self) -> NumericFormat<'_, T> {
        NumericFormat {
            storage: Cow::Borrowed(self.as_str()),
            marker: PhantomData,
        }
    }

    /// Converts this format into an owned value that can be stored indefinitely.
    pub fn into_owned(self) -> NumericFormat<'static, T> {
        NumericFormat {
            storage: Cow::Owned(self.storage.into_owned()),
            marker: PhantomData,
        }
    }

    /// Returns the target-normalized string storage.
    pub fn into_inner(self) -> Cow<'a, str> {
        self.storage
    }
}

impl<T> AsRef<str> for NumericFormat<'_, T>
where
    T: DataTypeKind,
{
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'a, T> TryFrom<&'a str> for NumericFormat<'a, T>
where
    T: DataTypeKind,
{
    type Error = NumericFormatError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<T> TryFrom<String> for NumericFormat<'static, T>
where
    T: DataTypeKind,
{
    type Error = NumericFormatError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<T> FromStr for NumericFormat<'static, T>
where
    T: DataTypeKind,
{
    type Err = NumericFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.to_owned())
    }
}

/// Describes why a C-style numeric format was rejected.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum NumericFormatError {
    /// The complete UTF-8 format exceeds the bounded native parsing contract.
    #[error("numeric format is {length} UTF-8 bytes; at most {maximum} bytes are supported")]
    FormatTooLong {
        byte_offset: usize,
        length: usize,
        maximum: usize,
    },
    /// The string contains a NUL byte and cannot be passed as one C string.
    #[error("numeric format contains a NUL byte at byte {byte_offset}")]
    InteriorNul { byte_offset: usize },
    /// A `%` directive is incomplete.
    #[error("numeric format has an incomplete directive at byte {byte_offset}")]
    UnterminatedDirective { byte_offset: usize },
    /// More than one directive would consume a value argument.
    #[error(
        "numeric format requests more than Dear ImGui's one value argument at byte {byte_offset}"
    )]
    MultipleConversions { byte_offset: usize },
    /// `*` requests another variadic width or precision argument.
    #[error("numeric format requests a dynamic variadic argument at byte {byte_offset}")]
    DynamicArgument { byte_offset: usize },
    /// A positional argument such as `%1$d` was requested.
    #[error("numeric format uses an unsupported positional argument at byte {byte_offset}")]
    PositionalArgument { byte_offset: usize },
    /// The directive uses a non-portable or unsupported flag.
    #[error("numeric format uses unsupported flag `{flag}` at byte {byte_offset}")]
    UnsupportedFlag { byte_offset: usize, flag: char },
    /// The flag is defined for printf but not for this numeric conversion.
    #[error(
        "numeric format flag `{flag}` is incompatible with `%{conversion}` at byte {byte_offset}"
    )]
    IncompatibleFlag {
        byte_offset: usize,
        flag: char,
        conversion: char,
    },
    /// The requested field width exceeds the bounded native parsing contract.
    #[error(
        "numeric format width {width} at byte {byte_offset} exceeds the supported maximum {maximum}"
    )]
    WidthTooLarge {
        byte_offset: usize,
        width: u32,
        maximum: u32,
    },
    /// The requested precision exceeds the bounded native parsing contract.
    #[error(
        "numeric format precision {precision} at byte {byte_offset} exceeds the supported maximum {maximum}"
    )]
    PrecisionTooLarge {
        byte_offset: usize,
        precision: u32,
        maximum: u32,
    },
    /// The directive uses a length modifier that does not match the C carrier type.
    #[error(
        "numeric format uses a length modifier that does not match its value type at byte {byte_offset}"
    )]
    UnsupportedLength { byte_offset: usize },
    /// The directive is not a supported numeric conversion.
    #[error("numeric format uses unsupported conversion `%{conversion}` at byte {byte_offset}")]
    UnsupportedConversion {
        byte_offset: usize,
        conversion: char,
    },
    /// The conversion is numeric but does not match the signedness or category of `T`.
    #[error("numeric format conversion does not match its value type at byte {byte_offset}")]
    TypeMismatch { byte_offset: usize },
    /// Dear ImGui copies directives into a fixed 32-byte stack buffer.
    #[error(
        "numeric format directive at byte {byte_offset} requires {length} bytes on a supported target; Dear ImGui supports at most {maximum}",
        maximum = MAX_DIRECTIVE_BYTES
    )]
    DirectiveTooLong { byte_offset: usize, length: usize },
}

impl NumericFormatError {
    /// Returns the byte offset where validation failed.
    pub fn byte_offset(&self) -> usize {
        match *self {
            Self::FormatTooLong { byte_offset, .. }
            | Self::InteriorNul { byte_offset }
            | Self::UnterminatedDirective { byte_offset }
            | Self::MultipleConversions { byte_offset }
            | Self::DynamicArgument { byte_offset }
            | Self::PositionalArgument { byte_offset }
            | Self::UnsupportedFlag { byte_offset, .. }
            | Self::IncompatibleFlag { byte_offset, .. }
            | Self::WidthTooLarge { byte_offset, .. }
            | Self::PrecisionTooLarge { byte_offset, .. }
            | Self::UnsupportedLength { byte_offset }
            | Self::UnsupportedConversion { byte_offset, .. }
            | Self::TypeMismatch { byte_offset }
            | Self::DirectiveTooLong { byte_offset, .. } => byte_offset,
        }
    }
}

fn validate_numeric_format<T>(format: &str) -> Result<(), NumericFormatError>
where
    T: DataTypeKind,
{
    validate_numeric_format_for_data_type(format, T::KIND)
}

fn validate_format_length(format: &str) -> Result<(), NumericFormatError> {
    if format.len() > MAX_FORMAT_BYTES {
        Err(NumericFormatError::FormatTooLong {
            byte_offset: MAX_FORMAT_BYTES,
            length: format.len(),
            maximum: MAX_FORMAT_BYTES,
        })
    } else {
        Ok(())
    }
}

fn normalize_wide_integer_length<'a, T>(storage: Cow<'a, str>) -> Cow<'a, str>
where
    T: DataTypeKind,
{
    if !matches!(T::KIND, DataType::I64 | DataType::U64) {
        return storage;
    }

    let Some(length_range) = find_wide_length_modifier(&storage) else {
        return storage;
    };
    let current = &storage[length_range.clone()];
    let target = if cfg!(target_env = "msvc") {
        "I64"
    } else {
        "ll"
    };
    if current == target {
        return storage;
    }

    let mut normalized = storage.into_owned();
    normalized.replace_range(length_range, target);
    Cow::Owned(normalized)
}

fn find_wide_length_modifier(format: &str) -> Option<Range<usize>> {
    let bytes = format.as_bytes();
    let mut byte_offset = 0;

    while byte_offset < bytes.len() {
        if bytes[byte_offset] != b'%' {
            byte_offset += 1;
            continue;
        }
        byte_offset += 1;
        if byte_offset < bytes.len() && bytes[byte_offset] == b'%' {
            byte_offset += 1;
            continue;
        }

        while byte_offset < bytes.len()
            && matches!(bytes[byte_offset], b'-' | b'+' | b' ' | b'#' | b'0')
        {
            byte_offset += 1;
        }
        if byte_offset < bytes.len() && bytes[byte_offset] == b'*' {
            byte_offset += 1;
        } else {
            while byte_offset < bytes.len() && bytes[byte_offset].is_ascii_digit() {
                byte_offset += 1;
            }
        }
        if byte_offset < bytes.len() && bytes[byte_offset] == b'.' {
            byte_offset += 1;
            if byte_offset < bytes.len() && bytes[byte_offset] == b'*' {
                byte_offset += 1;
            } else {
                while byte_offset < bytes.len() && bytes[byte_offset].is_ascii_digit() {
                    byte_offset += 1;
                }
            }
        }

        if bytes[byte_offset..].starts_with(b"ll") {
            return Some(byte_offset..byte_offset + 2);
        }
        if bytes[byte_offset..].starts_with(b"I64") {
            return Some(byte_offset..byte_offset + 3);
        }
        return None;
    }

    None
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ArgumentKind {
    SignedInteger,
    UnsignedInteger,
    SignedWideInteger,
    UnsignedWideInteger,
    Double,
}

impl ArgumentKind {
    fn from_data_type(data_type: DataType) -> Self {
        match data_type {
            DataType::I8 | DataType::I16 | DataType::I32 => Self::SignedInteger,
            DataType::U8 | DataType::U16 | DataType::U32 => Self::UnsignedInteger,
            DataType::I64 => Self::SignedWideInteger,
            DataType::U64 => Self::UnsignedWideInteger,
            DataType::F32 | DataType::F64 => Self::Double,
        }
    }

    fn accepts_conversion(self, conversion: u8) -> bool {
        match self {
            Self::SignedInteger | Self::SignedWideInteger => matches!(conversion, b'd' | b'i'),
            Self::UnsignedInteger | Self::UnsignedWideInteger => {
                matches!(conversion, b'u' | b'o' | b'x' | b'X')
            }
            Self::Double => matches!(conversion, b'e' | b'E' | b'f' | b'F' | b'g' | b'G'),
        }
    }

    fn accepts_length(self, length: LengthModifier) -> bool {
        match self {
            Self::SignedInteger | Self::UnsignedInteger => length == LengthModifier::None,
            Self::SignedWideInteger | Self::UnsignedWideInteger => {
                length == LengthModifier::LongLong
                    || cfg!(target_env = "msvc") && length == LengthModifier::MsvcI64
            }
            Self::Double => matches!(length, LengthModifier::None | LengthModifier::Long),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum LengthModifier {
    None,
    Char,
    Short,
    Long,
    LongLong,
    IntMax,
    Size,
    PtrDiff,
    LongDouble,
    MsvcI32,
    MsvcI64,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct DirectiveFlags {
    alternate: Option<usize>,
    explicit_sign: Option<usize>,
    leading_space: Option<usize>,
}

impl DirectiveFlags {
    fn record(&mut self, flag: u8, byte_offset: usize) {
        match flag {
            b'#' => {
                let _ = self.alternate.get_or_insert(byte_offset);
            }
            b'+' => {
                let _ = self.explicit_sign.get_or_insert(byte_offset);
            }
            b' ' => {
                let _ = self.leading_space.get_or_insert(byte_offset);
            }
            b'-' | b'0' => {}
            _ => {}
        }
    }

    fn first_incompatible(self, conversion: u8) -> Option<(usize, u8)> {
        let mut incompatible = [None; 3];
        if matches!(conversion, b'd' | b'i' | b'u') {
            incompatible[0] = self.alternate.map(|offset| (offset, b'#'));
        }
        if matches!(conversion, b'u' | b'o' | b'x' | b'X') {
            incompatible[1] = self.explicit_sign.map(|offset| (offset, b'+'));
            incompatible[2] = self.leading_space.map(|offset| (offset, b' '));
        }
        incompatible
            .into_iter()
            .flatten()
            .min_by_key(|(offset, _)| *offset)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum DecimalComponent {
    Width,
    Precision,
}

impl DecimalComponent {
    fn maximum(self) -> u32 {
        match self {
            Self::Width => MAX_FIELD_WIDTH,
            Self::Precision => MAX_PRECISION,
        }
    }

    fn too_large(self, byte_offset: usize, value: u32) -> NumericFormatError {
        match self {
            Self::Width => NumericFormatError::WidthTooLarge {
                byte_offset,
                width: value,
                maximum: self.maximum(),
            },
            Self::Precision => NumericFormatError::PrecisionTooLarge {
                byte_offset,
                precision: value,
                maximum: self.maximum(),
            },
        }
    }
}

fn validate_numeric_format_for_data_type(
    format: &str,
    data_type: DataType,
) -> Result<(), NumericFormatError> {
    validate_format_length(format)?;
    let bytes = format.as_bytes();
    if let Some(byte_offset) = bytes.iter().position(|byte| *byte == 0) {
        return Err(NumericFormatError::InteriorNul { byte_offset });
    }

    let argument_kind = ArgumentKind::from_data_type(data_type);
    let mut byte_offset = 0;
    let mut value_conversions = 0;

    while byte_offset < bytes.len() {
        if bytes[byte_offset] != b'%' {
            byte_offset += 1;
            continue;
        }

        let conversion_start = byte_offset;
        byte_offset += 1;
        if byte_offset == bytes.len() {
            return Err(NumericFormatError::UnterminatedDirective {
                byte_offset: conversion_start,
            });
        }
        if bytes[byte_offset] == b'%' {
            byte_offset += 1;
            continue;
        }

        value_conversions += 1;
        if value_conversions > 1 {
            return Err(NumericFormatError::MultipleConversions {
                byte_offset: conversion_start,
            });
        }

        let mut flags = DirectiveFlags::default();
        while byte_offset < bytes.len()
            && matches!(bytes[byte_offset], b'-' | b'+' | b' ' | b'#' | b'0')
        {
            flags.record(bytes[byte_offset], byte_offset);
            byte_offset += 1;
        }
        reject_unsupported_flag(bytes, byte_offset)?;

        if byte_offset < bytes.len() && bytes[byte_offset] == b'*' {
            return Err(NumericFormatError::DynamicArgument { byte_offset });
        }
        parse_bounded_decimal(bytes, &mut byte_offset, DecimalComponent::Width)?;
        reject_positional_argument(bytes, byte_offset)?;

        if byte_offset < bytes.len() && bytes[byte_offset] == b'.' {
            byte_offset += 1;
            if byte_offset < bytes.len() && bytes[byte_offset] == b'*' {
                return Err(NumericFormatError::DynamicArgument { byte_offset });
            }
            parse_bounded_decimal(bytes, &mut byte_offset, DecimalComponent::Precision)?;
            reject_positional_argument(bytes, byte_offset)?;
        }

        let length_offset = byte_offset;
        let (length, next_offset) = parse_length_modifier(bytes, byte_offset);
        byte_offset = next_offset;
        if byte_offset == bytes.len() {
            return Err(NumericFormatError::UnterminatedDirective {
                byte_offset: conversion_start,
            });
        }
        if length == LengthModifier::LongLong {
            let portable_format_length = format.len().saturating_add(1);
            if portable_format_length > MAX_FORMAT_BYTES {
                return Err(NumericFormatError::FormatTooLong {
                    byte_offset: MAX_FORMAT_BYTES,
                    length: portable_format_length,
                    maximum: MAX_FORMAT_BYTES,
                });
            }
        }

        let conversion = bytes[byte_offset];
        byte_offset += 1;
        let directive_length = byte_offset - conversion_start;
        let portable_directive_length = if length == LengthModifier::LongLong {
            directive_length.saturating_add(1)
        } else {
            directive_length
        };
        if portable_directive_length > MAX_DIRECTIVE_BYTES {
            return Err(NumericFormatError::DirectiveTooLong {
                byte_offset: conversion_start,
                length: portable_directive_length,
            });
        }
        if !is_supported_numeric_conversion(conversion) {
            return Err(NumericFormatError::UnsupportedConversion {
                byte_offset: conversion_start,
                conversion: char::from(conversion),
            });
        }
        if !argument_kind.accepts_conversion(conversion) {
            return Err(NumericFormatError::TypeMismatch {
                byte_offset: conversion_start,
            });
        }
        if !argument_kind.accepts_length(length) {
            return Err(NumericFormatError::UnsupportedLength {
                byte_offset: length_offset,
            });
        }
        if let Some((flag_offset, flag)) = flags.first_incompatible(conversion) {
            return Err(NumericFormatError::IncompatibleFlag {
                byte_offset: flag_offset,
                flag: char::from(flag),
                conversion: char::from(conversion),
            });
        }
    }

    Ok(())
}

fn parse_bounded_decimal(
    bytes: &[u8],
    byte_offset: &mut usize,
    component: DecimalComponent,
) -> Result<(), NumericFormatError> {
    let start = *byte_offset;
    let mut value = 0_u32;

    while *byte_offset < bytes.len() && bytes[*byte_offset].is_ascii_digit() {
        let digit = u32::from(bytes[*byte_offset] - b'0');
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(digit))
            .ok_or_else(|| component.too_large(start, u32::MAX))?;
        if value > component.maximum() {
            return Err(component.too_large(start, value));
        }
        *byte_offset += 1;
    }

    Ok(())
}

fn reject_unsupported_flag(bytes: &[u8], byte_offset: usize) -> Result<(), NumericFormatError> {
    if byte_offset < bytes.len() && matches!(bytes[byte_offset], b'\'' | b'_') {
        Err(NumericFormatError::UnsupportedFlag {
            byte_offset,
            flag: char::from(bytes[byte_offset]),
        })
    } else {
        Ok(())
    }
}

fn reject_positional_argument(bytes: &[u8], byte_offset: usize) -> Result<(), NumericFormatError> {
    if byte_offset < bytes.len() && bytes[byte_offset] == b'$' {
        Err(NumericFormatError::PositionalArgument { byte_offset })
    } else {
        Ok(())
    }
}

fn parse_length_modifier(bytes: &[u8], byte_offset: usize) -> (LengthModifier, usize) {
    let remaining = &bytes[byte_offset..];
    if remaining.starts_with(b"hh") {
        (LengthModifier::Char, byte_offset + 2)
    } else if remaining.starts_with(b"ll") {
        (LengthModifier::LongLong, byte_offset + 2)
    } else if remaining.starts_with(b"I32") {
        (LengthModifier::MsvcI32, byte_offset + 3)
    } else if remaining.starts_with(b"I64") {
        (LengthModifier::MsvcI64, byte_offset + 3)
    } else if let Some(first) = remaining.first() {
        let length = match first {
            b'h' => LengthModifier::Short,
            b'l' => LengthModifier::Long,
            b'j' => LengthModifier::IntMax,
            b'z' => LengthModifier::Size,
            b't' => LengthModifier::PtrDiff,
            b'L' => LengthModifier::LongDouble,
            _ => return (LengthModifier::None, byte_offset),
        };
        (length, byte_offset + 1)
    } else {
        (LengthModifier::None, byte_offset)
    }
}

fn is_supported_numeric_conversion(conversion: u8) -> bool {
    matches!(
        conversion,
        b'd' | b'i' | b'u' | b'o' | b'x' | b'X' | b'e' | b'E' | b'f' | b'F' | b'g' | b'G'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_formats_retain_borrowed_and_owned_storage() {
        let borrowed = NumericFormat::<f32>::new("%.3f").unwrap();
        assert_eq!(borrowed.as_str(), "%.3f");
        assert!(matches!(borrowed.clone().into_inner(), Cow::Borrowed(_)));

        let owned = NumericFormat::<u32>::new(String::from("0x%08X")).unwrap();
        assert_eq!(owned.as_str(), "0x%08X");
        assert!(matches!(owned.clone().into_inner(), Cow::Owned(_)));
        assert_eq!(owned.into_inner().as_ref(), "0x%08X");
    }

    #[test]
    fn formats_can_be_reborrowed_or_made_owned_without_revalidation() {
        let text = String::from("%.2f");
        let borrowed = NumericFormat::<f32>::new(text.as_str()).unwrap();
        assert_eq!(borrowed.borrowed().as_str(), "%.2f");

        let owned = borrowed.into_owned();
        drop(text);
        assert_eq!(owned.as_str(), "%.2f");
    }

    #[test]
    fn floating_formats_allow_one_conversion_and_literal_percent_signs() {
        for format in [
            "%.3f",
            "%+08.2e ms",
            "%.0f%%",
            "literal %% only",
            "plain text",
            "temperature: %.2f °C",
        ] {
            assert_eq!(validate_numeric_format::<f32>(format), Ok(()), "{format}");
            assert_eq!(validate_numeric_format::<f64>(format), Ok(()), "{format}");
        }
        assert_eq!(validate_numeric_format::<f64>("%lf"), Ok(()));
    }

    #[test]
    fn integer_formats_match_signedness_and_carrier_width() {
        for format in ["%d", "%08i", "value %d", "%+d", "% d"] {
            assert_eq!(validate_numeric_format::<i32>(format), Ok(()), "{format}");
        }
        for format in ["%u", "%08X", "0%o", "%#x", "%#o"] {
            assert_eq!(validate_numeric_format::<u32>(format), Ok(()), "{format}");
        }
        let signed = NumericFormat::<i64>::new("%lld").unwrap();
        let unsigned = NumericFormat::<u64>::new("0x%016I64X").unwrap();
        if cfg!(target_env = "msvc") {
            assert_eq!(signed.as_str(), "%I64d");
            assert_eq!(unsigned.as_str(), "0x%016I64X");
        } else {
            assert_eq!(signed.as_str(), "%lld");
            assert_eq!(unsigned.as_str(), "0x%016llX");
        }
    }

    #[test]
    fn formats_cannot_consume_missing_or_mistyped_arguments() {
        for format in [
            "%s", "%n", "%p", "%c", "%a", "%*f", "%.*f", "%2$f", "%f %f", "%Lf", "%zu", "%",
        ] {
            assert!(validate_numeric_format::<f32>(format).is_err(), "{format}");
        }

        assert!(validate_numeric_format::<i32>("%lld").is_err());
        assert!(validate_numeric_format::<i64>("%d").is_err());
        assert!(validate_numeric_format::<i32>("%u").is_err());
        assert!(validate_numeric_format::<u32>("%d").is_err());
        assert!(validate_numeric_format::<f64>("%d").is_err());
    }

    #[test]
    fn directives_must_fit_dear_imguis_sanitization_buffer() {
        let accepted = format!("%{}f", "0".repeat(28));
        assert_eq!(accepted.len(), 30);
        assert_eq!(validate_numeric_format::<f64>(&accepted), Ok(()));

        let rejected = format!("%{}f", "0".repeat(29));
        assert_eq!(rejected.len(), 31);
        assert!(matches!(
            validate_numeric_format::<f64>(&rejected),
            Err(NumericFormatError::DirectiveTooLong { length: 31, .. })
        ));
    }

    #[test]
    fn width_and_precision_are_parsed_with_bounded_arithmetic() {
        for format in ["%31d", "%031d"] {
            assert_eq!(validate_numeric_format::<i32>(format), Ok(()), "{format}");
        }
        for format in ["%.99f", "%.099f"] {
            assert_eq!(validate_numeric_format::<f64>(format), Ok(()), "{format}");
        }

        assert!(matches!(
            validate_numeric_format::<i32>("%32d"),
            Err(NumericFormatError::WidthTooLarge {
                width: 32,
                maximum: 31,
                ..
            })
        ));
        assert!(matches!(
            validate_numeric_format::<f64>("%.100f"),
            Err(NumericFormatError::PrecisionTooLarge {
                precision: 100,
                maximum: 99,
                ..
            })
        ));
        assert!(matches!(
            validate_numeric_format::<i32>("%999999999999999999999999999999d"),
            Err(NumericFormatError::WidthTooLarge { .. })
        ));
    }

    #[test]
    fn flags_must_have_defined_semantics_for_the_conversion() {
        for format in ["%#d", "%#i"] {
            assert!(matches!(
                validate_numeric_format::<i32>(format),
                Err(NumericFormatError::IncompatibleFlag { flag: '#', .. })
            ));
        }
        for format in ["%#u", "%+u", "% u", "%+o", "% x", "%+X"] {
            assert!(
                matches!(
                    validate_numeric_format::<u32>(format),
                    Err(NumericFormatError::IncompatibleFlag { .. })
                ),
                "{format}"
            );
        }
    }

    #[test]
    fn complete_formats_have_a_utf8_byte_limit() {
        let accepted = format!("{}x", "界".repeat(1365));
        assert_eq!(accepted.len(), MAX_FORMAT_BYTES);
        assert_eq!(validate_numeric_format::<f64>(&accepted), Ok(()));

        let rejected = format!("{accepted}x");
        assert!(matches!(
            validate_numeric_format::<f64>(&rejected),
            Err(NumericFormatError::FormatTooLong {
                byte_offset: MAX_FORMAT_BYTES,
                length: 4097,
                maximum: MAX_FORMAT_BYTES,
            })
        ));

        let portable_wide = format!("{}%I64d", "x".repeat(4091));
        assert_eq!(portable_wide.len(), MAX_FORMAT_BYTES);
        assert!(NumericFormat::<i64>::new(portable_wide).is_ok());

        let target_expansion = format!("{}%lld", "x".repeat(4092));
        assert_eq!(target_expansion.len(), MAX_FORMAT_BYTES);
        assert!(matches!(
            NumericFormat::<i64>::new(target_expansion),
            Err(NumericFormatError::FormatTooLong {
                length: 4097,
                maximum: MAX_FORMAT_BYTES,
                ..
            })
        ));
    }

    #[test]
    fn validation_reports_the_first_unsafe_construct() {
        let error = validate_numeric_format::<f64>("value: %f then %n").unwrap_err();
        assert_eq!(error.byte_offset(), 15);
        assert!(matches!(
            error,
            NumericFormatError::MultipleConversions { .. }
        ));

        let error = validate_numeric_format::<f64>("\0%f").unwrap_err();
        assert_eq!(error.byte_offset(), 0);
        assert!(matches!(error, NumericFormatError::InteriorNul { .. }));
    }
}
