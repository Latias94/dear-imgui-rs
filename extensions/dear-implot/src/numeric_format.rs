use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use dear_imgui_rs::{NumericFormat, NumericFormatError};

/// A validated floating-point format for ImPlot values.
///
/// ImPlot promotes the corresponding values to `double` before formatting.
/// Unlike core Dear ImGui numeric widgets, ImPlot forwards the complete format
/// to native printf paths, so this type also requires ASCII text. Use an axis
/// formatter closure when localized or arbitrary UTF-8 output is required.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatFormat<'a>(NumericFormat<'a, f64>);

impl<'a> FloatFormat<'a> {
    /// Validates a floating-point format accepted by ImPlot's native formatters.
    pub fn new(storage: impl Into<Cow<'a, str>>) -> Result<Self, FloatFormatError> {
        let format = NumericFormat::new(storage)?;
        if let Some(byte_offset) = format.as_str().bytes().position(|byte| !byte.is_ascii()) {
            return Err(FloatFormatError::NonAscii { byte_offset });
        }
        Ok(Self(format))
    }

    /// Returns the validated format string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Borrows this format without revalidating it.
    pub fn borrowed(&self) -> FloatFormat<'_> {
        FloatFormat(self.0.borrowed())
    }

    /// Converts this format into an owned value.
    pub fn into_owned(self) -> FloatFormat<'static> {
        FloatFormat(self.0.into_owned())
    }

    /// Returns the underlying typed Dear ImGui numeric format.
    pub fn into_numeric_format(self) -> NumericFormat<'a, f64> {
        self.0
    }
}

impl AsRef<str> for FloatFormat<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'a> TryFrom<&'a str> for FloatFormat<'a> {
    type Error = FloatFormatError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for FloatFormat<'static> {
    type Error = FloatFormatError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for FloatFormat<'static> {
    type Err = FloatFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.to_owned())
    }
}

/// Describes why an ImPlot floating-point format was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FloatFormatError {
    /// The C-style numeric directive is invalid for a promoted `double` value.
    Numeric(NumericFormatError),
    /// ImPlot forwards the complete string to a locale-sensitive native formatter.
    NonAscii { byte_offset: usize },
}

impl fmt::Display for FloatFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric(error) => error.fmt(formatter),
            Self::NonAscii { byte_offset } => write!(
                formatter,
                "ImPlot numeric formats must be ASCII; non-ASCII text starts at byte {byte_offset}"
            ),
        }
    }
}

impl std::error::Error for FloatFormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(error) => Some(error),
            Self::NonAscii { .. } => None,
        }
    }
}

impl From<NumericFormatError> for FloatFormatError {
    fn from(error: NumericFormatError) -> Self {
        Self::Numeric(error)
    }
}

/// A validated floating-point format that fits ImPlot's axis storage.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisFormat<'a>(FloatFormat<'a>);

impl<'a> AxisFormat<'a> {
    /// Maximum format length accepted by ImPlot's 16-byte axis buffer.
    pub const MAX_LEN: usize = 15;

    /// Validates a floating-point format and the axis storage limit.
    pub fn new(storage: impl Into<Cow<'a, str>>) -> Result<Self, AxisFormatError> {
        Self::try_from(FloatFormat::new(storage)?)
    }

    /// Returns the validated format string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Borrows this axis format without revalidating it.
    pub fn borrowed(&self) -> AxisFormat<'_> {
        AxisFormat(self.0.borrowed())
    }

    /// Converts this format into an owned value.
    pub fn into_owned(self) -> AxisFormat<'static> {
        AxisFormat(self.0.into_owned())
    }

    /// Returns the underlying general-purpose floating-point format.
    pub fn into_float_format(self) -> FloatFormat<'a> {
        self.0
    }
}

impl<'a> TryFrom<FloatFormat<'a>> for AxisFormat<'a> {
    type Error = AxisFormatError;

    fn try_from(format: FloatFormat<'a>) -> Result<Self, Self::Error> {
        let length = format.as_str().len();
        if length > Self::MAX_LEN {
            return Err(AxisFormatError::TooLong {
                length,
                maximum: Self::MAX_LEN,
            });
        }
        Ok(Self(format))
    }
}

impl AsRef<str> for AxisFormat<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'a> TryFrom<&'a str> for AxisFormat<'a> {
    type Error = AxisFormatError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for AxisFormat<'static> {
    type Error = AxisFormatError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for AxisFormat<'static> {
    type Err = AxisFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.to_owned())
    }
}

/// Describes why an ImPlot axis format was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AxisFormatError {
    /// The underlying numeric format is invalid.
    Format(FloatFormatError),
    /// The complete string does not fit ImPlot's fixed-size axis buffer.
    TooLong { length: usize, maximum: usize },
}

impl fmt::Display for AxisFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(error) => error.fmt(formatter),
            Self::TooLong { length, maximum } => write!(
                formatter,
                "axis format is {length} bytes; ImPlot supports at most {maximum}"
            ),
        }
    }
}

impl std::error::Error for AxisFormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Format(error) => Some(error),
            Self::TooLong { .. } => None,
        }
    }
}

impl From<FloatFormatError> for AxisFormatError {
    fn from(error: FloatFormatError) -> Self {
        Self::Format(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_formats_validate_numeric_type_and_storage_length() {
        assert_eq!(AxisFormat::new("%.3f").unwrap().as_str(), "%.3f");
        assert!(matches!(
            AxisFormat::new("%s"),
            Err(AxisFormatError::Format(_))
        ));
        assert!(matches!(
            FloatFormat::new("%.2f °C"),
            Err(FloatFormatError::NonAscii { .. })
        ));

        let maximum = format!("{}%.1f", "x".repeat(11));
        assert_eq!(maximum.len(), AxisFormat::MAX_LEN);
        assert!(AxisFormat::new(maximum).is_ok());

        let too_long = format!("{}%.1f", "x".repeat(12));
        assert!(matches!(
            AxisFormat::new(too_long),
            Err(AxisFormatError::TooLong { length: 16, .. })
        ));
    }
}
