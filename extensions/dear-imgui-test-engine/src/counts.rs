use std::num::NonZeroU32;

use crate::{TestEngineError, TestEngineResult};

/// Positive script count or frame count for test-engine actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptCount(NonZeroU32);

impl ScriptCount {
    /// Creates a positive count after validating the native `int` boundary.
    pub fn new(count: u32) -> TestEngineResult<Self> {
        Self::try_from(count)
    }

    /// Creates a count from a non-zero value after validating the native `int` boundary.
    pub fn from_nonzero(count: NonZeroU32) -> TestEngineResult<Self> {
        Self::new(count.get())
    }

    #[inline]
    pub(super) fn raw(self) -> i32 {
        self.0.get() as i32
    }
}

impl TryFrom<u32> for ScriptCount {
    type Error = TestEngineError;

    fn try_from(count: u32) -> Result<Self, Self::Error> {
        let value = NonZeroU32::new(count).ok_or_else(|| {
            TestEngineError::invalid_input(
                "ScriptCount::new",
                "count",
                "count must be greater than zero",
            )
        })?;
        if count > i32::MAX as u32 {
            return Err(TestEngineError::invalid_input(
                "ScriptCount::new",
                "count",
                "count exceeds the native i32 range",
            ));
        }
        Ok(Self(value))
    }
}

/// Bounded positive limit or the explicit upstream `ALL` sentinel.
///
/// The representation is private so values beyond the native `i32` range cannot be
/// constructed through a public enum variant.
///
/// ```compile_fail
/// use dear_imgui_test_engine::ScriptLimit;
/// use std::num::NonZeroU32;
///
/// let _ = ScriptLimit(Some(NonZeroU32::MAX));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptLimit(Option<NonZeroU32>);

impl ScriptLimit {
    /// The only unbounded value accepted by the safe API.
    pub const ALL: Self = Self(None);

    /// Creates a positive limit. Zero is intentionally rejected.
    pub fn new(limit: u32) -> TestEngineResult<Self> {
        let value = NonZeroU32::new(limit).ok_or_else(|| {
            TestEngineError::invalid_input(
                "ScriptLimit::new",
                "limit",
                "limit must be zero-free; use ScriptLimit::ALL for an unbounded operation",
            )
        })?;
        Self::from_nonzero(value)
    }

    /// Creates a positive limit from a non-zero value.
    pub fn from_nonzero(limit: NonZeroU32) -> TestEngineResult<Self> {
        if limit.get() > i32::MAX as u32 {
            return Err(TestEngineError::invalid_input(
                "ScriptLimit::new",
                "limit",
                "limit exceeds the native i32 range",
            ));
        }
        Ok(Self(Some(limit)))
    }

    #[inline]
    pub(super) fn raw(self) -> i32 {
        match self.0 {
            None => -1,
            Some(limit) => limit.get() as i32,
        }
    }
}

impl TryFrom<u32> for ScriptLimit {
    type Error = TestEngineError;

    fn try_from(limit: u32) -> Result<Self, Self::Error> {
        Self::new(limit)
    }
}
