use fory::ForyObject;

use super::ResourceLedger;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub enum EquipmentKind {
    InfantryEquipment,
    SupportEquipment,
    Artillery,
    AntiTank,
    AntiAir,
    MotorizedEquipment,
    Armor,
    Fighter,
    Bomber,
    Unmodeled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EquipmentDemand {
    pub infantry_equipment: u32,
    pub support_equipment: u32,
    pub artillery: u32,
    pub anti_tank: u32,
    pub anti_air: u32,
    pub motorized_equipment: u32,
    pub armor: u32,
    pub fighters: u32,
    pub bombers: u32,
    pub manpower: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FieldedDivision {
    pub target_demand: EquipmentDemand,
    pub equipped_demand: EquipmentDemand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquipmentReserveRatios {
    pub infantry_equipment_bp: u16,
    pub support_equipment_bp: u16,
    pub artillery_bp: u16,
    pub anti_tank_bp: u16,
    pub anti_air_bp: u16,
    pub motorized_equipment_bp: u16,
    pub armor_bp: u16,
    pub fighters_bp: u16,
    pub bombers_bp: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TemplateDesignConstraints {
    pub manpower_limit: u32,
    pub infantry_equipment_limit: u32,
    pub support_equipment_limit: u32,
    pub artillery_limit: u32,
    pub anti_tank_limit: u32,
    pub anti_air_limit: u32,
    pub motorized_equipment_limit: u32,
    pub armor_limit: u32,
    pub fighters_limit: u32,
    pub bombers_limit: u32,
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
    pub motorized_equipment: EquipmentProfile,
    pub armor: EquipmentProfile,
    pub fighter: EquipmentProfile,
    pub bomber: EquipmentProfile,
}

impl EquipmentDemand {
    pub fn get(self, equipment: EquipmentKind) -> u32 {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
            EquipmentKind::MotorizedEquipment => self.motorized_equipment,
            EquipmentKind::Armor => self.armor,
            EquipmentKind::Fighter => self.fighters,
            EquipmentKind::Bomber => self.bombers,
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
            motorized_equipment: self.motorized_equipment * multiplier,
            armor: self.armor * multiplier,
            fighters: self.fighters * multiplier,
            bombers: self.bombers * multiplier,
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
            motorized_equipment: self
                .motorized_equipment
                .saturating_add(other.motorized_equipment),
            armor: self.armor.saturating_add(other.armor),
            fighters: self.fighters.saturating_add(other.fighters),
            bombers: self.bombers.saturating_add(other.bombers),
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
            motorized_equipment: self
                .motorized_equipment
                .saturating_sub(other.motorized_equipment),
            armor: self.armor.saturating_sub(other.armor),
            fighters: self.fighters.saturating_sub(other.fighters),
            bombers: self.bombers.saturating_sub(other.bombers),
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
            motorized_equipment: scale(self.motorized_equipment),
            armor: scale(self.armor),
            fighters: scale(self.fighters),
            bombers: scale(self.bombers),
            manpower: scale(self.manpower),
        }
    }

    pub fn scale_equipment_basis_points(self, basis_points: u16) -> Self {
        let mut scaled = self.scale_basis_points(basis_points);
        scaled.manpower = self.manpower;
        scaled
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
            motorized_equipment: self
                .motorized_equipment
                .saturating_mul(u32::from(reserve_ratios.motorized_equipment_bp))
                .div_ceil(10_000),
            armor: self
                .armor
                .saturating_mul(u32::from(reserve_ratios.armor_bp))
                .div_ceil(10_000),
            fighters: self
                .fighters
                .saturating_mul(u32::from(reserve_ratios.fighters_bp))
                .div_ceil(10_000),
            bombers: self
                .bombers
                .saturating_mul(u32::from(reserve_ratios.bombers_bp))
                .div_ceil(10_000),
            manpower: 0,
        }
    }

    pub const fn has_equipment(self) -> bool {
        self.infantry_equipment > 0
            || self.support_equipment > 0
            || self.artillery > 0
            || self.anti_tank > 0
            || self.anti_air > 0
            || self.motorized_equipment > 0
            || self.armor > 0
            || self.fighters > 0
            || self.bombers > 0
    }

    pub fn without_manpower(self) -> Self {
        Self {
            manpower: 0,
            ..self
        }
    }
}

impl FieldedDivision {
    pub fn new(target_demand: EquipmentDemand, equipped_demand: EquipmentDemand) -> Self {
        assert!(equipped_demand.infantry_equipment <= target_demand.infantry_equipment);
        assert!(equipped_demand.support_equipment <= target_demand.support_equipment);
        assert!(equipped_demand.artillery <= target_demand.artillery);
        assert!(equipped_demand.anti_tank <= target_demand.anti_tank);
        assert!(equipped_demand.anti_air <= target_demand.anti_air);
        assert!(equipped_demand.motorized_equipment <= target_demand.motorized_equipment);
        assert!(equipped_demand.armor <= target_demand.armor);
        assert!(equipped_demand.fighters <= target_demand.fighters);
        assert!(equipped_demand.bombers <= target_demand.bombers);
        assert_eq!(equipped_demand.manpower, target_demand.manpower);

        Self {
            target_demand,
            equipped_demand,
        }
    }

    pub fn reinforcement_gap(self) -> EquipmentDemand {
        self.target_demand.saturating_sub(self.equipped_demand)
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
            motorized_equipment: 0,
            armor: 0,
            fighters: 0,
            bombers: 0,
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
        if constraints.motorized_equipment_limit > 0
            && demand.motorized_equipment > constraints.motorized_equipment_limit
        {
            return false;
        }
        if constraints.armor_limit > 0 && demand.armor > constraints.armor_limit {
            return false;
        }
        if constraints.fighters_limit > 0 && demand.fighters > constraints.fighters_limit {
            return false;
        }
        if constraints.bombers_limit > 0 && demand.bombers > constraints.bombers_limit {
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
            Self::MotorizedEquipment => 250,
            Self::Armor => 1_200,
            Self::Fighter => 2_200,
            Self::Bomber => 2_800,
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
            motorized_equipment_bp: 1_500,
            armor_bp: 1_000,
            fighters_bp: 0,
            bombers_bp: 0,
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
            motorized_equipment: EquipmentProfile::new(
                250,
                ResourceLedger {
                    steel: 2,
                    rubber: 1,
                    ..ResourceLedger::default()
                },
            ),
            armor: EquipmentProfile::new(
                1_200,
                ResourceLedger {
                    steel: 2,
                    oil: 1,
                    ..ResourceLedger::default()
                },
            ),
            fighter: EquipmentProfile::new(
                2_200,
                ResourceLedger {
                    aluminium: 2,
                    oil: 1,
                    rubber: 1,
                    ..ResourceLedger::default()
                },
            ),
            bomber: EquipmentProfile::new(
                2_800,
                ResourceLedger {
                    aluminium: 2,
                    oil: 1,
                    rubber: 1,
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
            EquipmentKind::MotorizedEquipment => self.motorized_equipment,
            EquipmentKind::Armor => self.armor,
            EquipmentKind::Fighter => self.fighter,
            EquipmentKind::Bomber => self.bomber,
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
            EquipmentKind::MotorizedEquipment => self.motorized_equipment = profile,
            EquipmentKind::Armor => self.armor = profile,
            EquipmentKind::Fighter => self.fighter = profile,
            EquipmentKind::Bomber => self.bomber = profile,
            EquipmentKind::Unmodeled => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::domain::ResourceLedger;

    use super::{
        DivisionTemplate, EquipmentDemand, EquipmentKind, EquipmentReserveRatios, FieldedDivision,
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
                ..EquipmentDemand::default()
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
            ..TemplateDesignConstraints::default()
        });
        let rejected = template.fits(TemplateDesignConstraints {
            manpower_limit: 10_000,
            infantry_equipment_limit: 8_500,
            support_equipment_limit: 100,
            artillery_limit: 100,
            anti_tank_limit: 50,
            anti_air_limit: 50,
            ..TemplateDesignConstraints::default()
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
            ..EquipmentDemand::default()
        };
        let fielded = EquipmentDemand {
            infantry_equipment: 8_000,
            support_equipment: 400,
            artillery: 200,
            anti_tank: 120,
            anti_air: 120,
            manpower: 10_000,
            ..EquipmentDemand::default()
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
                ..EquipmentDemand::default()
            }
        );
    }

    #[test]
    fn equipment_only_scaling_preserves_fielded_manpower() {
        let demand = EquipmentDemand {
            infantry_equipment: 8_000,
            support_equipment: 60,
            artillery: 36,
            anti_tank: 0,
            anti_air: 0,
            manpower: 9_800,
            ..EquipmentDemand::default()
        };

        assert_eq!(
            demand.scale_equipment_basis_points(5_000),
            EquipmentDemand {
                infantry_equipment: 4_000,
                support_equipment: 30,
                artillery: 18,
                anti_tank: 0,
                anti_air: 0,
                manpower: 9_800,
                ..EquipmentDemand::default()
            }
        );
    }

    #[test]
    fn fielded_division_tracks_reinforcement_gap_without_double_counting_manpower() {
        let target = EquipmentDemand {
            infantry_equipment: 8_000,
            support_equipment: 60,
            artillery: 36,
            anti_tank: 0,
            anti_air: 0,
            manpower: 9_800,
            ..EquipmentDemand::default()
        };
        let equipped = target.scale_equipment_basis_points(5_000);
        let division = FieldedDivision::new(target, equipped);

        assert_eq!(
            division.reinforcement_gap(),
            EquipmentDemand {
                infantry_equipment: 4_000,
                support_equipment: 30,
                artillery: 18,
                anti_tank: 0,
                anti_air: 0,
                manpower: 0,
                ..EquipmentDemand::default()
            }
        );
    }

    proptest! {
        #[test]
        fn extended_equipment_scaling_preserves_manpower_and_never_increases(
            motorized in 0u32..20_000,
            armor in 0u32..5_000,
            fighters in 0u32..5_000,
            bombers in 0u32..5_000,
            manpower in 1u32..100_000,
            bp in 0u16..10_001,
        ) {
            let demand = EquipmentDemand {
                motorized_equipment: motorized,
                armor,
                fighters,
                bombers,
                manpower,
                ..EquipmentDemand::default()
            };
            let scaled = demand.scale_equipment_basis_points(bp);

            prop_assert_eq!(scaled.manpower, demand.manpower);
            prop_assert!(scaled.motorized_equipment <= demand.motorized_equipment);
            prop_assert!(scaled.armor <= demand.armor);
            prop_assert!(scaled.fighters <= demand.fighters);
            prop_assert!(scaled.bombers <= demand.bombers);
        }

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
