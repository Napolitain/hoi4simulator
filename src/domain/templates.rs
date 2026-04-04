#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentKind {
    InfantryEquipment,
    SupportEquipment,
    Artillery,
    AntiTank,
    AntiAir,
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

impl EquipmentDemand {
    pub fn get(self, equipment: EquipmentKind) -> u32 {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{DivisionTemplate, EquipmentDemand, TemplateDesignConstraints};

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
