use crate::domain::{
    FocusBuildingKind, FocusCondition, FocusEffect, FocusStateScope, GameDate, StateCondition,
    StateOperation,
};
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
    FocusUnavailable(Box<str>),
    UnknownFocus(Box<str>),
    UnsupportedFocusCondition(Box<str>),
    UnsupportedFocusEffect(Box<str>),
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
        debug_assert_country_invariants(&country);

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
                self.apply_action(scenario, &mut country, action.clone(), pivot_date)?;
                debug_assert_country_invariants(&country);
            }

            self.progress_focus(scenario, &mut country)?;
            self.progress_research(scenario, &mut country);
            self.advance_construction(scenario, &mut country);
            self.advance_production(scenario, &mut country);
            debug_assert_country_invariants(&country);

            if country.country.date == end {
                break;
            }

            let stability_drift_bp = country.next_daily_stability_drift_bp(&scenario.ideas);
            country.country.advance_day(
                country.political_power_daily_bonus_centi(&scenario.ideas),
                stability_drift_bp,
            );
            country.tick_active_ideas();
            debug_assert_country_invariants(&country);
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
            Action::Focus(action) => self.apply_focus_action(scenario, country, action),
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
            minimum_force_target_met: country.supported_divisions(
                scenario.force_plan.template.per_division_demand(),
                &scenario.ideas,
            ) >= scenario.force_goal.division_band().min,
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
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        action: FocusAction,
    ) -> Result<(), SimulationError> {
        if country.focus.is_some() {
            return Err(SimulationError::FocusAlreadyInProgress);
        }
        if country.has_completed_focus(&action.focus_id) {
            return Err(SimulationError::FocusUnavailable(action.focus_id));
        }

        let focus = scenario
            .focus_by_id(&action.focus_id)
            .ok_or_else(|| SimulationError::UnknownFocus(action.focus_id.clone()))?;
        if !self.focus_is_available(country, &scenario.ideas, focus)? {
            return Err(SimulationError::FocusUnavailable(action.focus_id));
        }
        if self.evaluate_focus_condition(country, &scenario.ideas, &focus.bypass)? {
            country.record_focus_completion(action.focus_id);
            return Ok(());
        }

        country.focus = Some(FocusProgress {
            focus_id: action.focus_id,
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

    fn progress_focus(
        &self,
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
    ) -> Result<(), SimulationError> {
        let Some(mut focus) = country.focus.take() else {
            return Ok(());
        };
        let definition = scenario
            .focus_by_id(&focus.focus_id)
            .ok_or_else(|| SimulationError::UnknownFocus(focus.focus_id.clone()))?;

        focus.days_progress += 1;
        if focus.days_progress >= definition.days {
            let effects = definition.effects.clone();
            let focus_id = focus.focus_id.clone();
            self.apply_focus_effects(scenario, country, &effects, None)?;
            country.record_focus_completion(focus_id);
            return Ok(());
        }

        country.focus = Some(focus);
        Ok(())
    }

    fn progress_research(&self, scenario: &France1936Scenario, country: &mut CountryRuntime) {
        for slot_index in 0..country.research_slots.len() {
            let Some(branch) = country.research_slots[slot_index].branch else {
                continue;
            };

            country.research_slots[slot_index].days_progress += 1;
            if country.research_slots[slot_index].days_progress
                >= self.research_days(branch, country.research_speed_bp(&scenario.ideas))
            {
                country.apply_research_completion(branch);
                country.research_slots[slot_index] = super::state::ResearchSlotState::default();
            }
        }
    }

    fn advance_construction(&self, scenario: &France1936Scenario, country: &mut CountryRuntime) {
        let available_civs = usize::from(country.available_civilian_factories(&scenario.ideas));
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
            let construction_speed_bp = u32::from(country.construction_speed_bp_for(
                self.focus_building_kind(country.construction_queue[index].kind),
                &scenario.ideas,
            ));
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

    fn advance_production(&self, scenario: &France1936Scenario, country: &mut CountryRuntime) {
        let output_bonus_bp = u32::from(country.military_output_bp(&scenario.ideas));

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

    pub(crate) fn focus_is_available(
        &self,
        country: &CountryRuntime,
        ideas: &[crate::domain::IdeaDefinition],
        focus: &crate::domain::NationalFocus,
    ) -> Result<bool, SimulationError> {
        if focus
            .prerequisites
            .iter()
            .any(|prerequisite| !country.has_completed_focus(prerequisite))
        {
            return Ok(false);
        }
        if focus
            .mutually_exclusive
            .iter()
            .any(|blocked| country.has_completed_focus(blocked))
        {
            return Ok(false);
        }

        self.evaluate_focus_condition(country, ideas, &focus.available)
    }

    fn evaluate_focus_condition(
        &self,
        country: &CountryRuntime,
        ideas: &[crate::domain::IdeaDefinition],
        condition: &FocusCondition,
    ) -> Result<bool, SimulationError> {
        match condition {
            FocusCondition::Always => Ok(true),
            FocusCondition::All(conditions) => {
                for condition in conditions {
                    if !self.evaluate_focus_condition(country, ideas, condition)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            FocusCondition::Any(conditions) => {
                for condition in conditions {
                    if self.evaluate_focus_condition(country, ideas, condition)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            FocusCondition::Not(condition) => {
                Ok(!self.evaluate_focus_condition(country, ideas, condition)?)
            }
            FocusCondition::HasCompletedFocus(id) => Ok(country.has_completed_focus(id)),
            FocusCondition::HasCountryFlag(flag) => Ok(country.has_country_flag(flag)),
            FocusCondition::HasDlc(_) => Ok(false),
            FocusCondition::HasGameRule { .. } => Ok(false),
            FocusCondition::HasIdea(id) => Ok(country.has_idea(id)),
            FocusCondition::HasWarSupportAtLeast(value) => {
                Ok(country.current_war_support_bp(ideas) >= *value)
            }
            FocusCondition::NumOfFactoriesAtLeast(value) => Ok(country.total_civilian_factories()
                + country.total_military_factories()
                >= *value),
            FocusCondition::NumOfMilitaryFactoriesAtLeast(value) => {
                Ok(country.total_military_factories() >= *value)
            }
            FocusCondition::AmountResearchSlotsGreaterThan(value) => {
                Ok(country.research_slots.len() > usize::from(*value))
            }
            FocusCondition::AmountResearchSlotsLessThan(value) => {
                Ok(country.research_slots.len() < usize::from(*value))
            }
            FocusCondition::AnyControlledState(limit)
            | FocusCondition::AnyOwnedState(limit)
            | FocusCondition::AnyState(limit) => {
                for index in 0..country.states.len() {
                    if self.evaluate_state_condition(country, index, limit)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            FocusCondition::Unsupported(name) => {
                Err(SimulationError::UnsupportedFocusCondition(name.clone()))
            }
        }
    }

    fn evaluate_state_condition(
        &self,
        country: &CountryRuntime,
        state_index: usize,
        condition: &StateCondition,
    ) -> Result<bool, SimulationError> {
        match condition {
            StateCondition::Always => Ok(true),
            StateCondition::All(conditions) => {
                for condition in conditions {
                    if !self.evaluate_state_condition(country, state_index, condition)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            StateCondition::Any(conditions) => {
                for condition in conditions {
                    if self.evaluate_state_condition(country, state_index, condition)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            StateCondition::Not(condition) => {
                Ok(!self.evaluate_state_condition(country, state_index, condition)?)
            }
            StateCondition::RawStateId(raw_state_id) => {
                Ok(country.state_defs[state_index].raw_state_id == *raw_state_id)
            }
            StateCondition::IsControlledByRoot | StateCondition::IsOwnedByRoot => Ok(true),
            StateCondition::IsCoreOfRoot => Ok(country.state_defs[state_index].is_core_of_root),
            StateCondition::OwnerIsRootOrSubject => Ok(true),
            StateCondition::HasStateFlag(flag) => {
                Ok(country.has_state_flag_by_index(state_index, flag))
            }
            StateCondition::InfrastructureLessThan(value) => {
                Ok(country.states[state_index].infrastructure < *value)
            }
            StateCondition::FreeSharedBuildingSlotsGreaterThan(value) => Ok(country.states
                [state_index]
                .free_slots(&country.state_defs[state_index])
                > *value),
            StateCondition::Unsupported(name) => {
                Err(SimulationError::UnsupportedFocusCondition(name.clone()))
            }
        }
    }

    fn apply_focus_effects(
        &self,
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        effects: &[FocusEffect],
        anchor_state: Option<usize>,
    ) -> Result<(), SimulationError> {
        for effect in effects {
            self.apply_focus_effect(scenario, country, effect, anchor_state)?;
        }

        Ok(())
    }

    fn apply_focus_effect(
        &self,
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        effect: &FocusEffect,
        anchor_state: Option<usize>,
    ) -> Result<(), SimulationError> {
        match effect {
            FocusEffect::AddIdea(id) => {
                if scenario.idea_by_id(id).is_none() {
                    return Err(SimulationError::UnsupportedFocusEffect(id.clone()));
                }
                country.add_idea(id.clone(), None);
            }
            FocusEffect::RemoveIdea(id) => {
                country.remove_idea(id);
            }
            FocusEffect::AddTimedIdea { id, days } => {
                if scenario.idea_by_id(id).is_none() {
                    return Err(SimulationError::UnsupportedFocusEffect(id.clone()));
                }
                country.add_idea(id.clone(), Some(*days));
            }
            FocusEffect::SwapIdea { remove, add } => {
                if scenario.idea_by_id(add).is_none() {
                    return Err(SimulationError::UnsupportedFocusEffect(add.clone()));
                }
                country.remove_idea(remove);
                country.add_idea(add.clone(), None);
            }
            FocusEffect::AddArmyExperience(amount) => {
                country.army_experience = country.army_experience.saturating_add(*amount);
            }
            FocusEffect::AddDoctrineCostReduction(reduction) => {
                country.add_doctrine_cost_reduction(reduction.clone());
            }
            FocusEffect::AddCountryLeaderTrait(trait_id) => {
                country.add_country_leader_trait(trait_id.clone());
            }
            FocusEffect::AddPoliticalPower(amount) => {
                country.country.political_power_centi = country
                    .country
                    .political_power_centi
                    .saturating_add(*amount);
            }
            FocusEffect::AddStability(amount) => {
                country.country.stability_bp = country.country.stability_bp.saturating_add(*amount);
            }
            FocusEffect::AddWarSupport(amount) => {
                country.country.war_support_bp =
                    country.country.war_support_bp.saturating_add(*amount);
            }
            FocusEffect::AddManpower(amount) => {
                country.country.population = country.country.population.saturating_add(*amount);
            }
            FocusEffect::AddResearchSlot(amount) => {
                for _ in 0..*amount {
                    country
                        .research_slots
                        .push(super::state::ResearchSlotState::default());
                }
            }
            FocusEffect::SetCountryFlag(flag) => {
                country.set_country_flag(flag.clone());
            }
            FocusEffect::AddEquipmentToStockpile { equipment, amount } => {
                country.stockpile.add(*equipment, *amount);
            }
            FocusEffect::StateScoped(scope_effects) => {
                let indices = self.select_focus_state_indices(
                    country,
                    scope_effects.scope,
                    &scope_effects.limit,
                    anchor_state,
                )?;
                for index in indices {
                    for operation in &scope_effects.operations {
                        self.apply_focus_state_operation(country, index, operation)?;
                    }
                }
            }
            FocusEffect::Unsupported(name) => {
                return Err(SimulationError::UnsupportedFocusEffect(name.clone()));
            }
        }

        Ok(())
    }

    fn select_focus_state_indices(
        &self,
        country: &CountryRuntime,
        scope: FocusStateScope,
        limit: &StateCondition,
        anchor_state: Option<usize>,
    ) -> Result<Vec<usize>, SimulationError> {
        let mut eligible = Vec::new();
        for index in 0..country.states.len() {
            if self.evaluate_state_condition(country, index, limit)? {
                eligible.push(index);
            }
        }
        eligible.sort_by_key(|index| {
            (
                country.state_defs[*index].frontier.is_some(),
                std::cmp::Reverse(country.state_defs[*index].economic_weight),
                country.state_defs[*index].raw_state_id,
            )
        });

        Ok(match scope {
            FocusStateScope::AnyState | FocusStateScope::EveryOwnedState => eligible,
            FocusStateScope::RandomControlledState | FocusStateScope::RandomOwnedState => {
                eligible.into_iter().take(1).collect()
            }
            FocusStateScope::RandomNeighborState => eligible
                .iter()
                .copied()
                .find(|index| Some(*index) != anchor_state)
                .into_iter()
                .collect(),
        })
    }

    fn apply_focus_state_operation(
        &self,
        country: &mut CountryRuntime,
        state_index: usize,
        operation: &StateOperation,
    ) -> Result<(), SimulationError> {
        match operation {
            StateOperation::AddExtraSharedBuildingSlots(amount) => {
                country.state_defs[state_index].building_slots = country.state_defs[state_index]
                    .building_slots
                    .saturating_add(*amount);
            }
            StateOperation::SetStateFlag(flag) => {
                country.set_state_flag_by_index(state_index, flag.clone());
            }
            StateOperation::NestedScope(scope) => {
                let indices = self.select_focus_state_indices(
                    country,
                    scope.scope,
                    &scope.limit,
                    Some(state_index),
                )?;
                for nested_index in indices {
                    for operation in &scope.operations {
                        self.apply_focus_state_operation(country, nested_index, operation)?;
                    }
                }
            }
            StateOperation::AddBuildingConstruction {
                kind,
                level,
                instant,
            } => {
                if !instant {
                    return Err(SimulationError::UnsupportedFocusEffect(
                        "non-instant focus construction".into(),
                    ));
                }
                for _ in 0..*level {
                    self.finish_focus_building(country, state_index, *kind);
                }
            }
        }

        Ok(())
    }

    fn finish_focus_building(
        &self,
        country: &mut CountryRuntime,
        state_index: usize,
        kind: FocusBuildingKind,
    ) {
        let definition = &country.state_defs[state_index];
        if matches!(
            kind,
            FocusBuildingKind::CivilianFactory | FocusBuildingKind::MilitaryFactory
        ) && country.states[state_index].free_slots(definition) == 0
        {
            return;
        }

        match kind {
            FocusBuildingKind::CivilianFactory => {
                country.states[state_index].civilian_factories += 1
            }
            FocusBuildingKind::MilitaryFactory => {
                country.states[state_index].military_factories += 1
            }
            FocusBuildingKind::Infrastructure => country.states[state_index].infrastructure += 1,
            FocusBuildingKind::LandFort => country.states[state_index].land_fort_level += 1,
        }
    }

    fn research_days(&self, branch: super::actions::ResearchBranch, research_speed_bp: u16) -> u16 {
        let base_days = match branch {
            super::actions::ResearchBranch::Industry => 140_u16,
            super::actions::ResearchBranch::Construction => 120_u16,
            super::actions::ResearchBranch::Electronics => 150_u16,
            super::actions::ResearchBranch::Production => 130_u16,
        };
        let speed_bp = u32::from(10_000_u16.saturating_add(research_speed_bp));
        u16::try_from((u32::from(base_days) * 10_000).div_ceil(speed_bp)).unwrap_or(u16::MAX)
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

    fn focus_building_kind(&self, kind: ConstructionKind) -> FocusBuildingKind {
        match kind {
            ConstructionKind::CivilianFactory => FocusBuildingKind::CivilianFactory,
            ConstructionKind::MilitaryFactory => FocusBuildingKind::MilitaryFactory,
            ConstructionKind::Infrastructure => FocusBuildingKind::Infrastructure,
            ConstructionKind::LandFort => FocusBuildingKind::LandFort,
        }
    }
}

#[inline]
fn debug_assert_country_invariants(country: &CountryRuntime) {
    #[cfg(debug_assertions)]
    country.assert_invariants();
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::domain::{
        DoctrineCostReduction, EconomyLaw, EquipmentKind, FocusBuildingKind, FocusCondition,
        FocusEffect, FocusStateScope, GameDate, IdeaDefinition, IdeaModifiers, MobilizationLaw,
        NationalFocus, StateCondition, StateOperation, StateScopedEffects, TradeLaw,
    };
    use crate::scenario::France1936Scenario;
    use crate::sim::{
        Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, FocusAction,
        LawAction, LawCategory, LawTarget, ProductionAction, ResearchAction, ResearchBranch,
        StateId,
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
                focus_id: "FRA_unsorted_a".into(),
            }),
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 1, 1),
                focus_id: "FRA_unsorted_b".into(),
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

    #[test]
    fn simulator_applies_exact_focus_rewards_and_timed_ideas() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![
                NationalFocus {
                    id: "FRA_devalue_the_franc".into(),
                    days: 1,
                    prerequisites: Vec::new(),
                    mutually_exclusive: Vec::new(),
                    available: FocusCondition::Always,
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                    effects: vec![FocusEffect::AddTimedIdea {
                        id: "FRA_devalued_currency".into(),
                        days: 5,
                    }],
                },
                NationalFocus {
                    id: "FRA_begin_rearmament".into(),
                    days: 1,
                    prerequisites: vec!["FRA_devalue_the_franc".into()],
                    mutually_exclusive: Vec::new(),
                    available: FocusCondition::AmountResearchSlotsLessThan(5),
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                    effects: vec![
                        FocusEffect::AddResearchSlot(1),
                        FocusEffect::StateScoped(StateScopedEffects {
                            scope: FocusStateScope::RandomOwnedState,
                            limit: StateCondition::IsCoreOfRoot,
                            operations: vec![
                                StateOperation::AddExtraSharedBuildingSlots(1),
                                StateOperation::AddBuildingConstruction {
                                    kind: FocusBuildingKind::MilitaryFactory,
                                    level: 1,
                                    instant: true,
                                },
                                StateOperation::SetStateFlag("FRA_rearmed".into()),
                            ],
                        }),
                    ],
                },
            ],
            vec![IdeaDefinition {
                id: "FRA_devalued_currency".into(),
                modifiers: IdeaModifiers {
                    consumer_goods_bp: -1_000,
                    ..IdeaModifiers::default()
                },
            }],
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 1, 1),
                focus_id: "FRA_devalue_the_franc".into(),
            }),
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 1, 2),
                focus_id: "FRA_begin_rearmament".into(),
            }),
        ];

        let result = engine
            .simulate(
                &scenario,
                runtime,
                &actions,
                GameDate::new(1936, 1, 2),
                scenario.pivot_window.start,
            )
            .unwrap();

        assert!(result.country.has_idea("FRA_devalued_currency"));
        assert_eq!(result.country.active_ideas[0].remaining_days, Some(4));
        assert_eq!(result.country.research_slots.len(), 3);
        assert_eq!(
            result
                .country
                .state(France1936Scenario::ILE_DE_FRANCE)
                .military_factories,
            scenario
                .bootstrap_runtime()
                .state(France1936Scenario::ILE_DE_FRANCE)
                .military_factories
                + 1
        );
        assert_eq!(
            result
                .country
                .state_def(France1936Scenario::ILE_DE_FRANCE)
                .building_slots,
            scenario
                .bootstrap_runtime()
                .state_def(France1936Scenario::ILE_DE_FRANCE)
                .building_slots
                + 1
        );
        assert!(
            result.country.state_flags[usize::from(France1936Scenario::ILE_DE_FRANCE.0)]
                .iter()
                .any(|flag| flag.as_ref() == "FRA_rearmed")
        );
        assert!(result.country.has_completed_focus("FRA_begin_rearmament"));
    }

    #[test]
    fn simulator_tracks_remove_idea_and_doctrine_side_effects() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            vec!["FRA_victors_of_wwi".into()],
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_army_reform".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_RESEARCH".into()],
                effects: vec![
                    FocusEffect::RemoveIdea("FRA_victors_of_wwi".into()),
                    FocusEffect::AddArmyExperience(10),
                    FocusEffect::AddDoctrineCostReduction(DoctrineCostReduction {
                        name: "FRA_army_reform".into(),
                        category: "land_doctrine".into(),
                        cost_reduction_bp: 5_000,
                        uses: 2,
                    }),
                    FocusEffect::AddCountryLeaderTrait("tenacious_negotiator".into()),
                ],
            }],
            vec![IdeaDefinition {
                id: "FRA_victors_of_wwi".into(),
                modifiers: IdeaModifiers {
                    research_speed_bp: -1_000,
                    ..IdeaModifiers::default()
                },
            }],
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Focus(FocusAction {
            date: GameDate::new(1936, 1, 1),
            focus_id: "FRA_army_reform".into(),
        })];

        let result = engine
            .simulate(
                &scenario,
                runtime,
                &actions,
                GameDate::new(1936, 1, 1),
                scenario.pivot_window.start,
            )
            .unwrap();

        assert!(!result.country.has_idea("FRA_victors_of_wwi"));
        assert_eq!(result.country.army_experience, 10);
        assert_eq!(
            result.country.doctrine_cost_reductions,
            vec![DoctrineCostReduction {
                name: "FRA_army_reform".into(),
                category: "land_doctrine".into(),
                cost_reduction_bp: 5_000,
                uses: 2,
            }]
        );
        assert_eq!(
            result.country.country_leader_traits,
            vec![Box::<str>::from("tenacious_negotiator")]
        );
    }

    fn fuzz_scenario() -> France1936Scenario {
        France1936Scenario::standard().with_exact_focus_data(
            3,
            vec!["FRA_victors_of_wwi".into()],
            Vec::new(),
            vec![
                NationalFocus {
                    id: "FRA_devalue_the_franc".into(),
                    days: 1,
                    prerequisites: Vec::new(),
                    mutually_exclusive: Vec::new(),
                    available: FocusCondition::Always,
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                    effects: vec![FocusEffect::AddTimedIdea {
                        id: "FRA_devalued_currency".into(),
                        days: 14,
                    }],
                },
                NationalFocus {
                    id: "FRA_begin_rearmament".into(),
                    days: 1,
                    prerequisites: vec!["FRA_devalue_the_franc".into()],
                    mutually_exclusive: Vec::new(),
                    available: FocusCondition::Always,
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                    effects: vec![
                        FocusEffect::AddResearchSlot(1),
                        FocusEffect::RemoveIdea("FRA_victors_of_wwi".into()),
                    ],
                },
                NationalFocus {
                    id: "FRA_army_reform".into(),
                    days: 1,
                    prerequisites: vec!["FRA_begin_rearmament".into()],
                    mutually_exclusive: Vec::new(),
                    available: FocusCondition::Always,
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_RESEARCH".into()],
                    effects: vec![
                        FocusEffect::AddArmyExperience(5),
                        FocusEffect::AddDoctrineCostReduction(DoctrineCostReduction {
                            name: "FRA_army_reform".into(),
                            category: "land_doctrine".into(),
                            cost_reduction_bp: 5_000,
                            uses: 2,
                        }),
                    ],
                },
            ],
            vec![
                IdeaDefinition {
                    id: "FRA_victors_of_wwi".into(),
                    modifiers: IdeaModifiers {
                        research_speed_bp: -500,
                        ..IdeaModifiers::default()
                    },
                },
                IdeaDefinition {
                    id: "FRA_devalued_currency".into(),
                    modifiers: IdeaModifiers {
                        consumer_goods_bp: -1_000,
                        ..IdeaModifiers::default()
                    },
                },
            ],
            Vec::new(),
        )
    }

    fn generated_action_from_spec(
        scenario: &France1936Scenario,
        spec: (u8, u8, u8, u8, u8),
    ) -> Action {
        let (kind, day, a, b, c) = spec;
        let date = scenario.start_date.add_days(u16::from(day % 90));

        match kind % 6 {
            0 => {
                let focus = &scenario.focuses[usize::from(a) % scenario.focuses.len()];
                Action::Focus(FocusAction {
                    date,
                    focus_id: focus.id.clone(),
                })
            }
            1 => Action::Research(ResearchAction {
                date,
                slot: a % 4,
                branch: match b % 4 {
                    0 => ResearchBranch::Industry,
                    1 => ResearchBranch::Construction,
                    2 => ResearchBranch::Electronics,
                    _ => ResearchBranch::Production,
                },
            }),
            2 => Action::Law(LawAction {
                date,
                target: match a % 7 {
                    0 => LawTarget::Economy(EconomyLaw::CivilianEconomy),
                    1 => LawTarget::Economy(EconomyLaw::EarlyMobilization),
                    2 => LawTarget::Economy(EconomyLaw::PartialMobilization),
                    3 => LawTarget::Economy(EconomyLaw::WarEconomy),
                    4 => LawTarget::Trade(TradeLaw::ExportFocus),
                    5 => LawTarget::Trade(TradeLaw::LimitedExports),
                    _ => LawTarget::Mobilization(match b % 3 {
                        0 => MobilizationLaw::VolunteerOnly,
                        1 => MobilizationLaw::LimitedConscription,
                        _ => MobilizationLaw::ExtensiveConscription,
                    }),
                },
            }),
            3 => Action::Construction(ConstructionAction {
                date,
                state: StateId(a % 12),
                kind: match b % 4 {
                    0 => ConstructionKind::CivilianFactory,
                    1 => ConstructionKind::MilitaryFactory,
                    2 => ConstructionKind::Infrastructure,
                    _ => ConstructionKind::LandFort,
                },
            }),
            4 => Action::Production(ProductionAction {
                date,
                slot: a % 6,
                equipment: match b % 5 {
                    0 => EquipmentKind::InfantryEquipment,
                    1 => EquipmentKind::SupportEquipment,
                    2 => EquipmentKind::Artillery,
                    3 => EquipmentKind::AntiTank,
                    _ => EquipmentKind::AntiAir,
                },
                factories: c % 12,
            }),
            _ => Action::Advisor(AdvisorAction {
                date,
                kind: match a % 3 {
                    0 => AdvisorKind::IndustryConcern,
                    1 => AdvisorKind::ResearchInstitute,
                    _ => AdvisorKind::MilitaryIndustrialist,
                },
            }),
        }
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

        #[test]
        fn simulator_no_action_runs_preserve_invariants_and_monotone_stockpile(
            day_offset in 0u16..120,
        ) {
            let scenario = France1936Scenario::standard();
            let runtime = scenario.bootstrap_runtime();
            let initial_stockpile = runtime.stockpile;
            let initial_political_power = runtime.country.political_power_centi;
            let end = scenario.start_date.add_days(day_offset);
            let engine = SimulationEngine::default();

            let outcome = engine
                .simulate(&scenario, runtime, &[], end, scenario.pivot_window.start)
                .unwrap();

            outcome.country.assert_invariants();
            prop_assert_eq!(outcome.country.country.date, end);
            prop_assert!(outcome.country.country.political_power_centi >= initial_political_power);
            prop_assert!(outcome.country.stockpile.infantry_equipment >= initial_stockpile.infantry_equipment);
            prop_assert!(outcome.country.stockpile.support_equipment >= initial_stockpile.support_equipment);
            prop_assert!(outcome.country.stockpile.artillery >= initial_stockpile.artillery);
            prop_assert!(outcome.country.stockpile.anti_tank >= initial_stockpile.anti_tank);
            prop_assert!(outcome.country.stockpile.anti_air >= initial_stockpile.anti_air);
        }

        #[test]
        fn simulator_generated_action_sequences_preserve_runtime_invariants(
            specs in prop::collection::vec((0u8..6, 0u8..120, 0u8..16, 0u8..16, 0u8..16), 0..24),
        ) {
            let scenario = fuzz_scenario();
            let mut actions: Vec<_> = specs
                .into_iter()
                .map(|spec| generated_action_from_spec(&scenario, spec))
                .collect();
            actions.sort_by_key(Action::date);

            let runtime = scenario.bootstrap_runtime();
            runtime.assert_invariants();
            let end = actions
                .last()
                .map(Action::date)
                .unwrap_or(scenario.start_date);
            let engine = SimulationEngine::default();

            let result = engine.simulate(
                &scenario,
                runtime,
                &actions,
                end,
                scenario.pivot_window.start,
            );

            if let Ok(outcome) = result {
                outcome.country.assert_invariants();
            }
        }
    }
}
