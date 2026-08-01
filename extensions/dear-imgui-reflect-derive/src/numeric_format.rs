use crate::internal::NumericFormatType;

const MAX_FORMAT_BYTES: usize = 4096;
const MAX_DIRECTIVE_BYTES: usize = 30;
const MAX_FIELD_WIDTH: u32 = 31;
const MAX_PRECISION: u32 = 99;

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

    fn too_large_message(self, byte_offset: usize, value: u32) -> String {
        match self {
            Self::Width => format!(
                "format width {value} at byte {byte_offset} exceeds the supported maximum {}",
                self.maximum()
            ),
            Self::Precision => format!(
                "format precision {value} at byte {byte_offset} exceeds the supported maximum {}",
                self.maximum()
            ),
        }
    }
}

#[cfg(test)]
pub fn validate(format: &str, numeric_type: NumericFormatType) -> Result<(), String> {
    validate_and_normalize(format, numeric_type).map(|_| ())
}

/// Validates a derive literal and normalizes target-specific wide modifiers.
///
/// MSVC's `%I64*` spelling is accepted for migration purposes but emitted as
/// the portable `%ll*` spelling so generated code validates on every target.
/// UTF-8 decoration is preserved, while the complete string, field width, and
/// precision are bounded to match the core runtime contract.
pub fn validate_and_normalize(
    format: &str,
    numeric_type: NumericFormatType,
) -> Result<String, String> {
    if numeric_type == NumericFormatType::PointerSized {
        return Err(
            "custom formats for isize/usize are target-width dependent; use a fixed-width numeric field"
                .to_owned(),
        );
    }

    if format.len() > MAX_FORMAT_BYTES {
        return Err(format!(
            "format is {} UTF-8 bytes; at most {MAX_FORMAT_BYTES} bytes are supported",
            format.len()
        ));
    }

    let bytes = format.as_bytes();
    if let Some(offset) = bytes.iter().position(|byte| *byte == 0) {
        return Err(format!("format contains a NUL byte at byte {offset}"));
    }

    let mut offset = 0;
    let mut conversions = 0;
    let mut normalized = None::<String>;
    let mut copied_until = 0;
    while offset < bytes.len() {
        if bytes[offset] != b'%' {
            offset += 1;
            continue;
        }

        let directive_start = offset;
        offset += 1;
        if offset == bytes.len() {
            return Err(format!(
                "format has an incomplete directive at byte {directive_start}"
            ));
        }
        if bytes[offset] == b'%' {
            offset += 1;
            continue;
        }

        conversions += 1;
        if conversions > 1 {
            return Err(format!(
                "format consumes more than one value at byte {directive_start}"
            ));
        }

        let mut flags = DirectiveFlags::default();
        while offset < bytes.len() && matches!(bytes[offset], b'-' | b'+' | b' ' | b'#' | b'0') {
            flags.record(bytes[offset], offset);
            offset += 1;
        }
        if offset < bytes.len() && matches!(bytes[offset], b'\'' | b'_') {
            return Err(format!(
                "format uses unsupported flag `{}` at byte {offset}",
                char::from(bytes[offset])
            ));
        }

        if offset < bytes.len() && bytes[offset] == b'*' {
            return Err(format!(
                "format requests a dynamic variadic argument at byte {offset}"
            ));
        }
        parse_bounded_decimal(bytes, &mut offset, DecimalComponent::Width)?;
        reject_positional(bytes, offset)?;

        if offset < bytes.len() && bytes[offset] == b'.' {
            offset += 1;
            if offset < bytes.len() && bytes[offset] == b'*' {
                return Err(format!(
                    "format requests a dynamic variadic argument at byte {offset}"
                ));
            }
            parse_bounded_decimal(bytes, &mut offset, DecimalComponent::Precision)?;
            reject_positional(bytes, offset)?;
        }

        let length_offset = offset;
        let (mut length, next_offset) = parse_length(bytes, offset);
        let source_length = length;
        offset = next_offset;
        if offset == bytes.len() {
            return Err(format!(
                "format has an incomplete directive at byte {directive_start}"
            ));
        }
        if source_length == LengthModifier::LongLong {
            let portable_format_length = format.len().saturating_add(1);
            if portable_format_length > MAX_FORMAT_BYTES {
                return Err(format!(
                    "format requires {portable_format_length} UTF-8 bytes on at least one supported target; at most {MAX_FORMAT_BYTES} bytes are supported"
                ));
            }
        }

        if length == LengthModifier::MsvcI64
            && matches!(
                numeric_type,
                NumericFormatType::Signed64 | NumericFormatType::Unsigned64
            )
        {
            let output = normalized.get_or_insert_with(|| String::with_capacity(format.len()));
            output.push_str(&format[copied_until..length_offset]);
            output.push_str("ll");
            copied_until = next_offset;
            length = LengthModifier::LongLong;
        }

        let conversion = bytes[offset];
        offset += 1;
        let directive_length = offset - directive_start;
        let portable_directive_length = if source_length == LengthModifier::LongLong
            && matches!(
                numeric_type,
                NumericFormatType::Signed64 | NumericFormatType::Unsigned64
            ) {
            directive_length + 1
        } else {
            directive_length
        };
        if portable_directive_length > MAX_DIRECTIVE_BYTES {
            return Err(format!(
                "format directive at byte {directive_start} requires {portable_directive_length} bytes on at least one supported target; Dear ImGui supports at most {MAX_DIRECTIVE_BYTES}"
            ));
        }
        validate_conversion(numeric_type, conversion, directive_start)?;
        validate_length(numeric_type, length, length_offset)?;
        if let Some((flag_offset, flag)) = flags.first_incompatible(conversion) {
            return Err(format!(
                "format flag `{}` is incompatible with `%{}` at byte {flag_offset}",
                char::from(flag),
                char::from(conversion)
            ));
        }
    }

    if let Some(mut output) = normalized {
        output.push_str(&format[copied_until..]);
        Ok(output)
    } else {
        Ok(format.to_owned())
    }
}

fn parse_bounded_decimal(
    bytes: &[u8],
    offset: &mut usize,
    component: DecimalComponent,
) -> Result<(), String> {
    let start = *offset;
    let mut value = 0_u32;

    while *offset < bytes.len() && bytes[*offset].is_ascii_digit() {
        let digit = u32::from(bytes[*offset] - b'0');
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(digit))
            .ok_or_else(|| component.too_large_message(start, u32::MAX))?;
        if value > component.maximum() {
            return Err(component.too_large_message(start, value));
        }
        *offset += 1;
    }

    Ok(())
}

fn reject_positional(bytes: &[u8], offset: usize) -> Result<(), String> {
    if offset < bytes.len() && bytes[offset] == b'$' {
        Err(format!(
            "format uses an unsupported positional argument at byte {offset}"
        ))
    } else {
        Ok(())
    }
}

fn validate_conversion(
    numeric_type: NumericFormatType,
    conversion: u8,
    offset: usize,
) -> Result<(), String> {
    let accepted = match numeric_type {
        NumericFormatType::Signed32 | NumericFormatType::Signed64 => {
            matches!(conversion, b'd' | b'i')
        }
        NumericFormatType::Unsigned32 | NumericFormatType::Unsigned64 => {
            matches!(conversion, b'u' | b'o' | b'x' | b'X')
        }
        NumericFormatType::Float => matches!(conversion, b'e' | b'E' | b'f' | b'F' | b'g' | b'G'),
        NumericFormatType::PointerSized => false,
    };
    if accepted {
        Ok(())
    } else {
        Err(format!(
            "format conversion `%{}` does not match the field type at byte {offset}",
            char::from(conversion)
        ))
    }
}

fn validate_length(
    numeric_type: NumericFormatType,
    length: LengthModifier,
    offset: usize,
) -> Result<(), String> {
    let accepted = match numeric_type {
        NumericFormatType::Signed32 | NumericFormatType::Unsigned32 => {
            length == LengthModifier::None
        }
        NumericFormatType::Signed64 | NumericFormatType::Unsigned64 => {
            length == LengthModifier::LongLong
        }
        NumericFormatType::Float => matches!(length, LengthModifier::None | LengthModifier::Long),
        NumericFormatType::PointerSized => false,
    };
    if accepted {
        Ok(())
    } else {
        Err(format!(
            "format length modifier does not match the field type at byte {offset}"
        ))
    }
}

fn parse_length(bytes: &[u8], offset: usize) -> (LengthModifier, usize) {
    let remaining = &bytes[offset..];
    if remaining.starts_with(b"hh") {
        (LengthModifier::Char, offset + 2)
    } else if remaining.starts_with(b"ll") {
        (LengthModifier::LongLong, offset + 2)
    } else if remaining.starts_with(b"I32") {
        (LengthModifier::MsvcI32, offset + 3)
    } else if remaining.starts_with(b"I64") {
        (LengthModifier::MsvcI64, offset + 3)
    } else if let Some(first) = remaining.first() {
        let length = match first {
            b'h' => LengthModifier::Short,
            b'l' => LengthModifier::Long,
            b'j' => LengthModifier::IntMax,
            b'z' => LengthModifier::Size,
            b't' => LengthModifier::PtrDiff,
            b'L' => LengthModifier::LongDouble,
            _ => return (LengthModifier::None, offset),
        };
        (length, offset + 1)
    } else {
        (LengthModifier::None, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_numeric_carriers() {
        assert_eq!(validate("%d", NumericFormatType::Signed32), Ok(()));
        assert_eq!(validate("%u", NumericFormatType::Unsigned32), Ok(()));
        assert_eq!(validate("%lld", NumericFormatType::Signed64), Ok(()));
        assert_eq!(validate("%llX", NumericFormatType::Unsigned64), Ok(()));
        assert_eq!(validate("%.2f%%", NumericFormatType::Float), Ok(()));
    }

    #[test]
    fn matches_the_core_numeric_format_contract_corpus() {
        // Keep this corpus aligned with dear-imgui/src/numeric_format.rs.
        for format in ["%d", "%08i", "value %d", "%+d", "% d"] {
            assert_eq!(validate(format, NumericFormatType::Signed32), Ok(()));
        }
        for format in ["%u", "%08X", "0%o", "%#x", "%#o"] {
            assert_eq!(validate(format, NumericFormatType::Unsigned32), Ok(()));
        }
        assert_eq!(validate("%lld", NumericFormatType::Signed64), Ok(()));
        assert_eq!(validate("0x%016llX", NumericFormatType::Unsigned64), Ok(()));
        for format in [
            "%.3f",
            "%+08.2e ms",
            "%.0f%%",
            "literal %% only",
            "temperature: %.2f °C",
        ] {
            assert_eq!(validate(format, NumericFormatType::Float), Ok(()));
        }
        assert_eq!(validate("%lf", NumericFormatType::Float), Ok(()));

        for format in [
            "%s", "%n", "%p", "%c", "%a", "%*f", "%.*f", "%2$f", "%f %f", "%Lf", "%zu", "%",
        ] {
            assert!(
                validate(format, NumericFormatType::Float).is_err(),
                "{format}"
            );
        }
        assert!(validate("%lld", NumericFormatType::Signed32).is_err());
        assert!(validate("%d", NumericFormatType::Signed64).is_err());
        assert!(validate("%u", NumericFormatType::Signed32).is_err());
        assert!(validate("%d", NumericFormatType::Unsigned32).is_err());
    }

    #[test]
    fn normalizes_msvc_wide_formats_without_touching_literal_text() {
        assert_eq!(
            validate_and_normalize("I64 value: %I64d", NumericFormatType::Signed64),
            Ok("I64 value: %lld".to_owned())
        );
        assert_eq!(
            validate_and_normalize("0x%016I64X", NumericFormatType::Unsigned64),
            Ok("0x%016llX".to_owned())
        );
        assert!(validate_and_normalize("%I64d", NumericFormatType::Signed32).is_err());

        let portable = format!("%{}lld", "0".repeat(25));
        assert_eq!(portable.len(), 29);
        assert_eq!(
            validate_and_normalize(&portable, NumericFormatType::Signed64),
            Ok(portable.clone())
        );
        let too_long_on_msvc = format!("%{}lld", "0".repeat(26));
        assert_eq!(too_long_on_msvc.len(), 30);
        assert!(validate_and_normalize(&too_long_on_msvc, NumericFormatType::Signed64).is_err());
    }

    #[test]
    fn rejects_variadic_and_type_mismatches() {
        for format in ["%s", "%c", "%n", "%*f", "%2$f", "%f %f"] {
            assert!(
                validate(format, NumericFormatType::Float).is_err(),
                "{format}"
            );
        }
        assert!(validate("%x", NumericFormatType::Signed32).is_err());
        assert!(validate("%d", NumericFormatType::Signed64).is_err());
        assert!(validate("%llu", NumericFormatType::Unsigned32).is_err());
        assert!(validate("%f", NumericFormatType::PointerSized).is_err());
    }

    #[test]
    fn width_and_precision_match_the_core_bounds() {
        for format in ["%31d", "%031d"] {
            assert_eq!(validate(format, NumericFormatType::Signed32), Ok(()));
        }
        for format in ["%.99f", "%.099f"] {
            assert_eq!(validate(format, NumericFormatType::Float), Ok(()));
        }

        let width_error = validate_and_normalize("%32d", NumericFormatType::Signed32).unwrap_err();
        assert!(width_error.contains("width 32"), "{width_error}");
        let precision_error =
            validate_and_normalize("%.100f", NumericFormatType::Float).unwrap_err();
        assert!(
            precision_error.contains("precision 100"),
            "{precision_error}"
        );
        assert!(
            validate(
                "%999999999999999999999999999999d",
                NumericFormatType::Signed32
            )
            .is_err()
        );
    }

    #[test]
    fn flag_conversion_rules_match_the_core_contract() {
        for format in ["%#d", "%#i"] {
            assert!(
                validate(format, NumericFormatType::Signed32).is_err(),
                "{format}"
            );
        }
        for format in ["%#u", "%+u", "% u", "%+o", "% x", "%+X"] {
            assert!(
                validate(format, NumericFormatType::Unsigned32).is_err(),
                "{format}"
            );
        }
    }

    #[test]
    fn complete_format_byte_limit_matches_the_core_contract() {
        let accepted = format!("{}x", "界".repeat(1365));
        assert_eq!(accepted.len(), MAX_FORMAT_BYTES);
        assert_eq!(
            validate_and_normalize(&accepted, NumericFormatType::Float),
            Ok(accepted.clone())
        );

        let rejected = format!("{accepted}x");
        let error = validate_and_normalize(&rejected, NumericFormatType::Float).unwrap_err();
        assert!(error.contains("4097 UTF-8 bytes"), "{error}");

        let portable_wide = format!("{}%I64d", "x".repeat(4091));
        assert_eq!(portable_wide.len(), MAX_FORMAT_BYTES);
        assert!(validate_and_normalize(&portable_wide, NumericFormatType::Signed64).is_ok());

        let target_expansion = format!("{}%lld", "x".repeat(4092));
        assert_eq!(target_expansion.len(), MAX_FORMAT_BYTES);
        let error =
            validate_and_normalize(&target_expansion, NumericFormatType::Signed64).unwrap_err();
        assert!(error.contains("requires 4097 UTF-8 bytes"), "{error}");
    }
}
