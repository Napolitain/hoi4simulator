use fory::ForyObject;

use crate::data::{DataError, StructuredFrance1936Dataset};
use crate::domain::{
    CountryLaws, DivisionTemplate, EquipmentDemand, EquipmentFactoryAllocation, EquipmentKind,
    ForceGoalSpec, ForcePlan, GameDate, Milestone, MilestoneKind, ModeledEquipmentProfiles,
    PivotWindow, ResourceLedger,
};
use crate::sim::{
    CountryRuntime, CountryState, ProductionLine, SimulationConfig, StateDefinition, StateId,
    StateRuntime,
};

use super::CountryScenario;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
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
    pub start_date: GameDate,
    pub pivot_window: PivotWindow,
    pub milestones: [Milestone; 4],
    pub force_goal: ForceGoalSpec,
    pub force_plan: ForcePlan,
    pub equipment_profiles: ModeledEquipmentProfiles,
    pub domestic_resources: ResourceLedger,
    pub starting_fielded_divisions: u16,
    pub frontier_forts: [FrontierFortRequirement; 2],
    pub economic_construction_order: Box<[StateId]>,
    pub infrastructure_order: Box<[StateId]>,
    pub military_construction_order: Box<[StateId]>,
    pub frontier_fort_order: Box<[StateId]>,
    pub initial_country: CountryState,
    pub initial_state_defs: Box<[StateDefinition]>,
    pub initial_states: Box<[StateRuntime]>,
    pub initial_production_lines: Box<[ProductionLine]>,
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
        let start_date = GameDate::new(1936, 1, 1);
        let equipment_profiles = ModeledEquipmentProfiles::default_1936();
        let force_goal = ForceGoalSpec::france_1939_default();
        let starting_fielded_divisions = force_goal.division_band().min;
        let initial_state_defs = vec![
            StateDefinition {
                id: Self::ILE_DE_FRANCE,
                raw_state_id: 0,
                name: "ile_de_france".into(),
                building_slots: 12,
                economic_weight: 12,
                infrastructure_target: 8,
                frontier: None,
                resources: ResourceLedger {
                    steel: 4,
                    aluminium: 2,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::NORD,
                raw_state_id: 1,
                name: "nord".into(),
                building_slots: 9,
                economic_weight: 10,
                infrastructure_target: 7,
                frontier: Some(Frontier::Belgium),
                resources: ResourceLedger {
                    steel: 8,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::NORMANDY,
                raw_state_id: 2,
                name: "normandy".into(),
                building_slots: 8,
                economic_weight: 9,
                infrastructure_target: 7,
                frontier: None,
                resources: ResourceLedger {
                    steel: 2,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::BRITTANY,
                raw_state_id: 3,
                name: "brittany".into(),
                building_slots: 7,
                economic_weight: 7,
                infrastructure_target: 6,
                frontier: None,
                resources: ResourceLedger {
                    steel: 1,
                    tungsten: 1,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::AQUITAINE,
                raw_state_id: 4,
                name: "aquitaine".into(),
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 6,
                frontier: None,
                resources: ResourceLedger {
                    steel: 3,
                    oil: 1,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::OCCITANIA,
                raw_state_id: 5,
                name: "occitania".into(),
                building_slots: 8,
                economic_weight: 7,
                infrastructure_target: 6,
                frontier: None,
                resources: ResourceLedger {
                    tungsten: 3,
                    steel: 2,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::PROVENCE,
                raw_state_id: 6,
                name: "provence".into(),
                building_slots: 8,
                economic_weight: 9,
                infrastructure_target: 7,
                frontier: None,
                resources: ResourceLedger {
                    aluminium: 5,
                    steel: 3,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::ALPS,
                raw_state_id: 7,
                name: "alps".into(),
                building_slots: 6,
                economic_weight: 6,
                infrastructure_target: 6,
                frontier: None,
                resources: ResourceLedger {
                    tungsten: 4,
                    steel: 1,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::LORRAINE,
                raw_state_id: 8,
                name: "lorraine".into(),
                building_slots: 9,
                economic_weight: 9,
                infrastructure_target: 7,
                frontier: Some(Frontier::Germany),
                resources: ResourceLedger {
                    steel: 16,
                    tungsten: 2,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::ALSACE,
                raw_state_id: 9,
                name: "alsace".into(),
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 7,
                frontier: Some(Frontier::Germany),
                resources: ResourceLedger {
                    steel: 10,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::CHAMPAGNE,
                raw_state_id: 10,
                name: "champagne".into(),
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 6,
                frontier: None,
                resources: ResourceLedger {
                    steel: 4,
                    ..ResourceLedger::default()
                },
            },
            StateDefinition {
                id: Self::PICARDY,
                raw_state_id: 11,
                name: "picardy".into(),
                building_slots: 8,
                economic_weight: 8,
                infrastructure_target: 6,
                frontier: Some(Frontier::Belgium),
                resources: ResourceLedger {
                    steel: 4,
                    ..ResourceLedger::default()
                },
            },
        ]
        .into_boxed_slice();
        let initial_states = vec![
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
        let initial_production_lines = vec![
            ProductionLine::new(crate::domain::EquipmentKind::InfantryEquipment, 8),
            ProductionLine::new(crate::domain::EquipmentKind::SupportEquipment, 2),
            ProductionLine::new(crate::domain::EquipmentKind::Artillery, 2),
            ProductionLine::new(crate::domain::EquipmentKind::AntiTank, 1),
            ProductionLine::new(crate::domain::EquipmentKind::AntiAir, 1),
        ]
        .into_boxed_slice();
        let domestic_resources = aggregate_domestic_resources(&initial_state_defs);
        let force_plan = Self::derive_force_plan(
            start_date,
            41_000_000,
            domestic_resources,
            force_goal,
            equipment_profiles,
            starting_fielded_divisions,
        );

        Self {
            reference_tag: "FRA",
            start_date,
            pivot_window: PivotWindow::new(GameDate::new(1938, 6, 1), GameDate::new(1939, 1, 1)),
            milestones: Self::default_milestones(),
            force_goal,
            force_plan,
            equipment_profiles,
            domestic_resources,
            starting_fielded_divisions,
            frontier_forts: Self::default_frontier_requirements(),
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
            initial_country: CountryState::new(start_date, 41_000_000, CountryLaws::default()),
            initial_state_defs,
            initial_states,
            initial_production_lines,
        }
    }

    pub fn from_dataset(dataset: StructuredFrance1936Dataset) -> Result<Self, DataError> {
        if dataset.tag != "FRA" {
            return Err(DataError::Validation(format!(
                "France1936Scenario expects FRA data, got {}",
                dataset.tag
            )));
        }
        if dataset.states.is_empty() {
            return Err(DataError::Validation(
                "France1936Scenario dataset contains no states".to_string(),
            ));
        }
        if dataset.production_lines.is_empty() {
            return Err(DataError::Validation(
                "France1936Scenario dataset contains no production lines".to_string(),
            ));
        }

        let start_date = parse_game_date(&dataset.start_date)?;
        let mut dataset_states = dataset.states;
        dataset_states.sort_by_key(|state| state.raw_state_id);

        let initial_state_defs = dataset_states
            .iter()
            .enumerate()
            .map(|(index, state)| {
                Ok(StateDefinition {
                    id: StateId(u8::try_from(index).map_err(|_| {
                        DataError::Validation(
                            "France1936Scenario exceeds the current dense StateId capacity"
                                .to_string(),
                        )
                    })?),
                    raw_state_id: state.raw_state_id,
                    name: state.source_name.clone().into_boxed_str(),
                    building_slots: state.building_slots,
                    economic_weight: state.economic_weight,
                    infrastructure_target: state.infrastructure_target,
                    frontier: state.frontier,
                    resources: state.resources,
                })
            })
            .collect::<Result<Vec<_>, DataError>>()?
            .into_boxed_slice();
        let initial_states = dataset_states
            .iter()
            .map(|state| StateRuntime {
                civilian_factories: state.civilian_factories,
                military_factories: state.military_factories,
                infrastructure: state.infrastructure,
                land_fort_level: state.land_fort_level,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let initial_production_lines = dataset
            .production_lines
            .into_iter()
            .map(|line| {
                ProductionLine::new_with_cost(line.equipment, line.factories, line.unit_cost_centi)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let domestic_resources = aggregate_domestic_resources(&initial_state_defs);
        let force_goal = ForceGoalSpec::france_1939_default();
        let force_plan = Self::derive_force_plan(
            start_date,
            dataset.population,
            domestic_resources,
            force_goal,
            dataset.equipment_profiles,
            dataset
                .starting_fielded_divisions
                .max(force_goal.division_band().min),
        );

        let economic_construction_order = sorted_state_ids(
            &initial_state_defs,
            &initial_states,
            |definition, runtime| {
                (
                    definition.frontier.is_some(),
                    std::cmp::Reverse(definition.economic_weight),
                    std::cmp::Reverse(definition.building_slots),
                    std::cmp::Reverse(runtime.infrastructure),
                    definition.raw_state_id,
                )
            },
        );
        let infrastructure_order = filtered_sorted_state_ids(
            &initial_state_defs,
            &initial_states,
            |definition, runtime| runtime.infrastructure < definition.infrastructure_target,
            |definition, runtime| {
                (
                    definition.frontier != Some(Frontier::Germany),
                    definition.frontier != Some(Frontier::Belgium),
                    std::cmp::Reverse(definition.economic_weight),
                    std::cmp::Reverse(runtime.infrastructure),
                    definition.raw_state_id,
                )
            },
        );
        let military_construction_order = sorted_state_ids(
            &initial_state_defs,
            &initial_states,
            |definition, runtime| {
                (
                    definition.frontier.is_none(),
                    std::cmp::Reverse(runtime.infrastructure),
                    std::cmp::Reverse(definition.economic_weight),
                    std::cmp::Reverse(definition.building_slots),
                    definition.raw_state_id,
                )
            },
        );
        let frontier_fort_order = filtered_sorted_state_ids(
            &initial_state_defs,
            &initial_states,
            |definition, _| definition.frontier.is_some(),
            |definition, runtime| {
                (
                    frontier_order_priority(definition.frontier),
                    std::cmp::Reverse(runtime.land_fort_level),
                    definition.raw_state_id,
                )
            },
        );

        if !frontier_fort_order.iter().any(|state| {
            initial_state_defs[usize::from(state.0)].frontier == Some(Frontier::Germany)
        }) {
            return Err(DataError::Validation(
                "France1936Scenario dataset did not expose any German frontier states".to_string(),
            ));
        }
        if !frontier_fort_order.iter().any(|state| {
            initial_state_defs[usize::from(state.0)].frontier == Some(Frontier::Belgium)
        }) {
            return Err(DataError::Validation(
                "France1936Scenario dataset did not expose any Belgian frontier states".to_string(),
            ));
        }

        Ok(Self {
            reference_tag: "FRA",
            start_date,
            pivot_window: PivotWindow::new(GameDate::new(1938, 6, 1), GameDate::new(1939, 1, 1)),
            milestones: Self::default_milestones(),
            force_goal,
            force_plan,
            equipment_profiles: dataset.equipment_profiles,
            domestic_resources,
            starting_fielded_divisions: dataset.starting_fielded_divisions,
            frontier_forts: Self::default_frontier_requirements(),
            economic_construction_order,
            infrastructure_order,
            military_construction_order,
            frontier_fort_order,
            initial_country: CountryState::new(start_date, dataset.population, dataset.laws),
            initial_state_defs,
            initial_states,
            initial_production_lines,
        })
    }

    pub fn bootstrap_runtime(&self) -> CountryRuntime {
        CountryRuntime::new(
            self.initial_country,
            self.initial_state_defs.clone(),
            self.initial_states.clone(),
            self.initial_production_lines.clone(),
        )
        .with_fielded_force(
            self.starting_fielded_divisions
                .min(self.force_plan.frontline_divisions),
            self.force_plan.template.per_division_demand(),
        )
    }

    pub fn readiness_demand_for(&self, divisions: u16) -> EquipmentDemand {
        assert!(self.force_goal.division_band().contains(divisions));
        self.force_plan.template.demand_for(divisions)
    }

    fn default_milestones() -> [Milestone; 4] {
        [
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
        ]
    }

    fn default_frontier_requirements() -> [FrontierFortRequirement; 2] {
        [
            FrontierFortRequirement {
                frontier: Frontier::Germany,
                target_level: 5,
            },
            FrontierFortRequirement {
                frontier: Frontier::Belgium,
                target_level: 5,
            },
        ]
    }

    fn derive_force_plan(
        start_date: GameDate,
        population: u64,
        domestic_resources: ResourceLedger,
        force_goal: ForceGoalSpec,
        equipment_profiles: ModeledEquipmentProfiles,
        starting_fielded_divisions: u16,
    ) -> ForcePlan {
        let division_band = force_goal.division_band();
        let min_divisions = division_band
            .min
            .max(starting_fielded_divisions.min(division_band.max));
        let days_to_target =
            u16::try_from(start_date.days_until(force_goal.target_date).max(1)).unwrap_or(u16::MAX);
        let factory_capacity_centi = estimated_factory_capacity_centi(days_to_target).max(1);
        let available_manpower = force_goal
            .target_mobilization_law
            .available_manpower(population);
        let manpower_reserve_floor =
            available_manpower * u64::from(force_goal.manpower_reserve_bp) / 10_000;
        let manpower_budget = available_manpower.saturating_sub(manpower_reserve_floor);
        let mut best_plan = None::<(i64, ForcePlan)>;

        for template in DivisionTemplate::france_line_candidates() {
            for divisions in min_divisions..=division_band.max {
                let frontline_demand = template.demand_for(divisions);
                if u64::from(frontline_demand.manpower) > manpower_budget {
                    continue;
                }

                let starting_fielded_demand =
                    template.demand_for(starting_fielded_divisions.min(divisions));
                let reserve_demand = frontline_demand.reserve_buffer(force_goal.reserve_ratios);
                let stockpile_target_demand = frontline_demand
                    .saturating_sub(starting_fielded_demand)
                    .plus(reserve_demand);
                let total_demand = frontline_demand.plus(reserve_demand);
                let factory_allocation = derive_factory_allocation(
                    stockpile_target_demand,
                    equipment_profiles,
                    factory_capacity_centi,
                );
                let daily_resource_use =
                    derive_daily_resource_use(factory_allocation, equipment_profiles);
                let resource_overdraw = daily_resource_use
                    .saturating_sub(domestic_resources)
                    .total();
                let resource_utilization_bp = daily_resource_use.utilization_bp(domestic_resources);
                let score = i64::from(divisions) * 20_000 + i64::from(resource_utilization_bp) * 4
                    - i64::from(resource_overdraw) * 50_000
                    - i64::from(factory_allocation.total()) * 40
                    - i64::from(template.estimated_ic_cost_centi() / 100);

                let plan = ForcePlan {
                    template,
                    frontline_divisions: divisions,
                    frontline_demand,
                    starting_fielded_demand,
                    reserve_demand,
                    stockpile_target_demand,
                    total_demand,
                    required_military_factories: factory_allocation.total(),
                    factory_allocation,
                    daily_resource_use,
                    resource_utilization_bp,
                };
                match best_plan {
                    Some((best_score, _)) if best_score >= score => {}
                    _ => best_plan = Some((score, plan)),
                }
            }
        }

        best_plan.map(|(_, plan)| plan).unwrap_or_else(|| {
            let template = DivisionTemplate::canonical_france_line();
            let frontline_demand = template.demand_for(min_divisions);
            let starting_fielded_demand =
                template.demand_for(starting_fielded_divisions.min(min_divisions));
            let reserve_demand = frontline_demand.reserve_buffer(force_goal.reserve_ratios);
            let stockpile_target_demand = frontline_demand
                .saturating_sub(starting_fielded_demand)
                .plus(reserve_demand);
            let total_demand = frontline_demand.plus(reserve_demand);
            let factory_allocation = derive_factory_allocation(
                stockpile_target_demand,
                equipment_profiles,
                factory_capacity_centi,
            );

            ForcePlan {
                template,
                frontline_divisions: min_divisions,
                frontline_demand,
                starting_fielded_demand,
                reserve_demand,
                stockpile_target_demand,
                total_demand,
                required_military_factories: factory_allocation.total(),
                factory_allocation,
                daily_resource_use: derive_daily_resource_use(
                    factory_allocation,
                    equipment_profiles,
                ),
                resource_utilization_bp: derive_daily_resource_use(
                    factory_allocation,
                    equipment_profiles,
                )
                .utilization_bp(domestic_resources),
            }
        })
    }
}

impl CountryScenario for France1936Scenario {
    fn reference_tag(&self) -> &'static str {
        self.reference_tag
    }

    fn start_date(&self) -> GameDate {
        self.start_date
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

fn parse_game_date(value: &str) -> Result<GameDate, DataError> {
    let mut parts = value.split('-');
    let Some(year) = parts.next().and_then(|part| part.parse::<u16>().ok()) else {
        return Err(DataError::Validation(format!(
            "invalid start date: {value}"
        )));
    };
    let Some(month) = parts.next().and_then(|part| part.parse::<u8>().ok()) else {
        return Err(DataError::Validation(format!(
            "invalid start date: {value}"
        )));
    };
    let Some(day) = parts.next().and_then(|part| part.parse::<u8>().ok()) else {
        return Err(DataError::Validation(format!(
            "invalid start date: {value}"
        )));
    };

    Ok(GameDate::new(year, month, day))
}

fn aggregate_domestic_resources(definitions: &[StateDefinition]) -> ResourceLedger {
    definitions
        .iter()
        .fold(ResourceLedger::default(), |total, state| {
            total.plus(state.resources)
        })
}

fn estimated_factory_capacity_centi(days: u16) -> u64 {
    let config = SimulationConfig::default();
    let mut efficiency = 100_u16;
    let mut total = 0_u64;

    for _ in 0..days {
        total +=
            u64::from(config.production_output_centi_per_factory) * u64::from(efficiency) / 1_000;
        if efficiency < config.production_efficiency_cap_permille {
            efficiency = (efficiency + config.production_efficiency_gain_permille)
                .min(config.production_efficiency_cap_permille);
        }
    }

    total
}

fn derive_factory_allocation(
    total_demand: EquipmentDemand,
    equipment_profiles: ModeledEquipmentProfiles,
    factory_capacity_centi: u64,
) -> EquipmentFactoryAllocation {
    let mut allocation = EquipmentFactoryAllocation::default();

    for equipment in [
        EquipmentKind::InfantryEquipment,
        EquipmentKind::SupportEquipment,
        EquipmentKind::Artillery,
        EquipmentKind::AntiTank,
        EquipmentKind::AntiAir,
    ] {
        let demand = total_demand.get(equipment);
        if demand == 0 {
            continue;
        }

        let total_ic =
            u64::from(demand) * u64::from(equipment_profiles.profile(equipment).unit_cost_centi);
        let factories = total_ic.div_ceil(factory_capacity_centi);
        allocation.set(equipment, u16::try_from(factories).unwrap_or(u16::MAX));
    }

    allocation
}

fn derive_daily_resource_use(
    allocation: EquipmentFactoryAllocation,
    equipment_profiles: ModeledEquipmentProfiles,
) -> ResourceLedger {
    [
        EquipmentKind::InfantryEquipment,
        EquipmentKind::SupportEquipment,
        EquipmentKind::Artillery,
        EquipmentKind::AntiTank,
        EquipmentKind::AntiAir,
    ]
    .into_iter()
    .fold(
        ResourceLedger::default(),
        |total: ResourceLedger, equipment| {
            total.plus(
                equipment_profiles
                    .profile(equipment)
                    .resources
                    .scale(allocation.get(equipment)),
            )
        },
    )
}

fn sorted_state_ids<K: Ord>(
    definitions: &[StateDefinition],
    states: &[StateRuntime],
    mut key: impl FnMut(&StateDefinition, &StateRuntime) -> K,
) -> Box<[StateId]> {
    let mut indices = (0..definitions.len()).collect::<Vec<_>>();
    indices.sort_by_key(|index| key(&definitions[*index], &states[*index]));
    indices
        .into_iter()
        .map(|index| definitions[index].id)
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn filtered_sorted_state_ids<K: Ord>(
    definitions: &[StateDefinition],
    states: &[StateRuntime],
    mut filter: impl FnMut(&StateDefinition, &StateRuntime) -> bool,
    mut key: impl FnMut(&StateDefinition, &StateRuntime) -> K,
) -> Box<[StateId]> {
    let mut indices = (0..definitions.len())
        .filter(|index| filter(&definitions[*index], &states[*index]))
        .collect::<Vec<_>>();
    indices.sort_by_key(|index| key(&definitions[*index], &states[*index]));
    indices
        .into_iter()
        .map(|index| definitions[index].id)
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn frontier_order_priority(frontier: Option<Frontier>) -> u8 {
    match frontier {
        Some(Frontier::Germany) => 0,
        Some(Frontier::Belgium) => 1,
        None => 2,
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{StructuredFrance1936Dataset, StructuredProductionLine, StructuredState};
    use crate::domain::{
        CountryLaws, EquipmentKind, MilestoneKind, ModeledEquipmentProfiles, ResourceLedger,
        TargetBand,
    };
    use crate::scenario::CountryScenario;

    use super::{France1936Scenario, Frontier};

    #[test]
    fn france_scenario_exposes_approved_default_targets() {
        let scenario = France1936Scenario::standard();

        assert_eq!(scenario.reference_tag, "FRA");
        assert_eq!(scenario.force_goal.division_band(), TargetBand::new(72, 96));
        assert!(scenario.force_plan.required_military_factories > 0);
        assert!(scenario.domestic_resources.steel > 0);
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
        let demand = scenario.readiness_demand_for(scenario.force_goal.division_band().min);

        assert!(demand.infantry_equipment > 0);
        assert!(demand.support_equipment > 0);
        assert!(demand.manpower > 0);
    }

    #[test]
    fn france_scenario_rejects_division_counts_outside_the_target_band() {
        let scenario = France1936Scenario::standard();
        let result = std::panic::catch_unwind(|| {
            scenario.readiness_demand_for(scenario.force_goal.division_band().min - 1)
        });

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

    #[test]
    fn france_scenario_can_be_loaded_from_structured_dataset() {
        let dataset = StructuredFrance1936Dataset {
            version: 1,
            profile: "fixture".to_string(),
            tag: "FRA".to_string(),
            start_date: "1936-01-01".to_string(),
            laws: CountryLaws::default(),
            population: 15_000_000,
            starting_fielded_divisions: 72,
            equipment_profiles: ModeledEquipmentProfiles::default_1936(),
            states: vec![
                StructuredState {
                    raw_state_id: 1,
                    name_token: "STATE_1".to_string(),
                    source_name: "ile_de_france".to_string(),
                    building_slots: 12,
                    economic_weight: 20,
                    infrastructure_target: 9,
                    frontier: None,
                    resources: ResourceLedger {
                        steel: 5,
                        aluminium: 1,
                        ..ResourceLedger::default()
                    },
                    civilian_factories: 8,
                    military_factories: 2,
                    infrastructure: 8,
                    land_fort_level: 0,
                    manpower: 8_000_000,
                },
                StructuredState {
                    raw_state_id: 2,
                    name_token: "STATE_2".to_string(),
                    source_name: "nord".to_string(),
                    building_slots: 8,
                    economic_weight: 15,
                    infrastructure_target: 8,
                    frontier: Some(Frontier::Belgium),
                    resources: ResourceLedger {
                        steel: 7,
                        ..ResourceLedger::default()
                    },
                    civilian_factories: 4,
                    military_factories: 2,
                    infrastructure: 7,
                    land_fort_level: 0,
                    manpower: 4_000_000,
                },
                StructuredState {
                    raw_state_id: 3,
                    name_token: "STATE_3".to_string(),
                    source_name: "lorraine".to_string(),
                    building_slots: 8,
                    economic_weight: 14,
                    infrastructure_target: 8,
                    frontier: Some(Frontier::Germany),
                    resources: ResourceLedger {
                        steel: 10,
                        tungsten: 3,
                        ..ResourceLedger::default()
                    },
                    civilian_factories: 3,
                    military_factories: 2,
                    infrastructure: 7,
                    land_fort_level: 1,
                    manpower: 3_000_000,
                },
            ],
            production_lines: vec![
                StructuredProductionLine {
                    raw_equipment_token: "infantry_equipment_1".to_string(),
                    equipment: EquipmentKind::InfantryEquipment,
                    factories: 8,
                    unit_cost_centi: 50,
                },
                StructuredProductionLine {
                    raw_equipment_token: "fighter_equipment_0".to_string(),
                    equipment: EquipmentKind::Unmodeled,
                    factories: 3,
                    unit_cost_centi: 2_200,
                },
            ],
            warnings: Vec::new(),
        };

        let scenario = France1936Scenario::from_dataset(dataset).unwrap();
        let runtime = scenario.bootstrap_runtime();

        assert_eq!(runtime.state_defs.len(), 3);
        assert_eq!(runtime.total_civilian_factories(), 15);
        assert_eq!(runtime.total_military_factories(), 6);
        assert_eq!(runtime.production_lines.len(), 2);
        assert!(scenario.force_plan.required_military_factories > 0);
        assert_eq!(
            runtime.production_lines[1].equipment,
            EquipmentKind::Unmodeled
        );
    }
}
