/// Binning methods for histograms
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinMethod {
    Sqrt = -1,
    Sturges = -2,
    Rice = -3,
    Scott = -4,
}

/// Histogram bin selector.
///
/// ImPlot uses positive integers for concrete bin counts and negative integer
/// sentinels for automatic binning methods. This type keeps those two meanings
/// explicit in the safe Rust API.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HistogramBins {
    /// Use a concrete positive bin count.
    Count(usize),
    /// Use an automatic ImPlot binning method.
    Method(BinMethod),
}

impl HistogramBins {
    /// The default ImPlot histogram binning method.
    pub const DEFAULT: Self = Self::Method(BinMethod::Sturges);

    /// Create a concrete positive bin count.
    pub const fn count(count: usize) -> Self {
        Self::Count(count)
    }

    /// Create an automatic binning-method selector.
    pub const fn method(method: BinMethod) -> Self {
        Self::Method(method)
    }

    pub(crate) fn raw(self, caller: &str) -> i32 {
        match self {
            Self::Count(count) => {
                assert!(count > 0, "{caller} bin count must be positive");
                i32::try_from(count)
                    .unwrap_or_else(|_| panic!("{caller} bin count exceeded ImPlot's i32 range"))
            }
            Self::Method(method) => method as i32,
        }
    }
}

impl Default for HistogramBins {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl From<usize> for HistogramBins {
    fn from(count: usize) -> Self {
        Self::Count(count)
    }
}

impl From<BinMethod> for HistogramBins {
    fn from(method: BinMethod) -> Self {
        Self::Method(method)
    }
}
