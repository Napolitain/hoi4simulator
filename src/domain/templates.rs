use fory::ForyObject;

use super::ResourceLedger;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub enum EquipmentKind {
    InfantryEquipment,
    SupportEquipment,
    Artillery,
    AntiTank,
    AntiAir,
    Unmodeled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EquipmentDemand {
    pub infantry_equipment: u32,
    pub support_equipment: u32,
    pub artillery: u32,
    pub anti_tank: u32,
    pub anti_air: u32,
    pub manpower: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquipmentReserveRatios {
    pub infantry_equipment_bp: u16,
    pub support_equipment_bp: u16,
    pub artillery_bp: u16,
    pub anti_tank_bp: u16,
    pub anti_air_bp: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TemplateDesignConstraints {
    pub manpower_limit: u32,
    pub infantry_equipment_limit: u32,
    pub support_equipment_limit: u32,
    pub artillery_limit: u32,
    pub anti_tank_limit: u32,
    pub anti_air_limit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateFitness {
    pub total_ic_cost_centi: u32,
    pub manpower: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub struct EquipmentProfile {
    pub unit_cost_centi: u32,
    pub resources: ResourceLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub struct ModeledEquipmentProfiles {
    pub infantry_equipment: EquipmentProfile,
    pub support_equipment: EquipmentProfile,
    pub artillery: EquipmentProfile,
    pub anti_tank: EquipmentProfile,
    pub anti_air: EquipmentProfile,
}

impl EquipmentDemand {
    pub fn get(self, equipment: EquipmentKind) -> u32 {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
            EquipmentKind::Unmodeled => 0,
        }
    }

    pub fn scale(self, divisions: u16) -> Self {
        let multiplier = u32::from(divisions);

        Self {
            infantry_equipment: self.infantry_equipment * multiplier,
            support_equipment: self.support_equipment * multiplier,
            artillery: self.artillery * multiplier,
            anti_tank: self.anti_tank * multiplier,
            anti_air: self.anti_air * multiplier,
            manpower: self.manpower * multiplier,
        }
    }

    pub fn plus(self, other: Self) -> Self {
        Self {
            infantry_equipment: self
                .infantry_equipment
                .saturating_add(other.infantry_equipment),
            support_equipment: self
                .support_equipment
                .saturating_add(other.support_equipment),
            artillery: self.artillery.saturating_add(other.artillery),
            anti_tank: self.anti_tank.saturating_add(other.anti_tank),
            anti_air: self.anti_air.saturating_add(other.anti_air),
            manpower: self.manpower.saturating_add(other.manpower),
        }
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            infantry_equipment: self
                .infantry_equipment
                .saturating_sub(other.infantry_equipment),
            support_equipment: self
                .support_equipment
                .saturating_sub(other.support_equipment),
            artillery: self.artillery.saturating_sub(other.artillery),
            anti_tank: self.anti_tank.saturating_sub(other.anti_tank),
            anti_air: self.anti_air.saturating_sub(other.anti_air),
            manpower: self.manpower.saturating_sub(other.manpower),
        }
    }

    pub fn scale_basis_points(self, basis_points: u16) -> Self {
        let scale = |value: u32| {
            value
                .saturating_mul(u32::from(basis_points))
                .div_ceil(10_000)
        };

        Self {
            infantry_equipment: scale(self.infantry_equipment),
            support_equipment: scale(self.support_equipment),
            artillery: scale(self.artillery),
            anti_tank: scale(self.anti_tank),
            anti_air: scale(self.anti_air),
            manpower: scale(self.manpower),
        }
    }

    pub fn reserve_buffer(self, reserve_ratios: EquipmentReserveRatios) -> Self {
        Self {
            infantry_equipment: self
                .infantry_equipment
                .saturating_mul(u32::from(reserve_ratios.infantry_equipment_bp))
                .div_ceil(10_000),
            support_equipment: self
                .support_equipment
                .saturating_mul(u32::from(reserve_ratios.support_equipment_bp))
                .div_ceil(10_000),
            artillery: self
                .artillery
                .saturating_mul(u32::from(reserve_ratios.artillery_bp))
                .div_ceil(10_000),
            anti_tank: self
                .anti_tank
                .saturating_mul(u32::from(reserve_ratios.anti_tank_bp))
                .div_ceil(10_000),
            anti_air: self
                .anti_air
                .saturating_mul(u32::from(reserve_ratios.anti_air_bp))
                .div_ceil(10_000),
            manpower: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportCompanies {
    pub logistics: bool,
    pub field_hospital: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivisionTemplate {
    pub name: &'static str,
    pub infantry_battalions: u8,
    pub artillery_battalions: u8,
    pub anti_tank_battalions: u8,
    pub anti_air_battalions: u8,
    pub support: SupportCompanies,
}

impl DivisionTemplate {
    pub fn canonical_france_line() -> Self {
        Self {
            name: "france_line_infantry",
            infantry_battalions: 8,
            artillery_battalions: 2,
            anti_tank_battalions: 1,
            anti_air_battalions: 1,
            support: SupportCompanies {
                logistics: true,
                field_hospital: true,
            },
        }
    }

    pub fn france_line_candidates() -> [Self; 5] {
        [
            Self::canonical_france_line(),
            Self {
                name: "france_economy_line",
                infantry_battalions: 9,
                artillery_battalions: 1,
                anti_tank_battalions: 0,
                anti_air_battalions: 1,
                support: SupportCompanies {
                    logistics: true,
                    field_hospital: false,
                },
            },
            Self {
                name: "france_defensive_line",
                infantry_battalions: 9,
                artillery_battalions: 1,
                anti_tank_battalions: 1,
                anti_air_battalions: 0,
                support: SupportCompanies {
                    logistics: true,
                    field_hospital: true,
                },
            },
            Self {
                name: "france_artillery_line",
                infantry_battalions: 8,
                artillery_battalions: 2,
                anti_tank_battalions: 0,
                anti_air_battalions: 0,
                support: SupportCompanies {
                    logistics: true,
                    field_hospital: true,
                },
            },
            Self {
                name: "france_mass_line",
                infantry_battalions: 10,
                artillery_battalions: 0,
                anti_tank_battalions: 0,
                anti_air_battalions: 0,
                support: SupportCompanies {
                    logistics: false,
                    field_hospital: true,
                },
            },
        ]
    }

    pub fn demand_for(self, divisions: u16) -> EquipmentDemand {
        self.per_division_demand().scale(divisions)
    }

    pub fn per_division_demand(self) -> EquipmentDemand {
        let mut demand = EquipmentDemand {
            infantry_equipment: u32::from(self.infantry_battalions) * 1_000,
            support_equipment: 0,
            artillery: u32::from(self.artillery_battalions) * 36,
            anti_tank: u32::from(self.anti_tank_battalions) * 36,
            anti_air: u32::from(self.anti_air_battalions) * 36,
            manpower: u32::from(self.infantry_battalions) * 1_000
                + u32::from(self.artillery_battalions) * 500
                + u32::from(self.anti_tank_battalions) * 500
                + u32::from(self.anti_air_battalions) * 500,
        };

        if self.support.logistics {
            demand.support_equipment += 30;
            demand.manpower += 300;
        }

        if self.support.field_hospital {
            demand.support_equipment += 30;
            demand.manpower += 300;
        }

        demand
    }

    pub fn estimated_ic_cost_centi(self) -> u32 {
        let demand = self.per_division_demand();

        demand.infantry_equipment * 50
            + demand.support_equipment * 400
            + demand.artillery * 350
            + demand.anti_tank * 400
            + demand.anti_air * 350
    }

    pub fn fitness(self) -> TemplateFitness {
        let demand = self.per_division_demand();

        TemplateFitness {
            total_ic_cost_centi: self.estimated_ic_cost_centi(),
            manpower: demand.manpower,
        }
    }

    pub fn fits(self, constraints: TemplateDesignConstraints) -> bool {
        let demand = self.per_division_demand();

        if constraints.manpower_limit > 0 && demand.manpower > constraints.manpower_limit {
            return false;
        }
        if constraints.infantry_equipment_limit > 0
            && demand.infantry_equipment > constraints.infantry_equipment_limit
        {
            return false;
        }
        if constraints.support_equipment_limit > 0
            && demand.support_equipment > constraints.support_equipment_limit
        {
            return false;
        }
        if constraints.artillery_limit > 0 && demand.artillery > constraints.artillery_limit {
            return false;
        }
        if constraints.anti_tank_limit > 0 && demand.anti_tank > constraints.anti_tank_limit {
            return false;
        }
        if constraints.anti_air_limit > 0 && demand.anti_air > constraints.anti_air_limit {
            return false;
        }

        true
    }
}

impl EquipmentKind {
    pub const fn default_unit_cost_centi(self) -> u32 {
        match self {
            Self::InfantryEquipment => 50,
            Self::SupportEquipment => 400,
            Self::Artillery => 350,
            Self::AntiTank => 400,
            Self::AntiAir => 350,
            Self::Unmodeled => 1_000,
        }
    }
}

impl EquipmentReserveRatios {
    pub const fn france_default() -> Self {
        Self {
            infantry_equipment_bp: 3_000,
            support_equipment_bp: 2_000,
            artillery_bp: 2_000,
            anti_tank_bp: 1_500,
            anti_air_bp: 1_500,
        }
    }
}

impl EquipmentProfile {
    pub const fn new(unit_cost_centi: u32, resources: ResourceLedger) -> Self {
        Self {
            unit_cost_centi,
            resources,
        }
    }
}

impl ModeledEquipmentProfiles {
    pub fn default_1936() -> Self {
        Self {
            infantry_equipment: EquipmentProfile::new(
                50,
                ResourceLedger {
                    steel: 2,
                    ..ResourceLedger::default()
                },
            ),
            support_equipment: EquipmentProfile::new(
                400,
                ResourceLedger {
                    steel: 2,
                    aluminium: 1,
                    ..ResourceLedger::default()
                },
            ),
            artillery: EquipmentProfile::new(
                350,
                ResourceLedger {
                    steel: 2,
                    tungsten: 1,
                    ..ResourceLedger::default()
                },
            ),
            anti_tank: EquipmentProfile::new(
                400,
                ResourceLedger {
                    steel: 2,
                    tungsten: 2,
                    ..ResourceLedger::default()
                },
            ),
            anti_air: EquipmentProfile::new(
                400,
                ResourceLedger {
                    steel: 2,
                    ..ResourceLedger::default()
                },
            ),
        }
    }

    pub fn profile(self, equipment: EquipmentKind) -> EquipmentProfile {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
            EquipmentKind::Unmodeled => EquipmentProfile::new(
                EquipmentKind::Unmodeled.default_unit_cost_centi(),
                ResourceLedger::default(),
            ),
        }
    }

    pub fn set(&mut self, equipment: EquipmentKind, profile: EquipmentProfile) {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment = profile,
            EquipmentKind::SupportEquipment => self.support_equipment = profile,
            EquipmentKind::Artillery => self.artillery = profile,
            EquipmentKind::AntiTank => self.anti_tank = profile,
            EquipmentKind::AntiAir => self.anti_air = profile,
            EquipmentKind::Unmodeled => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::domain::ResourceLedger;

    use super::{
        DivisionTemplate, EquipmentDemand, EquipmentKind, EquipmentReserveRatios,
        ModeledEquipmentProfiles, TemplateDesignConstraints,
    };

    #[test]
    fn france_line_template_generates_expected_per_division_demand() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.per_division_demand();

        assert_eq!(
            demand,
            EquipmentDemand {
                infantry_equipment: 8_000,
                support_equipment: 60,
                artillery: 72,
                anti_tank: 36,
                anti_air: 36,
                manpower: 10_600,
            }
        );
    }

    #[test]
    fn template_demand_scales_linearly() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.demand_for(50);

        assert_eq!(demand.infantry_equipment, 400_000);
        assert_eq!(demand.support_equipment, 3_000);
        assert_eq!(demand.artillery, 3_600);
        assert_eq!(demand.anti_tank, 1_800);
        assert_eq!(demand.anti_air, 1_800);
        assert_eq!(demand.manpower, 530_000);
    }

    #[test]
    fn template_fitness_exposes_ic_and_manpower_cost() {
        let template = DivisionTemplate::canonical_france_line();
        let fitness = template.fitness();

        assert!(fitness.total_ic_cost_centi > 0);
        assert_eq!(fitness.manpower, 10_600);
    }

    #[test]
    fn template_constraints_can_reject_expensive_designs() {
        let template = DivisionTemplate::canonical_france_line();
        let allowed = template.fits(TemplateDesignConstraints {
            manpower_limit: 11_000,
            infantry_equipment_limit: 8_500,
            support_equipment_limit: 100,
            artillery_limit: 100,
            anti_tank_limit: 50,
            anti_air_limit: 50,
        });
        let rejected = template.fits(TemplateDesignConstraints {
            manpower_limit: 10_000,
            infantry_equipment_limit: 8_500,
            support_equipment_limit: 100,
            artillery_limit: 100,
            anti_tank_limit: 50,
            anti_air_limit: 50,
        });

        assert!(allowed);
        assert!(!rejected);
    }

    #[test]
    fn equipment_demand_can_add_reserves_without_creating_manpower() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.per_division_demand();
        let reserve = demand.reserve_buffer(EquipmentReserveRatios::france_default());

        assert!(reserve.infantry_equipment > 0);
        assert!(reserve.artillery > 0);
        assert_eq!(reserve.manpower, 0);
        assert_eq!(demand.plus(reserve).manpower, demand.manpower);
    }

    #[test]
    fn france_candidate_library_offers_multiple_shapes() {
        let candidates = DivisionTemplate::france_line_candidates();

        assert_eq!(candidates.len(), 5);
        assert!(
            candidates
                .iter()
                .any(|template| template.name == "france_economy_line")
        );
        assert!(
            candidates
                .iter()
                .any(|template| template.name == "france_mass_line")
        );
    }

    #[test]
    fn default_1936_profiles_match_modeled_equipment_kinds() {
        let profiles = ModeledEquipmentProfiles::default_1936();

        assert_eq!(
            profiles
                .profile(EquipmentKind::InfantryEquipment)
                .unit_cost_centi,
            50
        );
        assert_eq!(
            profiles.profile(EquipmentKind::SupportEquipment).resources,
            ResourceLedger {
                steel: 2,
                aluminium: 1,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn equipment_demand_can_subtract_a_fielded_baseline() {
        let total = EquipmentDemand {
            infantry_equipment: 10_000,
            support_equipment: 600,
            artillery: 300,
            anti_tank: 120,
            anti_air: 90,
            manpower: 12_000,
        };
        let fielded = EquipmentDemand {
            infantry_equipment: 8_000,
            support_equipment: 400,
            artillery: 200,
            anti_tank: 120,
            anti_air: 120,
            manpower: 10_000,
        };

        assert_eq!(
            total.saturating_sub(fielded),
            EquipmentDemand {
                infantry_equipment: 2_000,
                support_equipment: 200,
                artillery: 100,
                anti_tank: 0,
                anti_air: 0,
                manpower: 2_000,
            }
        );
    }

    proptest! {
        #[test]
        fn demand_for_matches_scaled_per_division_demand(divisions in 0u16..128) {
            let template = DivisionTemplate::canonical_france_line();

            prop_assert_eq!(
                template.demand_for(divisions),
                template.per_division_demand().scale(divisions),
            );
        }
    }
}
