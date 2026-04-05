use super::ResourceLedger;
use super::calendar::GameDate;
use super::laws::MobilizationLaw;
use super::templates::{DivisionTemplate, EquipmentDemand, EquipmentKind, EquipmentReserveRatios};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EquipmentFactoryAllocation {
    pub infantry_equipment: u16,
    pub support_equipment: u16,
    pub artillery: u16,
    pub anti_tank: u16,
    pub anti_air: u16,
}

impl EquipmentFactoryAllocation {
    pub fn get(self, equipment: EquipmentKind) -> u16 {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
            EquipmentKind::Unmodeled => 0,
        }
    }

    pub fn set(&mut self, equipment: EquipmentKind, factories: u16) {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment = factories,
            EquipmentKind::SupportEquipment => self.support_equipment = factories,
            EquipmentKind::Artillery => self.artillery = factories,
            EquipmentKind::AntiTank => self.anti_tank = factories,
            EquipmentKind::AntiAir => self.anti_air = factories,
            EquipmentKind::Unmodeled => {}
        }
    }

    pub fn total(self) -> u16 {
        self.infantry_equipment
            .saturating_add(self.support_equipment)
            .saturating_add(self.artillery)
            .saturating_add(self.anti_tank)
            .saturating_add(self.anti_air)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForceGoalSpec {
    pub target_date: GameDate,
    pub army_band: TargetBand,
    pub divisions_per_army: u8,
    pub reserve_ratios: EquipmentReserveRatios,
    pub manpower_reserve_bp: u16,
    pub acceptable_stockpile_shortfall_bp: u16,
    pub target_mobilization_law: MobilizationLaw,
}

impl ForceGoalSpec {
    pub fn france_1939_default() -> Self {
        Self {
            target_date: GameDate::new(1939, 9, 1),
            army_band: TargetBand::new(3, 4),
            divisions_per_army: 24,
            reserve_ratios: EquipmentReserveRatios::france_default(),
            manpower_reserve_bp: 1_500,
            acceptable_stockpile_shortfall_bp: 500,
            target_mobilization_law: MobilizationLaw::ExtensiveConscription,
        }
    }

    pub fn division_band(self) -> TargetBand {
        let min = u32::from(self.army_band.min) * u32::from(self.divisions_per_army);
        let max = u32::from(self.army_band.max) * u32::from(self.divisions_per_army);

        TargetBand::new(
            u16::try_from(min).unwrap_or(u16::MAX),
            u16::try_from(max).unwrap_or(u16::MAX),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForcePlan {
    pub template: DivisionTemplate,
    pub frontline_divisions: u16,
    pub frontline_demand: EquipmentDemand,
    pub starting_fielded_equipped_demand: EquipmentDemand,
    pub reserve_demand: EquipmentDemand,
    pub stockpile_target_demand: EquipmentDemand,
    pub total_demand: EquipmentDemand,
    pub required_military_factories: u16,
    pub factory_allocation: EquipmentFactoryAllocation,
    pub daily_resource_use: ResourceLedger,
    pub resource_utilization_bp: u16,
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
    use crate::domain::MobilizationLaw;

    use super::{EquipmentFactoryAllocation, ForceGoalSpec, StrategicGoalWeights, TargetBand};

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

    #[test]
    fn france_force_goal_expands_armies_into_divisions() {
        let goal = ForceGoalSpec::france_1939_default();

        assert_eq!(goal.division_band(), TargetBand::new(72, 96));
        assert_eq!(
            goal.target_mobilization_law,
            MobilizationLaw::ExtensiveConscription
        );
    }

    #[test]
    fn factory_allocation_tracks_totals_by_equipment_kind() {
        let mut allocation = EquipmentFactoryAllocation::default();
        allocation.set(crate::domain::EquipmentKind::Artillery, 6);
        allocation.set(crate::domain::EquipmentKind::InfantryEquipment, 18);

        assert_eq!(allocation.total(), 24);
        assert_eq!(allocation.artillery, 6);
    }
}
