use crate::domain::{CountryLaws, EquipmentDemand, EquipmentKind, GameDate, ResourceLedger};
use crate::scenario::{Frontier, FrontierFortRequirement};

use super::actions::{ConstructionKind, FocusBranch, ResearchBranch, StateId};

pub const POLITICAL_POWER_UNIT: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategicPhase {
    PrePivot,
    PostPivot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountryState {
    pub date: GameDate,
    pub population: u64,
    pub political_power_centi: u32,
    pub political_power_daily_centi: u16,
    pub laws: CountryLaws,
}

impl CountryState {
    pub fn new(date: GameDate, population: u64, laws: CountryLaws) -> Self {
        assert!(population > 0);

        Self {
            date,
            population,
            political_power_centi: 0,
            political_power_daily_centi: 2 * POLITICAL_POWER_UNIT as u16,
            laws,
        }
    }

    pub fn advance_day(&mut self, daily_bonus_centi: u16) {
        self.date = self.date.next_day();
        self.political_power_centi += u32::from(self.political_power_daily_centi);
        self.political_power_centi += u32::from(daily_bonus_centi);
    }

    pub fn available_manpower(&self) -> u64 {
        self.laws.mobilization.available_manpower(self.population)
    }

    pub fn can_spend_political_power(&self, cost_centi: u32) -> bool {
        self.political_power_centi >= cost_centi
    }

    pub fn spend_political_power(&mut self, cost_centi: u32) -> bool {
        if !self.can_spend_political_power(cost_centi) {
            return false;
        }

        self.political_power_centi -= cost_centi;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDefinition {
    pub id: StateId,
    pub raw_state_id: u32,
    pub name: Box<str>,
    pub building_slots: u8,
    pub economic_weight: u16,
    pub infrastructure_target: u8,
    pub frontier: Option<Frontier>,
    pub resources: ResourceLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRuntime {
    pub civilian_factories: u8,
    pub military_factories: u8,
    pub infrastructure: u8,
    pub land_fort_level: u8,
}

impl StateRuntime {
    pub fn used_slots(self) -> u8 {
        self.civilian_factories + self.military_factories
    }

    pub fn free_slots(self, definition: &StateDefinition) -> u8 {
        definition.building_slots.saturating_sub(self.used_slots())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stockpile {
    pub infantry_equipment: u32,
    pub support_equipment: u32,
    pub artillery: u32,
    pub anti_tank: u32,
    pub anti_air: u32,
    pub unmodeled_equipment: u32,
}

impl Stockpile {
    pub fn add(&mut self, equipment: EquipmentKind, amount: u32) {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment += amount,
            EquipmentKind::SupportEquipment => self.support_equipment += amount,
            EquipmentKind::Artillery => self.artillery += amount,
            EquipmentKind::AntiTank => self.anti_tank += amount,
            EquipmentKind::AntiAir => self.anti_air += amount,
            EquipmentKind::Unmodeled => self.unmodeled_equipment += amount,
        }
    }

    pub fn get(self, equipment: EquipmentKind) -> u32 {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
            EquipmentKind::Unmodeled => self.unmodeled_equipment,
        }
    }

    pub fn ready_divisions(self, demand: EquipmentDemand, manpower_available: u64) -> u16 {
        assert!(demand.infantry_equipment > 0);
        assert!(demand.manpower > 0);

        let manpower_limit = manpower_available / u64::from(demand.manpower);
        let limit_for = |stockpile: u32, required: u32| {
            if required == 0 {
                u32::MAX
            } else {
                stockpile / required
            }
        };
        let equipment_limits = [
            limit_for(self.infantry_equipment, demand.infantry_equipment),
            limit_for(self.support_equipment, demand.support_equipment),
            limit_for(self.artillery, demand.artillery),
            limit_for(self.anti_tank, demand.anti_tank),
            limit_for(self.anti_air, demand.anti_air),
        ];

        let equipment_limit = equipment_limits.into_iter().min().unwrap_or(0);
        let divisions = equipment_limit.min(u32::try_from(manpower_limit).unwrap_or(u32::MAX));

        u16::try_from(divisions).unwrap_or(u16::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructionProject {
    pub state: StateId,
    pub kind: ConstructionKind,
    pub total_cost_centi: u32,
    pub progress_centi: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductionLine {
    pub equipment: EquipmentKind,
    pub factories: u8,
    pub unit_cost_centi: u32,
    pub efficiency_permille: u16,
    pub accumulated_ic_centi: u32,
}

impl ProductionLine {
    pub fn new(equipment: EquipmentKind, factories: u8) -> Self {
        Self::new_with_cost(equipment, factories, equipment.default_unit_cost_centi())
    }

    pub fn new_with_cost(equipment: EquipmentKind, factories: u8, unit_cost_centi: u32) -> Self {
        Self {
            equipment,
            factories,
            unit_cost_centi,
            efficiency_permille: 100,
            accumulated_ic_centi: 0,
        }
    }

    pub fn reassign(&mut self, equipment: EquipmentKind, factories: u8) {
        if self.equipment != equipment {
            self.efficiency_permille = 100;
            self.accumulated_ic_centi = 0;
            self.unit_cost_centi = equipment.default_unit_cost_centi();
        }

        self.equipment = equipment;
        self.factories = factories;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FocusSummary {
    pub economy: u16,
    pub industry: u16,
    pub military_industry: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResearchSummary {
    pub industry: u16,
    pub construction: u16,
    pub electronics: u16,
    pub production: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct AdvisorRoster {
    pub industry: bool,
    pub research: bool,
    pub military_industry: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusProgress {
    pub branch: FocusBranch,
    pub days_progress: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResearchSlotState {
    pub branch: Option<ResearchBranch>,
    pub days_progress: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountryRuntime {
    pub country: CountryState,
    pub state_defs: Box<[StateDefinition]>,
    pub states: Box<[StateRuntime]>,
    pub stockpile: Stockpile,
    pub fielded_divisions: u16,
    pub fielded_demand: EquipmentDemand,
    pub production_lines: Box<[ProductionLine]>,
    pub construction_queue: Vec<ConstructionProject>,
    pub focus: Option<FocusProgress>,
    pub completed_focuses: FocusSummary,
    pub research_slots: [ResearchSlotState; 2],
    pub completed_research: ResearchSummary,
    pub advisors: AdvisorRoster,
}

impl CountryRuntime {
    pub fn new(
        country: CountryState,
        state_defs: Box<[StateDefinition]>,
        states: Box<[StateRuntime]>,
        production_lines: Box<[ProductionLine]>,
    ) -> Self {
        assert_eq!(state_defs.len(), states.len());

        Self {
            country,
            state_defs,
            states,
            stockpile: Stockpile::default(),
            fielded_divisions: 0,
            fielded_demand: EquipmentDemand::default(),
            production_lines,
            construction_queue: Vec::with_capacity(64),
            focus: None,
            completed_focuses: FocusSummary::default(),
            research_slots: [ResearchSlotState::default(), ResearchSlotState::default()],
            completed_research: ResearchSummary::default(),
            advisors: AdvisorRoster::default(),
        }
    }

    pub fn with_fielded_force(
        mut self,
        divisions: u16,
        per_division_demand: EquipmentDemand,
    ) -> Self {
        self.fielded_divisions = divisions;
        self.fielded_demand = per_division_demand.scale(divisions);
        self
    }

    pub fn state_index(&self, state: StateId) -> usize {
        let index = usize::from(state.0);
        assert!(index < self.states.len());
        assert_eq!(self.state_defs[index].id, state);
        index
    }

    pub fn state(&self, state: StateId) -> &StateRuntime {
        let index = self.state_index(state);
        &self.states[index]
    }

    pub fn state_mut(&mut self, state: StateId) -> &mut StateRuntime {
        let index = self.state_index(state);
        &mut self.states[index]
    }

    pub fn total_civilian_factories(&self) -> u16 {
        self.states
            .iter()
            .map(|state| u16::from(state.civilian_factories))
            .sum()
    }

    pub fn total_military_factories(&self) -> u16 {
        self.states
            .iter()
            .map(|state| u16::from(state.military_factories))
            .sum()
    }

    pub fn assigned_military_factories(&self) -> u16 {
        self.production_lines
            .iter()
            .map(|line| u16::from(line.factories))
            .sum()
    }

    pub fn unassigned_military_factories(&self) -> u16 {
        self.total_military_factories()
            .saturating_sub(self.assigned_military_factories())
    }

    pub fn queued_factory_projects(&self, state: StateId) -> u8 {
        self.construction_queue
            .iter()
            .filter(|project| project.state == state)
            .filter(|project| {
                matches!(
                    project.kind,
                    ConstructionKind::CivilianFactory | ConstructionKind::MilitaryFactory
                )
            })
            .count() as u8
    }

    pub fn consumer_goods_factories(&self) -> u16 {
        let ratio_bp = match self.country.laws.economy {
            crate::domain::EconomyLaw::CivilianEconomy => 3_000_u16,
            crate::domain::EconomyLaw::EarlyMobilization => 2_500_u16,
            crate::domain::EconomyLaw::PartialMobilization => 2_000_u16,
            crate::domain::EconomyLaw::WarEconomy => 1_500_u16,
        };

        let total_factories =
            u32::from(self.total_civilian_factories() + self.total_military_factories());
        let goods = (total_factories * u32::from(ratio_bp)).div_ceil(10_000);

        u16::try_from(goods).unwrap_or(u16::MAX)
    }

    pub fn available_civilian_factories(&self) -> u16 {
        self.total_civilian_factories()
            .saturating_sub(self.consumer_goods_factories())
    }

    pub fn construction_speed_bp(&self) -> u16 {
        let mut bonus = 0_u16;
        bonus += self.completed_focuses.economy * 100;
        bonus += self.completed_focuses.industry * 150;
        bonus += self.completed_research.construction * 200;
        bonus += self.completed_research.industry * 100;

        if self.advisors.industry {
            bonus += 100;
        }

        bonus
    }

    pub fn military_output_bp(&self) -> u16 {
        let mut bonus = 0_u16;
        bonus += self.completed_focuses.military_industry * 200;
        bonus += self.completed_research.production * 250;

        if self.advisors.military_industry {
            bonus += 100;
        }

        bonus
    }

    pub fn political_power_daily_bonus_centi(&self) -> u16 {
        let mut bonus = 0_u16;

        if self.advisors.research {
            bonus += 25;
        }

        bonus
    }

    pub fn apply_focus_completion(&mut self, branch: FocusBranch) {
        match branch {
            FocusBranch::Economy => self.completed_focuses.economy += 1,
            FocusBranch::Industry => self.completed_focuses.industry += 1,
            FocusBranch::MilitaryIndustry => self.completed_focuses.military_industry += 1,
            FocusBranch::Politics | FocusBranch::Diplomacy => {}
        }
    }

    pub fn apply_research_completion(&mut self, branch: ResearchBranch) {
        match branch {
            ResearchBranch::Industry => self.completed_research.industry += 1,
            ResearchBranch::Construction => self.completed_research.construction += 1,
            ResearchBranch::Electronics => self.completed_research.electronics += 1,
            ResearchBranch::Production => self.completed_research.production += 1,
        }
    }

    pub fn frontier_forts_complete(&self, requirements: &[FrontierFortRequirement]) -> bool {
        requirements.iter().all(|requirement| {
            self.state_defs
                .iter()
                .zip(self.states.iter())
                .filter(|(definition, _)| definition.frontier == Some(requirement.frontier))
                .all(|(_, runtime)| runtime.land_fort_level >= requirement.target_level)
        })
    }

    pub fn domestic_resources(&self) -> ResourceLedger {
        self.state_defs
            .iter()
            .fold(ResourceLedger::default(), |total, state| {
                total.plus(state.resources)
            })
    }

    pub fn ready_divisions(&self, demand: EquipmentDemand) -> u16 {
        self.stockpile
            .ready_divisions(demand, self.country.available_manpower())
    }

    pub fn supported_divisions(&self, demand: EquipmentDemand) -> u16 {
        let free_manpower = self
            .country
            .available_manpower()
            .saturating_sub(u64::from(self.fielded_demand.manpower));

        self.fielded_divisions
            .saturating_add(self.stockpile.ready_divisions(demand, free_manpower))
    }

    pub fn queued_kind_projects(&self, state: StateId, kind: ConstructionKind) -> u8 {
        self.construction_queue
            .iter()
            .filter(|project| project.state == state && project.kind == kind)
            .count() as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrancePlanningState {
    pub country: CountryState,
    pub pivot_date: GameDate,
    pub military_factory_target_met: bool,
    pub frontier_forts_met: bool,
}

impl FrancePlanningState {
    pub fn phase(&self) -> StrategicPhase {
        if self.country.date < self.pivot_date {
            StrategicPhase::PrePivot
        } else {
            StrategicPhase::PostPivot
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{CountryLaws, DivisionTemplate, EquipmentKind, GameDate, ResourceLedger};
    use crate::scenario::{France1936Scenario, Frontier};
    use crate::sim::actions::{ConstructionKind, StateId};

    use super::{
        CountryRuntime, CountryState, FrancePlanningState, POLITICAL_POWER_UNIT, ProductionLine,
        StateDefinition, StateRuntime, Stockpile, StrategicPhase,
    };

    fn test_runtime() -> CountryRuntime {
        CountryRuntime::new(
            CountryState::new(
                GameDate::new(1936, 1, 1),
                40_000_000,
                CountryLaws::default(),
            ),
            vec![
                StateDefinition {
                    id: StateId(0),
                    raw_state_id: 16,
                    name: "paris".into(),
                    building_slots: 12,
                    economic_weight: 10,
                    infrastructure_target: 8,
                    frontier: None,
                    resources: ResourceLedger::default(),
                },
                StateDefinition {
                    id: StateId(1),
                    raw_state_id: 17,
                    name: "lorraine".into(),
                    building_slots: 8,
                    economic_weight: 7,
                    infrastructure_target: 7,
                    frontier: Some(Frontier::Germany),
                    resources: ResourceLedger {
                        steel: 12,
                        tungsten: 4,
                        ..ResourceLedger::default()
                    },
                },
            ]
            .into_boxed_slice(),
            vec![
                StateRuntime {
                    civilian_factories: 10,
                    military_factories: 4,
                    infrastructure: 8,
                    land_fort_level: 0,
                },
                StateRuntime {
                    civilian_factories: 4,
                    military_factories: 2,
                    infrastructure: 6,
                    land_fort_level: 5,
                },
            ]
            .into_boxed_slice(),
            vec![ProductionLine::new(EquipmentKind::InfantryEquipment, 5)].into_boxed_slice(),
        )
    }

    #[test]
    fn country_state_advances_daily_time_and_political_power() {
        let mut country = CountryState::new(
            GameDate::new(1936, 1, 1),
            40_000_000,
            CountryLaws::default(),
        );
        country.advance_day(0);

        assert_eq!(country.date, GameDate::new(1936, 1, 2));
        assert_eq!(country.political_power_centi, 2 * POLITICAL_POWER_UNIT);
    }

    #[test]
    fn country_state_uses_mobilization_law_for_manpower() {
        let country = CountryState::new(
            GameDate::new(1936, 1, 1),
            40_000_000,
            CountryLaws::default(),
        );

        assert_eq!(country.available_manpower(), 1_000_000);
    }

    #[test]
    fn country_runtime_uses_dense_state_ids() {
        let runtime = test_runtime();

        assert_eq!(runtime.state(StateId(1)).military_factories, 2);
    }

    #[test]
    fn country_runtime_counts_consumer_goods_from_total_factories() {
        let runtime = test_runtime();

        assert_eq!(runtime.total_civilian_factories(), 14);
        assert_eq!(runtime.total_military_factories(), 6);
        assert_eq!(runtime.consumer_goods_factories(), 6);
        assert_eq!(runtime.available_civilian_factories(), 8);
    }

    #[test]
    fn stockpile_converts_equipment_into_ready_divisions() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.per_division_demand();
        let stockpile = Stockpile {
            infantry_equipment: demand.infantry_equipment * 3,
            support_equipment: demand.support_equipment * 3,
            artillery: demand.artillery * 3,
            anti_tank: demand.anti_tank * 3,
            anti_air: demand.anti_air * 3,
            unmodeled_equipment: 0,
        };

        assert_eq!(stockpile.ready_divisions(demand, 500_000), 3);
    }

    #[test]
    fn france_planning_state_enters_post_pivot_on_pivot_day() {
        let country = CountryState::new(
            GameDate::new(1938, 6, 1),
            40_000_000,
            CountryLaws::default(),
        );
        let planning = FrancePlanningState {
            country,
            pivot_date: GameDate::new(1938, 6, 1),
            military_factory_target_met: false,
            frontier_forts_met: false,
        };

        assert_eq!(planning.phase(), StrategicPhase::PostPivot);
    }

    #[test]
    fn runtime_detects_when_frontier_fort_requirements_are_satisfied() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();

        assert!(!runtime.frontier_forts_complete(&scenario.frontier_forts));
    }

    #[test]
    fn runtime_aggregates_static_domestic_resources() {
        let runtime = test_runtime();

        assert_eq!(
            runtime.domestic_resources(),
            ResourceLedger {
                steel: 12,
                tungsten: 4,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn supported_divisions_include_fielded_force_and_reserve_stockpile() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.per_division_demand();
        let runtime = test_runtime().with_fielded_force(24, demand);
        let mut stocked_runtime = runtime.clone();
        stocked_runtime.stockpile = Stockpile {
            infantry_equipment: demand.infantry_equipment * 2,
            support_equipment: demand.support_equipment * 2,
            artillery: demand.artillery * 2,
            anti_tank: demand.anti_tank * 2,
            anti_air: demand.anti_air * 2,
            unmodeled_equipment: 0,
        };

        assert_eq!(stocked_runtime.supported_divisions(demand), 26);
    }

    #[test]
    fn queued_factory_project_count_ignores_non_slot_buildings() {
        let mut runtime = test_runtime();
        runtime.construction_queue.push(super::ConstructionProject {
            state: StateId(0),
            kind: ConstructionKind::CivilianFactory,
            total_cost_centi: 1,
            progress_centi: 0,
        });
        runtime.construction_queue.push(super::ConstructionProject {
            state: StateId(0),
            kind: ConstructionKind::Infrastructure,
            total_cost_centi: 1,
            progress_centi: 0,
        });

        assert_eq!(runtime.queued_factory_projects(StateId(0)), 1);
    }
}
