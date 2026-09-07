//! Local streaming interface matching the migrated algorithms' input/output types.
pub trait Indicator {
    type Input;
    type Output;
    fn update(&mut self, input: Self::Input) -> Option<Self::Output>;
    fn reset(&mut self);
    fn warmup_period(&self) -> usize;
    fn is_ready(&self) -> bool;
    fn name(&self) -> &'static str;
}
/// Batch replay uses exactly the streaming recurrence.
pub trait BatchExt: Indicator {
    fn batch(&mut self, inputs: &[Self::Input]) -> Vec<Option<Self::Output>>
    where
        Self::Input: Clone,
    {
        inputs
            .iter()
            .cloned()
            .map(|input| self.update(input))
            .collect()
    }
}
impl<T: Indicator> BatchExt for T {}
