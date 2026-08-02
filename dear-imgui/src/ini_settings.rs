//! Validated Dear ImGui `.ini` retention configuration.
//!
//! Configure retention before loading settings or starting the first frame.

use std::fmt;
use std::num::NonZeroU16;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use thiserror::Error;

const FIRST_PACKED_YEAR: u16 = 2001;
const LAST_PACKED_YEAR: u16 = 2127;

/// A Gregorian date that Dear ImGui can round-trip through `ImGuiPackedDate`.
///
/// Dear ImGui stores the year in seven bits as an offset from 2000, with zero reserved for an
/// invalid date. The safe range is therefore 2001 through 2127 inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IniSessionDate {
    year: u16,
    month: u8,
    day: u8,
}

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(rename = "IniSessionDate")]
struct IniSessionDateWire {
    year: u16,
    month: u8,
    day: u8,
}

#[cfg(feature = "serde")]
impl Serialize for IniSessionDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        IniSessionDateWire {
            year: self.year,
            month: self.month,
            day: self.day,
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for IniSessionDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = IniSessionDateWire::deserialize(deserializer)?;
        Self::new(wire.year, wire.month, wire.day).map_err(serde::de::Error::custom)
    }
}

impl IniSessionDate {
    /// Creates a date that is representable by Dear ImGui's packed `.ini` format.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, IniSessionDateError> {
        Self::from_parts(u32::from(year), u32::from(month), u32::from(day))
    }

    /// Returns the calendar year.
    #[inline]
    pub const fn year(self) -> u16 {
        self.year
    }

    /// Returns the calendar month in the range 1 through 12.
    #[inline]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// Returns the day of month.
    #[inline]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// Returns the `YYYYMMDD` representation consumed by Dear ImGui.
    #[inline]
    pub const fn as_yyyymmdd(self) -> u32 {
        self.year as u32 * 10_000 + self.month as u32 * 100 + self.day as u32
    }

    /// Returns the largest safe automatic-discard period for this date.
    ///
    /// Dear ImGui subtracts whole months without guarding the packed year against underflow.
    #[inline]
    pub const fn max_auto_discard_months(self) -> u16 {
        (self.year - FIRST_PACKED_YEAR) * 12 + (self.month - 1) as u16
    }

    fn from_parts(year: u32, month: u32, day: u32) -> Result<Self, IniSessionDateError> {
        if !(u32::from(FIRST_PACKED_YEAR)..=u32::from(LAST_PACKED_YEAR)).contains(&year) {
            return Err(IniSessionDateError::YearOutOfRange { year });
        }
        if !(1..=12).contains(&month) {
            return Err(IniSessionDateError::MonthOutOfRange { month });
        }
        let days_in_month = days_in_month(year, month);
        if !(1..=days_in_month).contains(&day) {
            return Err(IniSessionDateError::DayOutOfRange { year, month, day });
        }

        Ok(Self {
            year: year as u16,
            month: month as u8,
            day: day as u8,
        })
    }
}

impl TryFrom<u32> for IniSessionDate {
    type Error = IniSessionDateError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_parts(value / 10_000, (value / 100) % 100, value % 100)
    }
}

impl From<IniSessionDate> for u32 {
    #[inline]
    fn from(value: IniSessionDate) -> Self {
        value.as_yyyymmdd()
    }
}

impl fmt::Display for IniSessionDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// Errors returned when constructing an [`IniSessionDate`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum IniSessionDateError {
    /// The year cannot round-trip through Dear ImGui's packed representation.
    #[error("year {year} is outside Dear ImGui's supported packed-date range 2001 through 2127")]
    YearOutOfRange { year: u32 },
    /// The month is not a Gregorian calendar month.
    #[error("month {month} is outside the Gregorian range 1 through 12")]
    MonthOutOfRange { month: u32 },
    /// The day is invalid for the supplied Gregorian year and month.
    #[error("day {day} is invalid for {year:04}-{month:02}")]
    DayOutOfRange { year: u32, month: u32, day: u32 },
}

/// One coherent `.ini` retention configuration owned by a [`crate::Context`].
///
/// `AutoDiscard` always records last-used dates because Dear ImGui needs those dates to decide
/// which settings to remove. Its month count is bounded again by the session date when applied.
///
/// # Example
///
/// ```no_run
/// use std::num::NonZeroU16;
/// use dear_imgui_rs::{Context, IniSessionDate, IniSettingsRetention};
///
/// let mut context = Context::create();
/// let session_date = IniSessionDate::new(2026, 7, 30).expect("valid date");
/// context
///     .set_ini_settings_retention(IniSettingsRetention::AutoDiscard {
///         session_date,
///         months: NonZeroU16::new(6).unwrap(),
///     })
///     .expect("retention fits the packed date range");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IniSettingsRetention {
    /// Keep every entry. A session date may still be supplied for platform integrations, but no
    /// last-used date is saved and no automatic cleanup runs.
    Disabled {
        /// Optional application session date.
        session_date: Option<IniSessionDate>,
    },
    /// Save last-used dates without automatically removing entries.
    ///
    /// The session date may be absent on platforms built without time functions. Keeping this
    /// state distinct from [`IniSettingsRetention::Disabled`] preserves the configured intent and
    /// allows the native fields to round-trip without silently disabling date recording.
    RecordLastUsed {
        /// The date written into supported `.ini` entries, when the platform provides one.
        session_date: Option<IniSessionDate>,
    },
    /// Save last-used dates and discard entries older than the requested number of months.
    ///
    /// On the first settings load after enabling this policy, Dear ImGui also removes every
    /// supported entry that has no `LastUsed` field. This includes settings written before date
    /// recording was enabled, regardless of their actual age.
    AutoDiscard {
        /// The date used as the upper bound for retention.
        session_date: IniSessionDate,
        /// Number of whole months that Dear ImGui retains.
        months: NonZeroU16,
    },
}

impl IniSettingsRetention {
    /// Creates a disabled retention configuration without a session date.
    #[inline]
    pub const fn disabled() -> Self {
        Self::Disabled { session_date: None }
    }

    /// Returns the configuration's session date, when one is configured.
    #[inline]
    pub const fn session_date(self) -> Option<IniSessionDate> {
        match self {
            Self::Disabled { session_date } => session_date,
            Self::RecordLastUsed { session_date } => session_date,
            Self::AutoDiscard { session_date, .. } => Some(session_date),
        }
    }

    /// Returns the automatic-discard period, when enabled.
    #[inline]
    pub const fn auto_discard_months(self) -> Option<NonZeroU16> {
        match self {
            Self::AutoDiscard { months, .. } => Some(months),
            Self::Disabled { .. } | Self::RecordLastUsed { .. } => None,
        }
    }

    /// Returns whether native last-used date recording is enabled.
    ///
    /// This reports the configured flag. [`IniSettingsRetention::RecordLastUsed`] with no session
    /// date preserves that flag for clockless platforms, but cannot write a date until the
    /// platform supplies one.
    #[inline]
    pub const fn last_used_date_recording_enabled(self) -> bool {
        !matches!(self, Self::Disabled { .. })
    }

    pub(crate) fn validate(self) -> Result<(), IniSettingsRetentionError> {
        if let Self::AutoDiscard {
            session_date,
            months,
        } = self
        {
            let max_months = session_date.max_auto_discard_months();
            if months.get() > max_months {
                return Err(IniSettingsRetentionError::RetentionUnderflowsSessionDate {
                    session_date,
                    months,
                    max_months,
                });
            }
        }
        Ok(())
    }

    pub(crate) const fn raw_parts(self) -> (i32, bool, i32) {
        match self {
            Self::Disabled { session_date } => (
                match session_date {
                    Some(date) => date.as_yyyymmdd() as i32,
                    None => 0,
                },
                false,
                0,
            ),
            Self::RecordLastUsed { session_date } => (
                match session_date {
                    Some(date) => date.as_yyyymmdd() as i32,
                    None => 0,
                },
                true,
                0,
            ),
            Self::AutoDiscard {
                session_date,
                months,
            } => (session_date.as_yyyymmdd() as i32, true, months.get() as i32),
        }
    }

    pub(crate) fn from_raw(
        session_date: i32,
        save_last_used_date: bool,
        auto_discard_months: i32,
    ) -> Result<Self, IniSettingsRetentionError> {
        let session_date = match session_date {
            0 => None,
            raw if raw > 0 => IniSessionDate::try_from(raw as u32)
                .map(Some)
                .map_err(|_| IniSettingsRetentionError::InvalidNativeSessionDate { raw })?,
            raw => return Err(IniSettingsRetentionError::InvalidNativeSessionDate { raw }),
        };
        let months = match auto_discard_months {
            0 => None,
            raw if raw > 0 => {
                NonZeroU16::new(u16::try_from(raw).map_err(|_| {
                    IniSettingsRetentionError::InvalidNativeAutoDiscardMonths { raw }
                })?)
            }
            raw => return Err(IniSettingsRetentionError::InvalidNativeAutoDiscardMonths { raw }),
        };

        match (save_last_used_date, session_date, months) {
            (false, session_date, None) => Ok(Self::Disabled { session_date }),
            (true, session_date, None) => Ok(Self::RecordLastUsed { session_date }),
            (true, Some(session_date), Some(months)) => {
                let retention = Self::AutoDiscard {
                    session_date,
                    months,
                };
                retention.validate()?;
                Ok(retention)
            }
            (false, _, Some(_)) => Err(IniSettingsRetentionError::InvalidNativeState {
                reason: "automatic discard is enabled while last-used dates are disabled",
            }),
            (true, None, Some(_)) => Err(IniSettingsRetentionError::InvalidNativeState {
                reason: "automatic discard is enabled without a session date",
            }),
        }
    }
}

/// Errors returned while reading or applying [`IniSettingsRetention`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum IniSettingsRetentionError {
    /// The requested period would underflow Dear ImGui's packed date year.
    #[error(
        "automatic discard period of {months} months exceeds the {max_months}-month limit for session date {session_date}"
    )]
    RetentionUnderflowsSessionDate {
        /// Date used as the retention upper bound.
        session_date: IniSessionDate,
        /// Requested automatic-discard period.
        months: NonZeroU16,
        /// Largest supported period for `session_date`.
        max_months: u16,
    },
    /// The session date is immutable after Dear ImGui has started its first frame.
    #[error("`.ini` retention must be configured before the first Dear ImGui frame")]
    LockedAfterFirstFrame,
    /// Automatic cleanup cannot be changed after settings have already been loaded.
    #[error("`.ini` retention must be configured before loading Dear ImGui settings")]
    LockedAfterSettingsLoad,
    /// Native platform state contains a date outside the safe packed range.
    #[error("native Platform_SessionDate {raw} is not a valid Dear ImGui packed date")]
    InvalidNativeSessionDate { raw: i32 },
    /// Native IO state contains an unsupported automatic-discard period.
    #[error("native ConfigIniSettingsAutoDiscardMonths {raw} is invalid")]
    InvalidNativeAutoDiscardMonths { raw: i32 },
    /// Native settings fields encode a combination that cannot be represented safely.
    #[error("native `.ini` retention state is invalid: {reason}")]
    InvalidNativeState {
        /// Explanation of the inconsistent native fields.
        reason: &'static str,
    },
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => unreachable!("IniSessionDate validates the month before asking for its day count"),
    }
}

const fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100))
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use super::*;
    use serde_test::{Token, assert_tokens};

    #[test]
    fn session_date_serde_uses_one_stable_wire_shape() {
        let date = IniSessionDate::new(2024, 2, 29).unwrap();
        assert_tokens(
            &date,
            &[
                Token::Struct {
                    name: "IniSessionDate",
                    len: 3,
                },
                Token::Str("year"),
                Token::U16(2024),
                Token::Str("month"),
                Token::U8(2),
                Token::Str("day"),
                Token::U8(29),
                Token::StructEnd,
            ],
        );
    }

    #[test]
    fn session_date_deserialization_preserves_validation() {
        let valid = r#"{"year":2024,"month":2,"day":29}"#;
        assert_eq!(
            serde_json::from_str::<IniSessionDate>(valid).unwrap(),
            IniSessionDate::new(2024, 2, 29).unwrap()
        );

        for invalid in [
            r#"{"year":2000,"month":1,"day":1}"#,
            r#"{"year":2128,"month":1,"day":1}"#,
            r#"{"year":2026,"month":13,"day":1}"#,
            r#"{"year":2100,"month":2,"day":29}"#,
        ] {
            assert!(serde_json::from_str::<IniSessionDate>(invalid).is_err());
        }
    }

    #[test]
    fn retention_deserialization_cannot_bypass_session_date_validation() {
        for invalid in [
            r#"{"Disabled":{"session_date":{"year":2000,"month":1,"day":1}}}"#,
            r#"{"RecordLastUsed":{"session_date":{"year":2128,"month":1,"day":1}}}"#,
            r#"{"AutoDiscard":{"session_date":{"year":2100,"month":2,"day":29},"months":1}}"#,
        ] {
            assert!(serde_json::from_str::<IniSettingsRetention>(invalid).is_err());
        }
    }
}
