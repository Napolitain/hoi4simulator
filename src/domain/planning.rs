use super::calendar::GameDate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MilestoneKind {
    Economic,
    Readiness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub name: &'static str,
    pub date: GameDate,
    pub kind: MilestoneKind,
}

impl Milestone {
    pub const fn new(name: &'static str, date: GameDate, kind: MilestoneKind) -> Self {
        Self { name, date, kind }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetBand {
    pub min: u16,
    pub max: u16,
}

impl TargetBand {
    pub fn new(min: u16, max: u16) -> Self {
        assert!(min <= max);
        Self { min, max }
    }

    pub fn contains(self, value: u16) -> bool {
        value >= self.min && value <= self.max
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrategicGoalWeights {
    pub industry: u16,
    pub readiness: u16,
    pub politics: u16,
    pub research: u16,
}

impl StrategicGoalWeights {
    pub fn new(industry: u16, readiness: u16, politics: u16, research: u16) -> Self {
        assert!(industry + readiness + politics + research > 0);

        Self {
            industry,
            readiness,
            politics,
            research,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StrategicGoalWeights, TargetBand};

    #[test]
    fn target_band_contains_values_in_range() {
        let band = TargetBand::new(50, 60);

        assert!(band.contains(50));
        assert!(band.contains(55));
        assert!(band.contains(60));
        assert!(!band.contains(49));
        assert!(!band.contains(61));
    }

    #[test]
    fn strategic_goal_weights_require_a_non_zero_total() {
        let result = std::panic::catch_unwind(|| StrategicGoalWeights::new(0, 0, 0, 0));

        assert!(result.is_err());
    }
}
