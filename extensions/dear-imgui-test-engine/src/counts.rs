use std::num::NonZeroU32;

/// Positive script count or frame count for test-engine actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptCount(NonZeroU32);

impl ScriptCount {
    /// Create a positive script count.
    #[inline]
    pub const fn from_nonzero(count: NonZeroU32) -> Self {
        Self(count)
    }

    /// Create a positive script count or frame count.
    ///
    /// Panics if `count` is zero or exceeds the test engine's `int` range.
    #[inline]
    pub const fn new(count: u32) -> Self {
        assert!(count > 0, "ScriptCount::new() requires a non-zero count");
        assert!(
            count <= i32::MAX as u32,
            "ScriptCount::new() count exceeded i32::MAX"
        );
        match NonZeroU32::new(count) {
            Some(count) => Self(count),
            None => unreachable!(),
        }
    }

    #[inline]
    pub(super) fn raw(self) -> i32 {
        self.0.get() as i32
    }
}

impl From<NonZeroU32> for ScriptCount {
    fn from(count: NonZeroU32) -> Self {
        Self::from_nonzero(count)
    }
}

/// Optional non-negative limit for batch item actions.
///
/// The upstream test engine uses `-1` to mean "no limit". This type keeps that
/// sentinel out of the safe Rust API while still allowing bounded depths/passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptLimit(Option<u32>);

impl ScriptLimit {
    /// No limit.
    pub const ALL: Self = Self(None);

    /// Create a non-negative limit.
    ///
    /// Panics if `limit` exceeds the test engine's `int` range.
    #[inline]
    pub const fn new(limit: u32) -> Self {
        assert!(
            limit <= i32::MAX as u32,
            "ScriptLimit::new() limit exceeded i32::MAX"
        );
        Self(Some(limit))
    }

    #[inline]
    pub(super) fn raw(self) -> i32 {
        match self.0 {
            Some(limit) => limit as i32,
            None => -1,
        }
    }
}

impl From<u32> for ScriptLimit {
    fn from(limit: u32) -> Self {
        Self::new(limit)
    }
}
