use crate::domain::{
    CountryLaws, DivisionTemplate, EquipmentDemand, GameDate, Milestone, MilestoneKind,
    PivotWindow, TargetBand,
};
use crate::sim::{
    CountryRuntime, CountryState, ProductionLine, StateDefinition, StateId, StateRuntime,
};

use super::CountryScenario;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frontier {
    Germany,
    Belgium,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrontierFortRequirement {
    pub frontier: Frontier,
    pub target_level: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct France1936Scenario {
    pub reference_tag: &'static str,
    pub pivot_window: PivotWindow,
    pub milestones: [Milestone; 4],
    pub military_factory_target: u16,
    pub readiness_band: TargetBand,
    pub canonical_template: DivisionTemplate,
    pub frontier_forts: [FrontierFortRequirement; 2],
    pub economic_construction_order: Box<[StateId]>,
    pub infrastructure_order: Box<[StateId]>,
    pub military_construction_order: Box<[StateId]>,
    pub frontier_fort_order: Box<[StateId]>,
}

impl France1936Scenario {
    pub const ILE_DE_FRANCE: StateId = StateId(0);
    pub const NORD: StateId = StateId(1);
    pub const NORMANDY: StateId = StateId(2);
    pub const BRITTANY: StateId = StateId(3);
    pub const AQUITAINE: StateId = StateId(4);
    pub const OCCITANIA: StateId = StateId(5);
    pub const PROVENCE: StateId = StateId(6);
    pub const ALPS: StateId = StateId(7);
    pub const LORRAINE: StateId = StateId(8);
    pub const ALSACE: StateId = StateId(9);
    pub const CHAMPAGNE: StateId = StateId(10);
    pub const PICARDY: StateId = StateId(11);

    pub fn standard() -> Self {
        Self {
            reference_tag: "FRA",
            pivot_window: PivotWindow::new(GameDate::new(1938, 6, 1), GameDate::new(1939, 1, 1)),
            milestones: [
                Milestone::new(
                    "economic_checkpoint_1937",
                    GameDate::new(1937, 1, 1),
                    MilestoneKind::Economic,
                ),
                Milestone::new(
                    "economic_checkpoint_1938",
                    GameDate::new(1938, 1, 1),
                    MilestoneKind::Economic,
                ),
                Milestone::new(
                    "war_readiness_1939",
                    GameDate::new(1939, 9, 1),
                    MilestoneKind::Readiness,
                ),
                Milestone::new(
                    "fall_of_france_1940",
                    GameDate::new(1940, 5, 10),
                    MilestoneKind::Readiness,
                ),
            ],
            military_factory_target: 50,
            readiness_band: TargetBand::new(50, 60),
            canonical_template: DivisionTemplate::canonical_france_line(),
            frontier_forts: [
                FrontierFortRequirement {
                    frontier: Frontier::Germany,
                    target_level: 5,
                },
                FrontierFortRequirement {
                    frontier: Frontier::Belgium,
                    target_level: 5,
                },
            ],
            economic_construction_order: vec![
                Self::ILE_DE_FRANCE,
                Self::NORMANDY,
                Self::PROVENCE,
                Self::NORD,
                Self::AQUITAINE,
                Self::BRITTANY,
                Self::OCCITANIA,
                Self::CHAMPAGNE,
                Self::PICARDY,
                Self::ALPS,
                Self::LORRAINE,
                Self::ALSACE,
            ]
            .into_boxed_slice(),
            infrastructure_order: vec![
                Self::ILE_DE_FRANCE,
                Self::NORD,
                Self::LORRAINE,
                Self::ALSACE,
            ]
            .into_boxed_slice(),
            military_construction_order: vec![
                Self::LORRAINE,
                Self::ALSACE,
                Self::NORD,
                Self::PICARDY,
                Self::CHAMPAGNE,
                Self::PROVENCE,
                Self::NORMANDY,
                Self::ILE_DE_FRANCE,
                Self::AQUITAINE,
                Self::OCCITANIA,
                Self::BRITTANY,
                Self::ALPS,
            ]
            .into_boxed_slice(),
            frontier_fort_order: vec![Self::LORRAINE, Self::ALSACE, Self::NORD, Self::PICARDY]
                .into_boxed_slice(),
        }
    }

    pub fn bootstrap_runtime(&self) -> CountryRuntime {
        let state_defs = vec![
            StateDefinition {
                id: Self::ILE_DE_FRANCE,
                name: "ile_de_france",
                building_slots: 12,
                economic_weight: 12,
                infrastructure_target: 8,
                frontier: None,
            },
            StateDefinition {
                id: Self::NORD,
                name: "nord",
                building_slots: 9,
                economic_weight: 10,
                infrastructure_target: 7,
                frontier: Some(Frontier::Belgium),
            },
            StateDefinition {
                id: Self::NORMANDY,
                name: "normandy",
                building_slots: 8,
                economic_weight: 9,
                infrastructure_target: 7,
                frontier: None,
            },
            StateDefinition {
                id: Self::BRITTANY,
                name: "brittany",
                building_slots: 7,
                economic_weight: 7,
                infrastructure_target: 6,
                frontier: None,
            },
            StateDefinition {
                id: Self::AQUITAINE,
                name: "aquitaine",
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 6,
                frontier: None,
            },
            StateDefinition {
                id: Self::OCCITANIA,
                name: "occitania",
                building_slots: 8,
                economic_weight: 7,
                infrastructure_target: 6,
                frontier: None,
            },
            StateDefinition {
                id: Self::PROVENCE,
                name: "provence",
                building_slots: 8,
                economic_weight: 9,
                infrastructure_target: 7,
                frontier: None,
            },
            StateDefinition {
                id: Self::ALPS,
                name: "alps",
                building_slots: 6,
                economic_weight: 6,
                infrastructure_target: 6,
                frontier: None,
            },
            StateDefinition {
                id: Self::LORRAINE,
                name: "lorraine",
                building_slots: 9,
                economic_weight: 9,
                infrastructure_target: 7,
                frontier: Some(Frontier::Germany),
            },
            StateDefinition {
                id: Self::ALSACE,
                name: "alsace",
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 7,
                frontier: Some(Frontier::Germany),
            },
            StateDefinition {
                id: Self::CHAMPAGNE,
                name: "champagne",
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 6,
                frontier: None,
            },
            StateDefinition {
                id: Self::PICARDY,
                name: "picardy",
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 6,
                frontier: Some(Frontier::Belgium),
            },
        ]
        .into_boxed_slice();

        let states = vec![
            StateRuntime {
                civilian_factories: 8,
                military_factories: 2,
                infrastructure: 8,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 4,
                military_factories: 2,
                infrastructure: 7,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 4,
                military_factories: 1,
                infrastructure: 6,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 3,
                military_factories: 1,
                infrastructure: 5,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 3,
                military_factories: 1,
                infrastructure: 5,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 3,
                military_factories: 1,
                infrastructure: 5,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 4,
                military_factories: 2,
                infrastructure: 6,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 2,
                military_factories: 1,
                infrastructure: 5,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 3,
                military_factories: 2,
                infrastructure: 7,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 2,
                military_factories: 1,
                infrastructure: 7,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 3,
                military_factories: 1,
                infrastructure: 6,
                land_fort_level: 0,
            },
            StateRuntime {
                civilian_factories: 3,
                military_factories: 1,
                infrastructure: 6,
                land_fort_level: 0,
            },
        ]
        .into_boxed_slice();

        let production_lines = vec![
            ProductionLine::new(crate::domain::EquipmentKind::InfantryEquipment, 8),
            ProductionLine::new(crate::domain::EquipmentKind::SupportEquipment, 2),
            ProductionLine::new(crate::domain::EquipmentKind::Artillery, 2),
            ProductionLine::new(crate::domain::EquipmentKind::AntiTank, 1),
            ProductionLine::new(crate::domain::EquipmentKind::AntiAir, 1),
        ]
        .into_boxed_slice();

        CountryRuntime::new(
            CountryState::new(
                GameDate::new(1936, 1, 1),
                41_000_000,
                CountryLaws::default(),
            ),
            state_defs,
            states,
            production_lines,
        )
    }

    pub fn readiness_demand_for(&self, divisions: u16) -> EquipmentDemand {
        assert!(self.readiness_band.contains(divisions));
        self.canonical_template.demand_for(divisions)
    }
}

impl CountryScenario for France1936Scenario {
    fn reference_tag(&self) -> &'static str {
        self.reference_tag
    }

    fn start_date(&self) -> GameDate {
        GameDate::new(1936, 1, 1)
    }

    fn pivot_window(&self) -> PivotWindow {
        self.pivot_window
    }

    fn milestones(&self) -> &[Milestone] {
        &self.milestones
    }

    fn bootstrap_runtime(&self) -> CountryRuntime {
        France1936Scenario::bootstrap_runtime(self)
    }

    fn readiness_demand_for(&self, divisions: u16) -> EquipmentDemand {
        France1936Scenario::readiness_demand_for(self, divisions)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{MilestoneKind, TargetBand};
    use crate::scenario::CountryScenario;

    use super::{France1936Scenario, Frontier};

    #[test]
    fn france_scenario_exposes_approved_default_targets() {
        let scenario = France1936Scenario::standard();

        assert_eq!(scenario.reference_tag, "FRA");
        assert_eq!(scenario.military_factory_target, 50);
        assert_eq!(scenario.readiness_band, TargetBand::new(50, 60));
    }

    #[test]
    fn france_scenario_tracks_frontier_fort_targets_for_both_borders() {
        let scenario = France1936Scenario::standard();

        assert_eq!(scenario.frontier_forts[0].frontier, Frontier::Germany);
        assert_eq!(scenario.frontier_forts[1].frontier, Frontier::Belgium);
        assert_eq!(scenario.frontier_forts[0].target_level, 5);
        assert_eq!(scenario.frontier_forts[1].target_level, 5);
    }

    #[test]
    fn france_scenario_orders_economic_and_readiness_milestones() {
        let scenario = France1936Scenario::standard();

        assert_eq!(scenario.milestones[0].kind, MilestoneKind::Economic);
        assert_eq!(scenario.milestones[1].kind, MilestoneKind::Economic);
        assert_eq!(scenario.milestones[2].kind, MilestoneKind::Readiness);
        assert_eq!(scenario.milestones[3].kind, MilestoneKind::Readiness);
        assert!(scenario.milestones[0].date < scenario.milestones[1].date);
        assert!(scenario.milestones[1].date < scenario.milestones[2].date);
        assert!(scenario.milestones[2].date < scenario.milestones[3].date);
    }

    #[test]
    fn france_scenario_bootstraps_dense_state_runtime() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();

        assert_eq!(runtime.state_defs.len(), 12);
        assert_eq!(runtime.total_civilian_factories(), 42);
        assert_eq!(runtime.total_military_factories(), 16);
    }

    #[test]
    fn france_scenario_computes_readiness_demand_for_valid_band_counts() {
        let scenario = France1936Scenario::standard();
        let demand = scenario.readiness_demand_for(50);

        assert_eq!(demand.infantry_equipment, 400_000);
        assert_eq!(demand.support_equipment, 3_000);
        assert_eq!(demand.manpower, 530_000);
    }

    #[test]
    fn france_scenario_rejects_division_counts_outside_the_target_band() {
        let scenario = France1936Scenario::standard();
        let result = std::panic::catch_unwind(|| scenario.readiness_demand_for(49));

        assert!(result.is_err());
    }

    #[test]
    fn france_scenario_implements_the_country_scenario_trait() {
        let scenario = France1936Scenario::standard();
        let trait_view = &scenario as &dyn CountryScenario;

        assert_eq!(trait_view.reference_tag(), "FRA");
        assert_eq!(
            trait_view.start_date(),
            crate::domain::GameDate::new(1936, 1, 1)
        );
    }
}
