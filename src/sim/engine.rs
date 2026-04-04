use crate::domain::GameDate;
use crate::scenario::France1936Scenario;

use super::actions::{
    Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, FocusAction,
    LawAction, LawCategory, LawTarget, ProductionAction, ResearchAction, ResearchBranch,
};
use super::rules::{ConstructionDecisionContext, FranceHeuristicRules, RuleViolation};
use super::state::{
    ConstructionProject, CountryRuntime, FocusProgress, POLITICAL_POWER_UNIT, StrategicPhase,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationConfig {
    pub civilian_factory_cost_centi: u32,
    pub military_factory_cost_centi: u32,
    pub infrastructure_cost_centi: u32,
    pub land_fort_cost_centi: u32,
    pub construction_output_centi_per_factory: u16,
    pub production_output_centi_per_factory: u16,
    pub production_efficiency_gain_permille: u16,
    pub production_efficiency_cap_permille: u16,
    pub max_civs_per_project: u8,
    pub focus_days: u16,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            civilian_factory_cost_centi: 1_080_000,
            military_factory_cost_centi: 720_000,
            infrastructure_cost_centi: 300_000,
            land_fort_cost_centi: 300_000,
            construction_output_centi_per_factory: 500,
            production_output_centi_per_factory: 450,
            production_efficiency_gain_permille: 5,
            production_efficiency_cap_permille: 1_000,
            max_civs_per_project: 15,
            focus_days: 70,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimulationError {
    ActionsOutOfOrder,
    ActionDateOutOfRange(GameDate),
    InvalidState(super::actions::StateId),
    NoFreeFactorySlot(super::actions::StateId),
    FocusAlreadyInProgress,
    InsufficientPoliticalPower,
    ResearchSlotBusy(u8),
    InvalidResearchSlot(u8),
    InvalidProductionSlot(u8),
    DuplicateProductionSlot(u8),
    DuplicateResearchBranch(ResearchBranch),
    DuplicateLawCategory(LawCategory),
    LawAlreadySet(LawTarget),
    HeuristicViolation(RuleViolation),
    DuplicateAdvisor(AdvisorKind),
    HardRequirementsUnsatisfied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationOutcome {
    pub country: CountryRuntime,
    pub applied_actions: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SimulationEngine {
    pub config: SimulationConfig,
}

impl SimulationEngine {
    pub fn new(config: SimulationConfig) -> Self {
        Self { config }
    }

    pub fn simulate(
        &self,
        scenario: &France1936Scenario,
        mut country: CountryRuntime,
        actions: &[Action],
        end: GameDate,
        pivot_date: GameDate,
    ) -> Result<SimulationOutcome, SimulationError> {
        assert!(scenario.pivot_window.contains(pivot_date));

        if actions
            .windows(2)
            .any(|pair| pair[0].date() > pair[1].date())
        {
            return Err(SimulationError::ActionsOutOfOrder);
        }

        let mut action_index = 0_usize;

        loop {
            let day_start = action_index;
            while action_index < actions.len()
                && actions[action_index].date() == country.country.date
            {
                action_index += 1;
            }

            self.validate_same_day_actions(&country, &actions[day_start..action_index])?;

            for action in &actions[day_start..action_index] {
                self.apply_action(scenario, &mut country, *action, pivot_date)?;
            }

            self.progress_focus(&mut country);
            self.progress_research(&mut country);
            self.advance_construction(&mut country);
            self.advance_production(&mut country);

            if country.country.date == end {
                break;
            }

            country
                .country
                .advance_day(country.political_power_daily_bonus_centi());
        }

        if action_index < actions.len() {
            return Err(SimulationError::ActionDateOutOfRange(
                actions[action_index].date(),
            ));
        }

        Ok(SimulationOutcome {
            country,
            applied_actions: action_index,
        })
    }

    fn validate_same_day_actions(
        &self,
        country: &CountryRuntime,
        actions: &[Action],
    ) -> Result<(), SimulationError> {
        let mut focus_seen = false;
        let mut advisor_seen = [false; AdvisorKind::COUNT];
        let mut law_seen = [false; LawCategory::COUNT];
        let mut research_slot_seen = [false; 256];
        let mut production_slot_seen = [false; 256];
        let mut research_branch_seen = [false; ResearchBranch::COUNT];

        for slot in &country.research_slots {
            if let Some(branch) = slot.branch {
                research_branch_seen[branch.index()] = true;
            }
        }

        for action in actions {
            match *action {
                Action::Construction(_) => {}
                Action::Focus(_) => {
                    if focus_seen || country.focus.is_some() {
                        return Err(SimulationError::FocusAlreadyInProgress);
                    }
                    focus_seen = true;
                }
                Action::Law(action) => {
                    let category = action.target.category();
                    if law_seen[category.index()] {
                        return Err(SimulationError::DuplicateLawCategory(category));
                    }
                    law_seen[category.index()] = true;
                }
                Action::Advisor(action) => {
                    if advisor_seen[action.kind.index()] {
                        return Err(SimulationError::DuplicateAdvisor(action.kind));
                    }
                    advisor_seen[action.kind.index()] = true;
                }
                Action::Research(action) => {
                    let slot_index = usize::from(action.slot);
                    if slot_index >= country.research_slots.len() {
                        return Err(SimulationError::InvalidResearchSlot(action.slot));
                    }
                    if country.research_slots[slot_index].branch.is_some()
                        || research_slot_seen[slot_index]
                    {
                        return Err(SimulationError::ResearchSlotBusy(action.slot));
                    }
                    if research_branch_seen[action.branch.index()] {
                        return Err(SimulationError::DuplicateResearchBranch(action.branch));
                    }

                    research_slot_seen[slot_index] = true;
                    research_branch_seen[action.branch.index()] = true;
                }
                Action::Production(action) => {
                    let slot_index = usize::from(action.slot);
                    if slot_index >= country.production_lines.len() {
                        return Err(SimulationError::InvalidProductionSlot(action.slot));
                    }
                    if production_slot_seen[slot_index] {
                        return Err(SimulationError::DuplicateProductionSlot(action.slot));
                    }

                    production_slot_seen[slot_index] = true;
                }
            }
        }

        Ok(())
    }

    fn apply_action(
        &self,
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        action: Action,
        pivot_date: GameDate,
    ) -> Result<(), SimulationError> {
        match action {
            Action::Construction(action) => {
                self.apply_construction_action(scenario, country, action, pivot_date)
            }
            Action::Production(action) => self.apply_production_action(scenario, country, action),
            Action::Focus(action) => self.apply_focus_action(country, action, pivot_date),
            Action::Law(action) => self.apply_law_action(country, action, pivot_date),
            Action::Advisor(action) => self.apply_advisor_action(country, action, pivot_date),
            Action::Research(action) => self.apply_research_action(country, action),
        }
    }

    fn apply_construction_action(
        &self,
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        action: ConstructionAction,
        pivot_date: GameDate,
    ) -> Result<(), SimulationError> {
        let state_index = self
            .state_index(country, action.state)
            .ok_or(SimulationError::InvalidState(action.state))?;
        let definition = &country.state_defs[state_index];
        let runtime = country.states[state_index];

        let phase = self.phase_for(country.country.date, pivot_date);
        let context = ConstructionDecisionContext {
            phase,
            military_factory_target_met: country.total_military_factories()
                >= scenario.force_plan.required_military_factories,
            minimum_force_target_met: country
                .supported_divisions(scenario.force_plan.template.per_division_demand())
                >= scenario.force_goal.division_band().min,
            frontier_forts_met: country.frontier_forts_complete(&scenario.frontier_forts),
            civilian_exception: false,
            infrastructure_is_justified: runtime.infrastructure < definition.infrastructure_target,
        };

        FranceHeuristicRules::validate_construction(context, action.kind)
            .map_err(SimulationError::HeuristicViolation)?;

        if matches!(
            action.kind,
            ConstructionKind::CivilianFactory | ConstructionKind::MilitaryFactory
        ) {
            let queued = country.queued_factory_projects(action.state);
            if runtime.free_slots(definition) <= queued {
                return Err(SimulationError::NoFreeFactorySlot(action.state));
            }
        }

        country.construction_queue.push(ConstructionProject {
            state: action.state,
            kind: action.kind,
            total_cost_centi: self.construction_cost(action.kind),
            progress_centi: 0,
        });

        Ok(())
    }

    fn apply_focus_action(
        &self,
        country: &mut CountryRuntime,
        action: FocusAction,
        pivot_date: GameDate,
    ) -> Result<(), SimulationError> {
        let phase = self.phase_for(country.country.date, pivot_date);
        FranceHeuristicRules::validate_focus_branch(phase, action.branch)
            .map_err(SimulationError::HeuristicViolation)?;

        if country.focus.is_some() {
            return Err(SimulationError::FocusAlreadyInProgress);
        }

        country.focus = Some(FocusProgress {
            branch: action.branch,
            days_progress: 0,
        });

        Ok(())
    }

    fn apply_law_action(
        &self,
        country: &mut CountryRuntime,
        action: LawAction,
        pivot_date: GameDate,
    ) -> Result<(), SimulationError> {
        let phase = self.phase_for(country.country.date, pivot_date);
        FranceHeuristicRules::validate_law_target(phase, action.target)
            .map_err(SimulationError::HeuristicViolation)?;

        if self.law_is_active(country, action.target) {
            return Err(SimulationError::LawAlreadySet(action.target));
        }

        if !country
            .country
            .spend_political_power(150 * POLITICAL_POWER_UNIT)
        {
            return Err(SimulationError::InsufficientPoliticalPower);
        }

        match action.target {
            LawTarget::Economy(law) => country.country.laws.economy = law,
            LawTarget::Trade(law) => country.country.laws.trade = law,
            LawTarget::Mobilization(law) => country.country.laws.mobilization = law,
        }

        Ok(())
    }

    fn apply_advisor_action(
        &self,
        country: &mut CountryRuntime,
        action: AdvisorAction,
        pivot_date: GameDate,
    ) -> Result<(), SimulationError> {
        let phase = self.phase_for(country.country.date, pivot_date);
        FranceHeuristicRules::validate_advisor(phase, action.kind)
            .map_err(SimulationError::HeuristicViolation)?;

        if self.advisor_is_active(country, action.kind) {
            return Err(SimulationError::DuplicateAdvisor(action.kind));
        }

        if !country
            .country
            .spend_political_power(150 * POLITICAL_POWER_UNIT)
        {
            return Err(SimulationError::InsufficientPoliticalPower);
        }

        match action.kind {
            AdvisorKind::IndustryConcern => {
                country.advisors.industry = true;
            }
            AdvisorKind::ResearchInstitute => {
                country.advisors.research = true;
            }
            AdvisorKind::MilitaryIndustrialist => {
                country.advisors.military_industry = true;
            }
        }

        Ok(())
    }

    fn apply_research_action(
        &self,
        country: &mut CountryRuntime,
        action: ResearchAction,
    ) -> Result<(), SimulationError> {
        let slot_index = usize::from(action.slot);
        if slot_index >= country.research_slots.len() {
            return Err(SimulationError::InvalidResearchSlot(action.slot));
        }

        if country.research_slots[slot_index].branch.is_some() {
            return Err(SimulationError::ResearchSlotBusy(action.slot));
        }
        if country
            .research_slots
            .iter()
            .any(|slot| slot.branch == Some(action.branch))
        {
            return Err(SimulationError::DuplicateResearchBranch(action.branch));
        }

        country.research_slots[slot_index].branch = Some(action.branch);
        country.research_slots[slot_index].days_progress = 0;

        Ok(())
    }

    fn apply_production_action(
        &self,
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        action: ProductionAction,
    ) -> Result<(), SimulationError> {
        let slot_index = usize::from(action.slot);
        if slot_index >= country.production_lines.len() {
            return Err(SimulationError::InvalidProductionSlot(action.slot));
        }

        let line = &mut country.production_lines[slot_index];
        let changed_line_assignment =
            line.equipment != action.equipment || line.factories != action.factories;
        let demand_justified = country.stockpile.get(action.equipment)
            < scenario
                .force_plan
                .stockpile_target_demand
                .get(action.equipment);

        FranceHeuristicRules::validate_production_retune(super::rules::ProductionDecisionContext {
            changed_line_assignment,
            demand_justified,
        })
        .map_err(SimulationError::HeuristicViolation)?;

        line.reassign(action.equipment, action.factories);

        Ok(())
    }

    fn progress_focus(&self, country: &mut CountryRuntime) {
        let Some(mut focus) = country.focus else {
            return;
        };

        focus.days_progress += 1;
        if focus.days_progress >= self.config.focus_days {
            country.apply_focus_completion(focus.branch);
            country.focus = None;
            return;
        }

        country.focus = Some(focus);
    }

    fn progress_research(&self, country: &mut CountryRuntime) {
        for slot_index in 0..country.research_slots.len() {
            let Some(branch) = country.research_slots[slot_index].branch else {
                continue;
            };

            country.research_slots[slot_index].days_progress += 1;
            if country.research_slots[slot_index].days_progress >= self.research_days(branch) {
                country.apply_research_completion(branch);
                country.research_slots[slot_index] = super::state::ResearchSlotState::default();
            }
        }
    }

    fn advance_construction(&self, country: &mut CountryRuntime) {
        let available_civs = usize::from(country.available_civilian_factories());
        if available_civs == 0 || country.construction_queue.is_empty() {
            return;
        }

        let active_projects = country
            .construction_queue
            .len()
            .min(available_civs.div_ceil(usize::from(self.config.max_civs_per_project)));

        let mut remaining_civs = available_civs;
        for index in 0..active_projects {
            let assigned_civs = remaining_civs.min(usize::from(self.config.max_civs_per_project));
            remaining_civs -= assigned_civs;

            let state_index = usize::from(country.construction_queue[index].state.0);
            let infrastructure = u32::from(country.states[state_index].infrastructure);
            let infrastructure_multiplier_bp = 10_000 + infrastructure * 1_000;
            let construction_speed_bp = u32::from(country.construction_speed_bp());
            let daily_progress = u64::try_from(assigned_civs).unwrap_or(u64::MAX)
                * u64::from(self.config.construction_output_centi_per_factory)
                * u64::from(infrastructure_multiplier_bp)
                * u64::from(10_000 + construction_speed_bp)
                / 10_000
                / 10_000;

            country.construction_queue[index].progress_centi +=
                u32::try_from(daily_progress).unwrap_or(u32::MAX);
        }

        let mut index = 0_usize;
        while index < country.construction_queue.len() {
            if country.construction_queue[index].progress_centi
                >= country.construction_queue[index].total_cost_centi
            {
                let project = country.construction_queue.remove(index);
                self.finish_construction(country, project);
            } else {
                index += 1;
            }
        }
    }

    fn finish_construction(&self, country: &mut CountryRuntime, project: ConstructionProject) {
        let state = country.state_mut(project.state);

        match project.kind {
            ConstructionKind::CivilianFactory => state.civilian_factories += 1,
            ConstructionKind::MilitaryFactory => state.military_factories += 1,
            ConstructionKind::Infrastructure => state.infrastructure += 1,
            ConstructionKind::LandFort => state.land_fort_level += 1,
        }
    }

    fn advance_production(&self, country: &mut CountryRuntime) {
        let output_bonus_bp = u32::from(country.military_output_bp());

        for line in &mut country.production_lines {
            if line.factories == 0 {
                continue;
            }

            let daily_ic_centi = u64::from(line.factories)
                * u64::from(self.config.production_output_centi_per_factory)
                * u64::from(line.efficiency_permille)
                * u64::from(10_000 + output_bonus_bp)
                / 1_000
                / 10_000;

            line.accumulated_ic_centi += u32::try_from(daily_ic_centi).unwrap_or(u32::MAX);

            let produced_units = line.accumulated_ic_centi / line.unit_cost_centi;
            line.accumulated_ic_centi %= line.unit_cost_centi;

            country.stockpile.add(line.equipment, produced_units);

            if line.efficiency_permille < self.config.production_efficiency_cap_permille {
                line.efficiency_permille = (line.efficiency_permille
                    + self.config.production_efficiency_gain_permille)
                    .min(self.config.production_efficiency_cap_permille);
            }
        }
    }

    fn research_days(&self, branch: super::actions::ResearchBranch) -> u16 {
        match branch {
            super::actions::ResearchBranch::Industry => 140,
            super::actions::ResearchBranch::Construction => 120,
            super::actions::ResearchBranch::Electronics => 150,
            super::actions::ResearchBranch::Production => 130,
        }
    }

    fn construction_cost(&self, kind: ConstructionKind) -> u32 {
        match kind {
            ConstructionKind::CivilianFactory => self.config.civilian_factory_cost_centi,
            ConstructionKind::MilitaryFactory => self.config.military_factory_cost_centi,
            ConstructionKind::Infrastructure => self.config.infrastructure_cost_centi,
            ConstructionKind::LandFort => self.config.land_fort_cost_centi,
        }
    }

    fn phase_for(&self, date: GameDate, pivot_date: GameDate) -> StrategicPhase {
        if date < pivot_date {
            StrategicPhase::PrePivot
        } else {
            StrategicPhase::PostPivot
        }
    }

    fn law_is_active(&self, country: &CountryRuntime, target: LawTarget) -> bool {
        match target {
            LawTarget::Economy(law) => country.country.laws.economy == law,
            LawTarget::Trade(law) => country.country.laws.trade == law,
            LawTarget::Mobilization(law) => country.country.laws.mobilization == law,
        }
    }

    fn advisor_is_active(&self, country: &CountryRuntime, advisor: AdvisorKind) -> bool {
        match advisor {
            AdvisorKind::IndustryConcern => country.advisors.industry,
            AdvisorKind::ResearchInstitute => country.advisors.research,
            AdvisorKind::MilitaryIndustrialist => country.advisors.military_industry,
        }
    }

    fn state_index(
        &self,
        country: &CountryRuntime,
        state: super::actions::StateId,
    ) -> Option<usize> {
        let index = usize::from(state.0);
        if index >= country.states.len() {
            return None;
        }

        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::domain::{EconomyLaw, GameDate};
    use crate::scenario::France1936Scenario;
    use crate::sim::{
        Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, FocusAction,
        FocusBranch, LawAction, LawCategory, LawTarget, ProductionAction, ResearchAction,
        ResearchBranch,
    };

    use super::{SimulationConfig, SimulationEngine, SimulationError};

    #[test]
    fn simulator_rejects_unsorted_actions() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 2, 1),
                branch: FocusBranch::Economy,
            }),
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 1, 1),
                branch: FocusBranch::Industry,
            }),
        ];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            GameDate::new(1936, 2, 1),
            scenario.pivot_window.start,
        );

        assert_eq!(result, Err(SimulationError::ActionsOutOfOrder));
    }

    #[test]
    fn simulator_builds_a_pre_pivot_civilian_factory() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::new(SimulationConfig {
            civilian_factory_cost_centi: 5_000,
            ..SimulationConfig::default()
        });
        let actions = [Action::Construction(ConstructionAction {
            date: GameDate::new(1936, 1, 1),
            state: France1936Scenario::ILE_DE_FRANCE,
            kind: ConstructionKind::CivilianFactory,
        })];

        let result = engine
            .simulate(
                &scenario,
                runtime,
                &actions,
                GameDate::new(1936, 1, 20),
                scenario.pivot_window.start,
            )
            .unwrap();

        assert_eq!(
            result
                .country
                .state(France1936Scenario::ILE_DE_FRANCE)
                .civilian_factories,
            9
        );
    }

    #[test]
    fn simulator_counts_same_day_duplicate_construction_entries_as_distinct_projects() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::new(SimulationConfig {
            civilian_factory_cost_centi: 5_000,
            ..SimulationConfig::default()
        });
        let date = GameDate::new(1936, 1, 1);
        let actions = [
            Action::Construction(ConstructionAction {
                date,
                state: France1936Scenario::ILE_DE_FRANCE,
                kind: ConstructionKind::CivilianFactory,
            }),
            Action::Construction(ConstructionAction {
                date,
                state: France1936Scenario::ILE_DE_FRANCE,
                kind: ConstructionKind::CivilianFactory,
            }),
        ];

        let result = engine
            .simulate(
                &scenario,
                runtime,
                &actions,
                date,
                scenario.pivot_window.start,
            )
            .unwrap();

        assert_eq!(
            result
                .country
                .state(France1936Scenario::ILE_DE_FRANCE)
                .civilian_factories,
            10
        );
    }

    #[test]
    fn simulator_rejects_pre_pivot_military_construction() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Construction(ConstructionAction {
            date: GameDate::new(1936, 1, 1),
            state: France1936Scenario::LORRAINE,
            kind: ConstructionKind::MilitaryFactory,
        })];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            GameDate::new(1936, 1, 2),
            scenario.pivot_window.start,
        );

        assert!(matches!(
            result,
            Err(SimulationError::HeuristicViolation(_))
        ));
    }

    #[test]
    fn simulator_progresses_production_lines_into_stockpile() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();

        let result = engine
            .simulate(
                &scenario,
                runtime,
                &[],
                GameDate::new(1936, 2, 1),
                scenario.pivot_window.start,
            )
            .unwrap();

        assert!(result.country.stockpile.infantry_equipment > 0);
        assert!(result.country.stockpile.artillery > 0);
    }

    #[test]
    fn simulator_requires_political_power_for_law_changes() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Law(LawAction {
            date: GameDate::new(1936, 1, 1),
            target: LawTarget::Economy(EconomyLaw::EarlyMobilization),
        })];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            GameDate::new(1936, 1, 2),
            scenario.pivot_window.start,
        );

        assert_eq!(result, Err(SimulationError::InsufficientPoliticalPower));
    }

    #[test]
    fn simulator_allows_approved_advisor_after_political_power_is_available() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        runtime.country.political_power_centi = 200 * crate::sim::POLITICAL_POWER_UNIT;

        let engine = SimulationEngine::default();
        let actions = [Action::Advisor(AdvisorAction {
            date: GameDate::new(1936, 1, 1),
            kind: AdvisorKind::IndustryConcern,
        })];

        let result = engine
            .simulate(
                &scenario,
                runtime,
                &actions,
                GameDate::new(1936, 1, 2),
                scenario.pivot_window.start,
            )
            .unwrap();

        assert!(result.country.advisors.industry);
    }

    #[test]
    fn simulator_rejects_duplicate_same_day_research_branches() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let date = GameDate::new(1936, 1, 1);
        let actions = [
            Action::Research(ResearchAction {
                date,
                slot: 0,
                branch: ResearchBranch::Construction,
            }),
            Action::Research(ResearchAction {
                date,
                slot: 1,
                branch: ResearchBranch::Construction,
            }),
        ];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            date,
            scenario.pivot_window.start,
        );

        assert_eq!(
            result,
            Err(SimulationError::DuplicateResearchBranch(
                ResearchBranch::Construction,
            ))
        );
    }

    #[test]
    fn simulator_rejects_duplicate_same_day_law_categories() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        runtime.country.political_power_centi = 500 * crate::sim::POLITICAL_POWER_UNIT;
        let engine = SimulationEngine::default();
        let date = GameDate::new(1936, 1, 1);
        let actions = [
            Action::Law(LawAction {
                date,
                target: LawTarget::Economy(EconomyLaw::EarlyMobilization),
            }),
            Action::Law(LawAction {
                date,
                target: LawTarget::Economy(EconomyLaw::PartialMobilization),
            }),
        ];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            date,
            scenario.pivot_window.start,
        );

        assert_eq!(
            result,
            Err(SimulationError::DuplicateLawCategory(LawCategory::Economy))
        );
    }

    #[test]
    fn simulator_rejects_duplicate_same_day_production_slot_changes() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let date = GameDate::new(1936, 1, 1);
        let actions = [
            Action::Production(ProductionAction {
                date,
                slot: 0,
                equipment: crate::domain::EquipmentKind::InfantryEquipment,
                factories: 10,
            }),
            Action::Production(ProductionAction {
                date,
                slot: 0,
                equipment: crate::domain::EquipmentKind::Artillery,
                factories: 8,
            }),
        ];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            date,
            scenario.pivot_window.start,
        );

        assert_eq!(result, Err(SimulationError::DuplicateProductionSlot(0)));
    }

    proptest! {
        #[test]
        fn simulator_rejects_duplicate_research_branches_for_any_branch_and_same_day(
            day_offset in 0u16..35,
            branch in prop_oneof![
                Just(ResearchBranch::Industry),
                Just(ResearchBranch::Construction),
                Just(ResearchBranch::Electronics),
                Just(ResearchBranch::Production),
            ],
        ) {
            let scenario = France1936Scenario::standard();
            let runtime = scenario.bootstrap_runtime();
            let engine = SimulationEngine::default();
            let date = GameDate::new(1936, 1, 1).add_days(day_offset);
            let actions = [
                Action::Research(ResearchAction {
                    date,
                    slot: 0,
                    branch,
                }),
                Action::Research(ResearchAction {
                    date,
                    slot: 1,
                    branch,
                }),
            ];

            let result =
                engine.simulate(&scenario, runtime, &actions, date, scenario.pivot_window.start);

            prop_assert_eq!(result, Err(SimulationError::DuplicateResearchBranch(branch)));
        }
    }
}
