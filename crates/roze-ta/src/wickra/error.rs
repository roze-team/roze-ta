//! Local structured constructor errors for the selected Wickra algorithms.
pub const MAX_PERIOD: usize = 4096;
pub(crate) const PERIOD_ABOVE_MAX: &str = "period exceeds 4096";
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    PeriodZero,
    InvalidPeriod { message: &'static str },
    NonPositiveMultiplier,
    InvalidCandle { message: &'static str },
    InvalidTick { message: &'static str },
    NonFiniteInput,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
