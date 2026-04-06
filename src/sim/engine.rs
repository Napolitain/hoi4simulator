use crate::domain::{
    EquipmentDemand, EquipmentFactoryAllocation, EquipmentKind, FocusBuildingKind, FocusCondition,
    FocusEffect, FocusStateScope, GameDate, StateCondition, StateOperation,
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
    ConstructionCapReached(super::actions::StateId, ConstructionKind),
    FocusAlreadyInProgress,
    FocusUnavailable(Box<str>),
    UnknownFocus(Box<str>),
    UnsupportedFocusCondition(Box<str>),
    UnsupportedFocusEffect(Box<str>),
    InsufficientPoliticalPower,
    ResearchSlotBusy(u8),
    InvalidResearchSlot(u8),
    ResearchUnavailable(ResearchBranch),
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
            // Resolve start-of-day world state before any same-day action gating.
            country.apply_timeline_events(&scenario.timeline_events);
            country.prune_expired_country_flags();
            debug_assert_country_invariants(&country);

            let day_start = action_index;
            while action_index < actions.len()
                && actions[action_index].date() == country.country.date
            {
                action_index += 1;
            }

            self.validate_same_day_actions(&country, &actions[day_start..action_index])?;

            for action in &actions[day_start..action_index] {
                self.apply_action(scenario, &mut country, action, pivot_date)?;
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
        action: &Action,
        pivot_date: GameDate,
    ) -> Result<(), SimulationError> {
        match action {
            Action::Construction(action) => {
                self.apply_construction_action(scenario, country, *action, pivot_date)
            }
            Action::Production(action) => self.apply_production_action(scenario, country, *action),
            Action::Focus(action) => self.apply_focus_action(scenario, country, action),
            Action::Law(action) => self.apply_law_action(country, *action, pivot_date),
            Action::Advisor(action) => self.apply_advisor_action(country, *action, pivot_date),
            Action::Research(action) => self.apply_research_action(scenario, country, *action),
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
        let minimum_force_target_met = country.supported_divisions(
            scenario.force_plan.template.per_division_demand(),
            &scenario.ideas,
        ) >= scenario.force_goal.fort_construction_division_floor();
        let context = ConstructionDecisionContext {
            phase,
            military_factory_target_met: country.total_military_factories()
                >= scenario.force_plan.required_military_factories,
            minimum_force_target_met,
            frontier_forts_met: country.frontier_forts_complete(&scenario.frontier_forts),
            frontier_fort_priority: country.country.date >= scenario.milestones[2].date
                || self.military_base_covers_readiness_shortfall(scenario, country),
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
        if matches!(action.kind, ConstructionKind::Infrastructure)
            && runtime.infrastructure.saturating_add(
                country.queued_kind_projects(action.state, ConstructionKind::Infrastructure),
            ) >= 10
        {
            return Err(SimulationError::ConstructionCapReached(
                action.state,
                action.kind,
            ));
        }
        if matches!(action.kind, ConstructionKind::LandFort)
            && runtime.land_fort_level.saturating_add(
                country.queued_kind_projects(action.state, ConstructionKind::LandFort),
            ) >= 10
        {
            return Err(SimulationError::ConstructionCapReached(
                action.state,
                action.kind,
            ));
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
        action: &FocusAction,
    ) -> Result<(), SimulationError> {
        if country.focus.is_some() {
            return Err(SimulationError::FocusAlreadyInProgress);
        }
        if country.has_completed_focus(&action.focus_id) {
            return Err(SimulationError::FocusUnavailable(action.focus_id.clone()));
        }

        let focus = scenario
            .focus_by_id(&action.focus_id)
            .ok_or_else(|| SimulationError::UnknownFocus(action.focus_id.clone()))?;
        if !self.focus_is_available(country, scenario.reference_tag, &scenario.ideas, focus)? {
            return Err(SimulationError::FocusUnavailable(action.focus_id.clone()));
        }
        if self.evaluate_focus_condition(
            country,
            scenario.reference_tag,
            &scenario.ideas,
            &focus.bypass,
        )? {
            country.record_focus_completion(action.focus_id.clone());
            return Ok(());
        }

        country.focus = Some(FocusProgress {
            focus_id: action.focus_id.clone(),
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
        scenario: &France1936Scenario,
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

        let technology = if scenario.technology_tree.is_empty() {
            None
        } else {
            self.select_next_technology(scenario, country, action.branch)
                .ok_or(SimulationError::ResearchUnavailable(action.branch))
                .map(Some)?
        };

        country.research_slots[slot_index].branch = Some(action.branch);
        country.research_slots[slot_index].technology = technology;
        country.research_slots[slot_index].progress_centi = 0;

        Ok(())
    }

    fn select_next_technology(
        &self,
        scenario: &France1936Scenario,
        country: &CountryRuntime,
        branch: ResearchBranch,
    ) -> Option<crate::domain::TechId> {
        if scenario.technology_tree.is_empty()
            || scenario.technology_tree.len() != country.completed_technologies.len()
        {
            return None;
        }

        let mut reserved = vec![false; scenario.technology_tree.len()];
        for tech_id in country
            .research_slots
            .iter()
            .filter_map(|slot| slot.technology)
        {
            if tech_id.index() < reserved.len() {
                reserved[tech_id.index()] = true;
            }
        }

        let mut best = None::<(u32, u16, u16, u16)>;
        for node in scenario.technology_tree.nodes().iter() {
            if node.branch != branch
                || country.completed_technologies[node.id.index()]
                || reserved[node.id.index()]
                || !node
                    .prerequisites
                    .iter()
                    .all(|prerequisite| country.completed_technologies[prerequisite.index()])
                || !node.exclusive_with.iter().all(|exclusive| {
                    !country.completed_technologies[exclusive.index()]
                        && !reserved[exclusive.index()]
                })
            {
                continue;
            }

            let candidate = (
                country.technology_bonus_bp(node),
                u16::MAX - node.start_year,
                u16::MAX - node.id.0,
                node.id.0,
            );
            if best.as_ref().is_none_or(|current| &candidate > current) {
                best = Some(candidate);
            }
        }

        best.map(|(_, _, _, id)| crate::domain::TechId(id))
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
        debug_assert!(slot_index < country.production_lines.len());

        let unit_cost_centi = country
            .equipment_profiles
            .profile(action.equipment)
            .unit_cost_centi;
        let efficiency_floor_permille = country.production_efficiency_floor_permille();
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

        line.reassign(
            action.equipment,
            action.factories,
            unit_cost_centi,
            efficiency_floor_permille,
        );

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
            let required_progress_centi =
                if let Some(technology) = country.research_slots[slot_index].technology {
                    u32::from(scenario.technology_tree.node(technology).base_days) * 10_000
                } else {
                    u32::from(self.base_research_days(branch)) * 10_000
                };
            let daily_progress_centi = self.daily_research_progress_centi(
                scenario,
                country,
                country.research_slots[slot_index].technology,
                branch,
            );

            country.research_slots[slot_index].progress_centi = country.research_slots[slot_index]
                .progress_centi
                .saturating_add(daily_progress_centi);
            if country.research_slots[slot_index].progress_centi >= required_progress_centi {
                if let Some(technology) = country.research_slots[slot_index].technology {
                    let node = scenario.technology_tree.node(technology).clone();
                    country.apply_technology_completion(&node);
                } else {
                    country.apply_research_completion(branch);
                }
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
            let remaining_cost = country.construction_queue[index]
                .total_cost_centi
                .saturating_sub(country.construction_queue[index].progress_centi);
            let daily_progress = self
                .construction_daily_progress_centi(
                    assigned_civs,
                    infrastructure_multiplier_bp,
                    construction_speed_bp,
                )
                .min(remaining_cost);

            country.construction_queue[index].progress_centi = country.construction_queue[index]
                .progress_centi
                .saturating_add(daily_progress);
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
            ConstructionKind::Infrastructure => {
                state.infrastructure = state.infrastructure.saturating_add(1).min(10)
            }
            ConstructionKind::LandFort => {
                state.land_fort_level = state.land_fort_level.saturating_add(1).min(10)
            }
        }
    }

    fn advance_production(&self, scenario: &France1936Scenario, country: &mut CountryRuntime) {
        let output_bonus_bp = u32::from(country.military_output_bp(&scenario.ideas));
        let mut available_resources = country.domestic_resources(&scenario.ideas);
        let efficiency_cap_permille = country
            .production_efficiency_cap_permille(self.config.production_efficiency_cap_permille);
        let efficiency_gain_permille = country
            .production_efficiency_gain_permille(self.config.production_efficiency_gain_permille);

        for line in &mut country.production_lines {
            if line.factories == 0 {
                continue;
            }

            let resource_demand = line.daily_resource_demand(country.equipment_profiles);
            let resource_fulfillment_bp = resource_demand.fulfillment_bp(available_resources);
            let consumed_resources = resource_demand.scale_bp(resource_fulfillment_bp);
            let prior_efficiency = line.efficiency_permille;
            let daily_ic_centi = self.production_daily_ic_centi(
                line.factories,
                line.efficiency_permille,
                output_bonus_bp,
            );
            let daily_ic_centi = self.scale_by_bp(daily_ic_centi, resource_fulfillment_bp);

            line.accumulated_ic_centi = line.accumulated_ic_centi.saturating_add(daily_ic_centi);
            available_resources = available_resources.saturating_sub(consumed_resources);

            let produced_units = line.accumulated_ic_centi / line.unit_cost_centi;
            line.accumulated_ic_centi %= line.unit_cost_centi;

            country.stockpile.add(line.equipment, produced_units);

            if line.efficiency_permille < efficiency_cap_permille {
                line.efficiency_permille = (line.efficiency_permille + efficiency_gain_permille)
                    .min(efficiency_cap_permille);
            }

            debug_assert!(line.efficiency_permille >= prior_efficiency);
            debug_assert!(line.efficiency_permille <= efficiency_cap_permille);
        }
    }

    pub(crate) fn focus_is_available(
        &self,
        country: &CountryRuntime,
        root_tag: &str,
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

        self.evaluate_focus_condition(country, root_tag, ideas, &focus.available)
    }

    fn evaluate_focus_condition(
        &self,
        country: &CountryRuntime,
        root_tag: &str,
        ideas: &[crate::domain::IdeaDefinition],
        condition: &FocusCondition,
    ) -> Result<bool, SimulationError> {
        match condition {
            FocusCondition::Always => Ok(true),
            FocusCondition::All(conditions) => {
                for condition in conditions {
                    if !self.evaluate_focus_condition(country, root_tag, ideas, condition)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            FocusCondition::Any(conditions) => {
                for condition in conditions {
                    if self.evaluate_focus_condition(country, root_tag, ideas, condition)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            FocusCondition::Not(condition) => {
                Ok(!self.evaluate_focus_condition(country, root_tag, ideas, condition)?)
            }
            FocusCondition::HasCompletedFocus(id) => Ok(country.has_completed_focus(id)),
            FocusCondition::HasCountryFlag(flag) => Ok(country.has_country_flag(flag)),
            FocusCondition::HasDlc(id) => Ok(country.has_dlc(id)),
            FocusCondition::HasGameRule { .. } => Ok(false),
            FocusCondition::HasGovernment(government) => {
                Ok(country.country.government == *government)
            }
            FocusCondition::HasIdea(id) => Ok(country.has_idea(id)),
            FocusCondition::IsInFaction(expected) => Ok(country.in_faction() == *expected),
            FocusCondition::IsPuppet(expected) | FocusCondition::IsSubject(expected) => {
                Ok(country.is_subject() == *expected)
            }
            FocusCondition::OriginalTag(tag) => Ok(country.original_tag.as_ref() == tag.as_ref()),
            FocusCondition::Timeline(condition) => {
                Ok(condition.evaluate(country.country.date, &country.world_state, root_tag))
            }
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
            FocusEffect::AddIdea(_)
            | FocusEffect::RemoveIdea(_)
            | FocusEffect::AddTimedIdea { .. }
            | FocusEffect::SwapIdea { .. } => {
                Self::apply_focus_idea_effect(scenario, country, effect)?;
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
                let stability_bp =
                    u32::from(country.country.stability_bp).saturating_add(u32::from(*amount));
                country.country.stability_bp =
                    u16::try_from(stability_bp.min(10_000)).unwrap_or(10_000);
            }
            FocusEffect::AddWarSupport(amount) => {
                let war_support_bp =
                    u32::from(country.country.war_support_bp).saturating_add(u32::from(*amount));
                country.country.war_support_bp =
                    u16::try_from(war_support_bp.min(10_000)).unwrap_or(10_000);
            }
            FocusEffect::AddManpower(amount) => {
                country.country.population = country.country.population.saturating_add(*amount);
            }
            FocusEffect::AddResearchSlot(amount) => {
                debug_assert!(
                    *amount <= 8,
                    "research slot amount {} exceeds limit",
                    amount
                );
                for _ in 0..*amount {
                    country
                        .research_slots
                        .push(super::state::ResearchSlotState::default());
                }
            }
            FocusEffect::SetCountryFlag { flag, days } => {
                let expires_on = days.map(|days| country.country.date.add_days(days));
                country.set_country_flag(flag.clone(), expires_on);
            }
            FocusEffect::AddEquipmentToStockpile { equipment, amount } => {
                country.stockpile.add(*equipment, *amount);
            }
            FocusEffect::AddTechnologyBonus(bonus) => {
                country.add_technology_bonus(bonus.clone());
            }
            FocusEffect::CreateFaction(faction) => {
                country.create_faction(faction.clone());
            }
            FocusEffect::CreateWarGoal { target, kind } => {
                country.add_war_goal(target.clone(), kind.clone());
            }
            FocusEffect::JoinFaction(target) => {
                country.join_faction(target);
            }
            FocusEffect::SetCountryRule { rule, enabled } => {
                country.set_country_rule(rule.clone(), *enabled);
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
                        self.apply_focus_state_operation(country, index, operation, 0)?;
                    }
                }
            }
            FocusEffect::SetPolitics {
                government,
                elections_allowed,
                last_election,
            } => {
                country.country.government = *government;
                if let Some(elections_allowed) = elections_allowed {
                    country.country.elections_allowed = *elections_allowed;
                }
                if last_election.is_some() {
                    country.country.last_election = *last_election;
                }
            }
            FocusEffect::TransferState(raw_state_id) => {
                country.transfer_state_to_root(*raw_state_id);
            }
            FocusEffect::Unsupported(name) => {
                return Err(SimulationError::UnsupportedFocusEffect(name.clone()));
            }
        }

        Ok(())
    }

    fn apply_focus_idea_effect(
        scenario: &France1936Scenario,
        country: &mut CountryRuntime,
        effect: &FocusEffect,
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
            _ => {}
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
        depth: u8,
    ) -> Result<(), SimulationError> {
        debug_assert!(
            state_index < country.states.len(),
            "state_index {} out of bounds (len {})",
            state_index,
            country.states.len()
        );
        debug_assert!(
            depth < 8,
            "state operation nesting depth {} exceeds limit",
            depth
        );
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
                        self.apply_focus_state_operation(
                            country,
                            nested_index,
                            operation,
                            depth + 1,
                        )?;
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
                debug_assert!(*level <= 10, "building level {} exceeds limit", level);
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
        debug_assert!(
            state_index < country.states.len(),
            "state_index {} out of bounds (len {})",
            state_index,
            country.states.len()
        );
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
            FocusBuildingKind::Infrastructure => {
                country.states[state_index].infrastructure = country.states[state_index]
                    .infrastructure
                    .saturating_add(1)
                    .min(10)
            }
            FocusBuildingKind::LandFort => {
                country.states[state_index].land_fort_level = country.states[state_index]
                    .land_fort_level
                    .saturating_add(1)
                    .min(10)
            }
        }
    }

    fn base_research_days(&self, branch: ResearchBranch) -> u16 {
        match branch {
            ResearchBranch::Industry => 140_u16,
            ResearchBranch::Construction => 120_u16,
            ResearchBranch::Electronics => 150_u16,
            ResearchBranch::Production => 130_u16,
        }
    }

    fn daily_research_progress_centi(
        &self,
        scenario: &France1936Scenario,
        country: &CountryRuntime,
        technology: Option<crate::domain::TechId>,
        branch: ResearchBranch,
    ) -> u32 {
        let mut research_speed_bp =
            10_000_u32.saturating_add(u32::from(country.research_speed_bp(&scenario.ideas)));
        if let Some(technology) = technology {
            research_speed_bp = research_speed_bp.saturating_add(
                country.technology_bonus_bp(scenario.technology_tree.node(technology)),
            );
            let ahead_penalty_bp = self.ahead_of_time_penalty_bp(
                country.country.date,
                scenario.technology_tree.node(technology).start_year,
            );
            let progress = (u64::from(research_speed_bp) * 10_000)
                .div_ceil(u64::from(10_000 + ahead_penalty_bp));
            return u32::try_from(progress.max(1)).unwrap_or(u32::MAX);
        }

        let _ = branch;
        research_speed_bp.max(1)
    }

    fn ahead_of_time_penalty_bp(&self, current_date: GameDate, start_year: u16) -> u32 {
        let start_date = GameDate::new(start_year, 1, 1);
        if current_date >= start_date {
            return 0;
        }

        let days_ahead = u32::try_from(current_date.days_until(start_date)).unwrap_or(u32::MAX);
        days_ahead.saturating_mul(20_000).div_ceil(365)
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

    fn construction_daily_progress_centi(
        &self,
        assigned_civs: usize,
        infrastructure_multiplier_bp: u32,
        construction_speed_bp: u32,
    ) -> u32 {
        let daily_progress = u64::try_from(assigned_civs).unwrap_or(u64::MAX)
            * u64::from(self.config.construction_output_centi_per_factory)
            * u64::from(infrastructure_multiplier_bp)
            * u64::from(10_000 + construction_speed_bp)
            / 10_000
            / 10_000;

        u32::try_from(daily_progress).unwrap_or(u32::MAX)
    }

    fn production_daily_ic_centi(
        &self,
        factories: u8,
        efficiency_permille: u16,
        output_bonus_bp: u32,
    ) -> u32 {
        let daily_ic_centi = u64::from(factories)
            * u64::from(self.config.production_output_centi_per_factory)
            * u64::from(efficiency_permille)
            * u64::from(10_000 + output_bonus_bp)
            / 1_000
            / 10_000;

        u32::try_from(daily_ic_centi).unwrap_or(u32::MAX)
    }

    fn military_base_covers_readiness_shortfall(
        &self,
        scenario: &France1936Scenario,
        country: &CountryRuntime,
    ) -> bool {
        let shortfall = self.readiness_shortfall_demand(scenario, country);
        if !shortfall.has_equipment() {
            return true;
        }

        let days_remaining = u16::try_from(
            country
                .country
                .date
                .days_until(scenario.milestones[3].date)
                .max(1),
        )
        .unwrap_or(u16::MAX);
        let allocation = self.factory_allocation_for_demand(
            shortfall,
            country.equipment_profiles,
            days_remaining,
        );
        country.total_military_factories() >= allocation.total()
    }

    fn readiness_shortfall_demand(
        &self,
        scenario: &France1936Scenario,
        country: &CountryRuntime,
    ) -> EquipmentDemand {
        let mut remaining_stockpile = country.stockpile;
        let mut ready_divisions = 0_u16;
        let target_ready = scenario.force_goal.division_band().min;
        let mut shortfall = EquipmentDemand::default();

        for division in country.fielded_force.iter() {
            if !division.target_demand.has_equipment() {
                continue;
            }
            if ready_divisions >= target_ready {
                break;
            }

            let gap = division.reinforcement_gap();
            if !gap.has_equipment() {
                ready_divisions = ready_divisions.saturating_add(1);
                continue;
            }
            if remaining_stockpile.covers(gap) {
                remaining_stockpile = remaining_stockpile.saturating_sub_demand(gap);
                ready_divisions = ready_divisions.saturating_add(1);
                continue;
            }

            shortfall = shortfall.plus(EquipmentDemand {
                infantry_equipment: gap
                    .infantry_equipment
                    .saturating_sub(remaining_stockpile.infantry_equipment),
                support_equipment: gap
                    .support_equipment
                    .saturating_sub(remaining_stockpile.support_equipment),
                artillery: gap.artillery.saturating_sub(remaining_stockpile.artillery),
                anti_tank: gap.anti_tank.saturating_sub(remaining_stockpile.anti_tank),
                anti_air: gap.anti_air.saturating_sub(remaining_stockpile.anti_air),
                motorized_equipment: gap
                    .motorized_equipment
                    .saturating_sub(remaining_stockpile.motorized_equipment),
                armor: gap.armor.saturating_sub(remaining_stockpile.armor),
                fighters: gap.fighters.saturating_sub(remaining_stockpile.fighters),
                bombers: gap.bombers.saturating_sub(remaining_stockpile.bombers),
                manpower: 0,
            });
            remaining_stockpile = remaining_stockpile.saturating_sub_demand(gap);
            ready_divisions = ready_divisions.saturating_add(1);
        }

        if ready_divisions < target_ready {
            shortfall = shortfall.plus(
                scenario
                    .force_plan
                    .template
                    .per_division_demand()
                    .without_manpower()
                    .scale(target_ready.saturating_sub(ready_divisions)),
            );
        }

        shortfall
    }

    fn factory_allocation_for_demand(
        &self,
        demand: EquipmentDemand,
        equipment_profiles: crate::domain::ModeledEquipmentProfiles,
        days_remaining: u16,
    ) -> EquipmentFactoryAllocation {
        let factory_capacity_centi = self.estimated_factory_capacity_centi(days_remaining).max(1);
        let mut allocation = EquipmentFactoryAllocation::default();

        for equipment in [
            EquipmentKind::InfantryEquipment,
            EquipmentKind::SupportEquipment,
            EquipmentKind::Artillery,
            EquipmentKind::AntiTank,
            EquipmentKind::AntiAir,
            EquipmentKind::MotorizedEquipment,
            EquipmentKind::Armor,
            EquipmentKind::Fighter,
            EquipmentKind::Bomber,
        ] {
            let amount = demand.get(equipment);
            if amount == 0 {
                continue;
            }
            let total_ic = u64::from(amount)
                * u64::from(equipment_profiles.profile(equipment).unit_cost_centi);
            allocation.set(
                equipment,
                u16::try_from(total_ic.div_ceil(factory_capacity_centi)).unwrap_or(u16::MAX),
            );
        }

        allocation
    }

    fn estimated_factory_capacity_centi(&self, days: u16) -> u64 {
        let mut efficiency = 100_u16;
        let mut total = 0_u64;

        for _ in 0..days {
            total += u64::from(self.config.production_output_centi_per_factory)
                * u64::from(efficiency)
                / 1_000;
            if efficiency < self.config.production_efficiency_cap_permille {
                efficiency = (efficiency + self.config.production_efficiency_gain_permille)
                    .min(self.config.production_efficiency_cap_permille);
            }
        }

        total
    }

    fn scale_by_bp(&self, value: u32, basis_points: u16) -> u32 {
        u32::try_from(u64::from(value) * u64::from(basis_points) / 10_000).unwrap_or(u32::MAX)
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
fn debug_assert_country_invariants(_country: &CountryRuntime) {
    #[cfg(debug_assertions)]
    _country.assert_invariants();
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::domain::{
        DoctrineCostReduction, EconomyLaw, EquipmentKind, EquipmentProfile, EquipmentUnlock,
        FocusBuildingKind, FocusCondition, FocusEffect, FocusStateScope, GameDate,
        GovernmentIdeology, IdeaDefinition, IdeaModifiers, MobilizationLaw, NationalFocus,
        ResourceLedger, StateCondition, StateOperation, StateScopedEffects, TechId,
        TechnologyBonus, TechnologyModifiers, TechnologyNode, TechnologyTree, TimelineCondition,
        TradeLaw,
    };
    use crate::scenario::France1936Scenario;
    use crate::sim::{
        Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind,
        ConstructionProject, FocusAction, LawAction, LawCategory, LawTarget, ProductionAction,
        ProductionLine, ResearchAction, ResearchBranch, StateId, Stockpile,
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
    fn simulator_rejects_infrastructure_above_level_ten() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let date = GameDate::new(1936, 1, 1);
        runtime.state_defs[usize::from(France1936Scenario::ILE_DE_FRANCE.0)]
            .infrastructure_target = 10;
        let actions = [
            Action::Construction(ConstructionAction {
                date,
                state: France1936Scenario::ILE_DE_FRANCE,
                kind: ConstructionKind::Infrastructure,
            }),
            Action::Construction(ConstructionAction {
                date,
                state: France1936Scenario::ILE_DE_FRANCE,
                kind: ConstructionKind::Infrastructure,
            }),
            Action::Construction(ConstructionAction {
                date,
                state: France1936Scenario::ILE_DE_FRANCE,
                kind: ConstructionKind::Infrastructure,
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
            Err(SimulationError::ConstructionCapReached(
                France1936Scenario::ILE_DE_FRANCE,
                ConstructionKind::Infrastructure,
            ))
        );
    }

    #[test]
    fn simulator_allows_pre_pivot_military_construction() {
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

        let outcome =
            result.expect("pre-pivot military factories stay legal for EarlyMilitaryPivot");
        assert_eq!(outcome.country.construction_queue.len(), 1);
        assert_eq!(
            outcome.country.construction_queue[0].kind,
            ConstructionKind::MilitaryFactory
        );
    }

    #[test]
    fn focus_building_completion_clamps_infrastructure_and_land_forts() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let state_index = usize::from(France1936Scenario::ILE_DE_FRANCE.0);
        runtime.states[state_index].infrastructure = 10;
        runtime.states[state_index].land_fort_level = 10;

        engine.finish_focus_building(&mut runtime, state_index, FocusBuildingKind::Infrastructure);
        engine.finish_focus_building(&mut runtime, state_index, FocusBuildingKind::LandFort);

        assert_eq!(runtime.states[state_index].infrastructure, 10);
        assert_eq!(runtime.states[state_index].land_fort_level, 10);
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
    fn ahead_of_time_penalty_recedes_as_year_approaches() {
        let scenario = France1936Scenario::standard().with_exact_technology_data(
            TechnologyTree::new(vec![TechnologyNode {
                id: TechId(0),
                token: "construction2".into(),
                branch: ResearchBranch::Construction,
                categories: vec!["construction_tech".into()].into_boxed_slice(),
                start_year: 1937,
                base_days: 100,
                prerequisites: Vec::new().into_boxed_slice(),
                exclusive_with: Vec::new().into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: Vec::new().into_boxed_slice(),
            }]),
            Vec::new(),
        );
        let engine = SimulationEngine::default();
        let mut runtime = scenario.bootstrap_runtime();

        runtime.country.date = GameDate::new(1936, 1, 1);
        let far_ahead = engine.daily_research_progress_centi(
            &scenario,
            &runtime,
            Some(TechId(0)),
            ResearchBranch::Construction,
        );
        runtime.country.date = GameDate::new(1936, 12, 31);
        let near_ahead = engine.daily_research_progress_centi(
            &scenario,
            &runtime,
            Some(TechId(0)),
            ResearchBranch::Construction,
        );
        runtime.country.date = GameDate::new(1937, 1, 1);
        let on_time = engine.daily_research_progress_centi(
            &scenario,
            &runtime,
            Some(TechId(0)),
            ResearchBranch::Construction,
        );

        assert!(far_ahead < near_ahead);
        assert!(near_ahead < on_time);
    }

    #[test]
    fn exact_research_completion_unlocks_runtime_equipment_profiles() {
        let upgraded_artillery = EquipmentProfile::new(
            525,
            ResourceLedger {
                steel: 3,
                tungsten: 2,
                ..ResourceLedger::default()
            },
        );
        let scenario = France1936Scenario::standard().with_exact_technology_data(
            TechnologyTree::new(vec![TechnologyNode {
                id: TechId(0),
                token: "artillery1".into(),
                branch: ResearchBranch::Production,
                categories: vec!["artillery".into()].into_boxed_slice(),
                start_year: 1936,
                base_days: 1,
                prerequisites: Vec::new().into_boxed_slice(),
                exclusive_with: Vec::new().into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: vec![EquipmentUnlock {
                    kind: EquipmentKind::Artillery,
                    profile: upgraded_artillery,
                }]
                .into_boxed_slice(),
            }]),
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Research(ResearchAction {
            date: GameDate::new(1936, 1, 1),
            slot: 0,
            branch: ResearchBranch::Production,
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

        assert!(result.country.completed_technologies[0]);
        assert_eq!(
            result.country.equipment_profiles.artillery,
            upgraded_artillery
        );
        assert_eq!(result.country.production_lines[2].unit_cost_centi, 525);
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
    fn simulator_rejects_duplicate_advisor_purchase_on_a_later_day() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        runtime.country.political_power_centi = 400 * crate::sim::POLITICAL_POWER_UNIT;

        let engine = SimulationEngine::default();
        let actions = [
            Action::Advisor(AdvisorAction {
                date: GameDate::new(1936, 1, 1),
                kind: AdvisorKind::IndustryConcern,
            }),
            Action::Advisor(AdvisorAction {
                date: GameDate::new(1936, 1, 2),
                kind: AdvisorKind::IndustryConcern,
            }),
        ];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            GameDate::new(1936, 1, 2),
            scenario.pivot_window.start,
        );

        assert_eq!(
            result,
            Err(SimulationError::DuplicateAdvisor(
                AdvisorKind::IndustryConcern
            ))
        );
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
    fn focus_availability_supports_politics_faction_and_original_tag_checks() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_political_gate".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::All(vec![
                    FocusCondition::HasGovernment(GovernmentIdeology::Democratic),
                    FocusCondition::IsInFaction(true),
                    FocusCondition::IsSubject(false),
                    FocusCondition::IsPuppet(false),
                    FocusCondition::OriginalTag("FRA".into()),
                ]),
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_POLITICAL".into()],
                effects: vec![FocusEffect::SetCountryFlag {
                    flag: "FRA_political_gate_completed".into(),
                    days: None,
                }],
            }],
            Vec::new(),
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Focus(FocusAction {
            date: GameDate::new(1936, 1, 1),
            focus_id: "FRA_political_gate".into(),
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

        assert!(
            result
                .country
                .has_country_flag("FRA_political_gate_completed")
        );
    }

    #[test]
    fn simulator_applies_political_diplomatic_and_territorial_focus_effects() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_political_realignment".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_POLITICAL".into()],
                effects: vec![
                    FocusEffect::SetPolitics {
                        government: GovernmentIdeology::Communism,
                        elections_allowed: Some(false),
                        last_election: Some(GameDate::new(1932, 5, 1)),
                    },
                    FocusEffect::SetCountryRule {
                        rule: "can_create_factions".into(),
                        enabled: true,
                    },
                    FocusEffect::CreateFaction("FRA_popular_front".into()),
                    FocusEffect::CreateWarGoal {
                        target: "GER".into(),
                        kind: "topple_government".into(),
                    },
                    FocusEffect::TransferState(17),
                    FocusEffect::AddTechnologyBonus(TechnologyBonus {
                        name: "FRA_artillery_focus".into(),
                        categories: vec!["artillery".into()].into_boxed_slice(),
                        bonus_bp: 10_000,
                        uses: 1,
                    }),
                ],
            }],
            Vec::new(),
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Focus(FocusAction {
            date: GameDate::new(1936, 1, 1),
            focus_id: "FRA_political_realignment".into(),
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

        assert_eq!(
            result.country.country.government,
            GovernmentIdeology::Communism
        );
        assert!(!result.country.country.elections_allowed);
        assert_eq!(
            result.country.country.last_election,
            Some(GameDate::new(1932, 5, 1))
        );
        assert!(result.country.has_country_rule("can_create_factions"));
        assert_eq!(
            result.country.world_state.country_faction("FRA"),
            Some("FRA_popular_front")
        );
        assert_eq!(result.country.war_goals[0].target.as_ref(), "GER");
        assert_eq!(
            result.country.war_goals[0].kind.as_ref(),
            "topple_government"
        );
        assert_eq!(result.country.transferred_states, vec![17]);
        assert_eq!(result.country.technology_bonuses.len(), 1);
    }

    #[test]
    fn technology_bonus_prefers_matching_research_and_is_consumed_on_completion() {
        let scenario = France1936Scenario::standard().with_exact_technology_data(
            TechnologyTree::new(vec![
                TechnologyNode {
                    id: TechId(0),
                    token: "support_tech".into(),
                    branch: ResearchBranch::Production,
                    categories: vec!["support".into()].into_boxed_slice(),
                    start_year: 1936,
                    base_days: 1,
                    prerequisites: Vec::new().into_boxed_slice(),
                    exclusive_with: Vec::new().into_boxed_slice(),
                    modifiers: TechnologyModifiers::default(),
                    equipment_unlocks: Vec::new().into_boxed_slice(),
                },
                TechnologyNode {
                    id: TechId(1),
                    token: "artillery1".into(),
                    branch: ResearchBranch::Production,
                    categories: vec!["artillery".into()].into_boxed_slice(),
                    start_year: 1936,
                    base_days: 1,
                    prerequisites: Vec::new().into_boxed_slice(),
                    exclusive_with: Vec::new().into_boxed_slice(),
                    modifiers: TechnologyModifiers::default(),
                    equipment_unlocks: Vec::new().into_boxed_slice(),
                },
            ]),
            Vec::new(),
        );
        let mut runtime = scenario.bootstrap_runtime();
        runtime.add_technology_bonus(TechnologyBonus {
            name: "FRA_artillery_focus".into(),
            categories: vec!["artillery".into()].into_boxed_slice(),
            bonus_bp: 10_000,
            uses: 1,
        });
        let engine = SimulationEngine::default();
        let actions = [Action::Research(ResearchAction {
            date: GameDate::new(1936, 1, 1),
            slot: 0,
            branch: ResearchBranch::Production,
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

        assert_eq!(
            result.country.completed_technologies.as_ref(),
            &[false, true]
        );
        assert!(result.country.technology_bonuses.is_empty());
    }

    #[test]
    fn focus_availability_respects_timeline_date_and_war_conditions() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let focus = NationalFocus {
            id: "FRA_timeline_gate".into(),
            days: 1,
            prerequisites: Vec::new(),
            mutually_exclusive: Vec::new(),
            available: FocusCondition::All(vec![
                FocusCondition::Timeline(Box::new(TimelineCondition::DateAtLeast(GameDate::new(
                    1939, 9, 1,
                )))),
                FocusCondition::Not(Box::new(FocusCondition::Timeline(Box::new(
                    TimelineCondition::HasWarWith("GER".into()),
                )))),
            ]),
            bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
            search_filters: Vec::new(),
            effects: Vec::new(),
        };

        runtime.country.date = GameDate::new(1939, 8, 31);
        assert!(
            !engine
                .focus_is_available(&runtime, scenario.reference_tag, &scenario.ideas, &focus)
                .unwrap()
        );

        runtime.country.date = GameDate::new(1939, 9, 1);
        assert!(
            engine
                .focus_is_available(&runtime, scenario.reference_tag, &scenario.ideas, &focus)
                .unwrap()
        );

        runtime.country.date = GameDate::new(1939, 9, 3);
        runtime.apply_timeline_events(&scenario.timeline_events);
        assert!(
            !engine
                .focus_is_available(&runtime, scenario.reference_tag, &scenario.ideas, &focus)
                .unwrap()
        );
    }

    #[test]
    fn focus_availability_respects_enabled_dlc_gates() {
        let scenario = France1936Scenario::standard();
        let engine = SimulationEngine::default();
        let focus = NationalFocus {
            id: "FRA_dlc_gate".into(),
            days: 1,
            prerequisites: Vec::new(),
            mutually_exclusive: Vec::new(),
            available: FocusCondition::HasDlc("La Resistance".into()),
            bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
            search_filters: Vec::new(),
            effects: Vec::new(),
        };

        let runtime_without_dlc = scenario.bootstrap_runtime();
        assert!(
            !engine
                .focus_is_available(
                    &runtime_without_dlc,
                    scenario.reference_tag,
                    &scenario.ideas,
                    &focus,
                )
                .unwrap()
        );

        let runtime_with_dlc = scenario
            .bootstrap_runtime()
            .with_enabled_dlcs(vec!["La Resistance".into()].into_boxed_slice());
        assert!(
            engine
                .focus_is_available(
                    &runtime_with_dlc,
                    scenario.reference_tag,
                    &scenario.ideas,
                    &focus
                )
                .unwrap()
        );
    }

    #[test]
    fn simulator_applies_timeline_events_during_daily_progression() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        runtime.country.date = GameDate::new(1939, 9, 2);
        let engine = SimulationEngine::default();

        let outcome = engine
            .simulate(
                &scenario,
                runtime,
                &[],
                GameDate::new(1939, 9, 3),
                scenario.pivot_window.start,
            )
            .unwrap();

        assert!(outcome.country.world_state.countries_at_war("FRA", "GER"));
    }

    #[test]
    fn simulator_resolves_timeline_events_before_same_day_focus_actions() {
        let date = GameDate::new(1939, 9, 3);
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_timeline_gate".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Not(Box::new(FocusCondition::Timeline(Box::new(
                    TimelineCondition::HasWarWith("GER".into()),
                )))),
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: Vec::new(),
                effects: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = scenario.bootstrap_runtime();
        runtime.country.date = date;
        let engine = SimulationEngine::default();
        let actions = [Action::Focus(FocusAction {
            date,
            focus_id: "FRA_timeline_gate".into(),
        })];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            date,
            scenario.pivot_window.start,
        );

        assert_eq!(
            result,
            Err(SimulationError::FocusUnavailable(
                "FRA_timeline_gate".into()
            ))
        );
    }

    #[test]
    fn simulator_prunes_expired_flags_before_same_day_focus_actions() {
        let date = GameDate::new(1936, 1, 2);
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_flag_gate".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::HasCountryFlag("FRA_expiring_gate".into()),
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: Vec::new(),
                effects: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = scenario.bootstrap_runtime();
        runtime.country.date = date;
        runtime.set_country_flag("FRA_expiring_gate", Some(date));
        let engine = SimulationEngine::default();
        let actions = [Action::Focus(FocusAction {
            date,
            focus_id: "FRA_flag_gate".into(),
        })];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            date,
            scenario.pivot_window.start,
        );

        assert_eq!(
            result,
            Err(SimulationError::FocusUnavailable("FRA_flag_gate".into()))
        );
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

    #[test]
    fn simulator_clamps_focus_support_rewards_to_the_valid_range() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_support_campaign".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_POLITICAL".into()],
                effects: vec![
                    FocusEffect::AddStability(6_000),
                    FocusEffect::AddWarSupport(8_000),
                ],
            }],
            Vec::new(),
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [Action::Focus(FocusAction {
            date: GameDate::new(1936, 1, 1),
            focus_id: "FRA_support_campaign".into(),
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

        assert_eq!(result.country.country.stability_bp, 10_000);
        assert_eq!(result.country.country.war_support_bp, 10_000);
    }

    #[test]
    fn simulator_blocks_mutually_exclusive_focuses_after_completion() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![
                NationalFocus {
                    id: "FRA_focus_a".into(),
                    days: 1,
                    prerequisites: Vec::new(),
                    mutually_exclusive: vec!["FRA_focus_b".into()],
                    available: FocusCondition::Always,
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                    effects: vec![FocusEffect::AddPoliticalPower(10)],
                },
                NationalFocus {
                    id: "FRA_focus_b".into(),
                    days: 1,
                    prerequisites: Vec::new(),
                    mutually_exclusive: vec!["FRA_focus_a".into()],
                    available: FocusCondition::Always,
                    bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                    search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                    effects: vec![FocusEffect::AddPoliticalPower(10)],
                },
            ],
            Vec::new(),
            Vec::new(),
        );
        let runtime = scenario.bootstrap_runtime();
        let engine = SimulationEngine::default();
        let actions = [
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 1, 1),
                focus_id: "FRA_focus_a".into(),
            }),
            Action::Focus(FocusAction {
                date: GameDate::new(1936, 1, 2),
                focus_id: "FRA_focus_b".into(),
            }),
        ];

        let result = engine.simulate(
            &scenario,
            runtime,
            &actions,
            GameDate::new(1936, 1, 2),
            scenario.pivot_window.start,
        );

        assert_eq!(
            result,
            Err(SimulationError::FocusUnavailable("FRA_focus_b".into()))
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
                target: match a % 10 {
                    0 => LawTarget::Economy(EconomyLaw::CivilianEconomy),
                    1 => LawTarget::Economy(EconomyLaw::EarlyMobilization),
                    2 => LawTarget::Economy(EconomyLaw::PartialMobilization),
                    3 => LawTarget::Economy(EconomyLaw::WarEconomy),
                    4 => LawTarget::Economy(EconomyLaw::TotalMobilization),
                    5 => LawTarget::Trade(TradeLaw::FreeTrade),
                    6 => LawTarget::Trade(TradeLaw::ExportFocus),
                    7 => LawTarget::Trade(TradeLaw::LimitedExports),
                    8 => LawTarget::Trade(TradeLaw::ClosedEconomy),
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

        #[test]
        fn advance_production_conserves_ic_into_stockpile_units(
            factories in 1u8..15,
            unit_cost_centi in 1u32..20_000,
            accumulated_ic_centi in 0u32..20_000,
        ) {
            let mut scenario = France1936Scenario::standard();
            scenario.initial_country.laws.trade = TradeLaw::ClosedEconomy;
            for state in scenario.initial_state_defs.iter_mut() {
                state.resources = ResourceLedger::default();
            }
            scenario.initial_state_defs[0].resources = ResourceLedger {
                steel: 1_000,
                ..ResourceLedger::default()
            };
            let mut runtime = scenario.bootstrap_runtime();
            let mut line = ProductionLine::new_with_cost(
                EquipmentKind::InfantryEquipment,
                factories,
                unit_cost_centi,
            );
            line.accumulated_ic_centi = accumulated_ic_centi % unit_cost_centi;
            runtime.production_lines = vec![line].into_boxed_slice();
            runtime.stockpile = Stockpile::default();
            let engine = SimulationEngine::new(SimulationConfig {
                production_efficiency_gain_permille: 0,
                ..SimulationConfig::default()
            });

            let line_before = runtime.production_lines[0];
            let daily_ic_centi = engine.production_daily_ic_centi(
                line_before.factories,
                line_before.efficiency_permille,
                u32::from(runtime.military_output_bp(&scenario.ideas)),
            );
            let total_ic_centi = line_before.accumulated_ic_centi + daily_ic_centi;
            let expected_units = total_ic_centi / line_before.unit_cost_centi;
            let expected_remainder = total_ic_centi % line_before.unit_cost_centi;

            engine.advance_production(&scenario, &mut runtime);

            prop_assert_eq!(runtime.stockpile.infantry_equipment, expected_units);
            prop_assert_eq!(runtime.production_lines[0].accumulated_ic_centi, expected_remainder);
        }

        #[test]
        fn advance_production_static_lines_gain_efficiency_monotonically_up_to_cap(
            factories in 1u8..15,
            days in 1usize..365,
        ) {
            let mut scenario = France1936Scenario::standard();
            scenario.initial_country.laws.trade = TradeLaw::ClosedEconomy;
            for state in scenario.initial_state_defs.iter_mut() {
                state.resources = ResourceLedger::default();
            }
            scenario.initial_state_defs[0].resources = ResourceLedger {
                steel: 1_000,
                ..ResourceLedger::default()
            };
            let mut runtime = scenario.bootstrap_runtime();
            runtime.production_lines = vec![ProductionLine::new(EquipmentKind::InfantryEquipment, factories)]
                .into_boxed_slice();
            let engine = SimulationEngine::default();
            let mut previous_efficiency = runtime.production_lines[0].efficiency_permille;

            for _ in 0..days {
                engine.advance_production(&scenario, &mut runtime);
                let current_efficiency = runtime.production_lines[0].efficiency_permille;
                prop_assert!(current_efficiency >= previous_efficiency);
                prop_assert!(current_efficiency <= engine.config.production_efficiency_cap_permille);
                previous_efficiency = current_efficiency;
            }
        }

        #[test]
        fn advance_production_single_line_scales_ic_by_resource_fulfillment(
            factories in 1u8..15,
            efficiency_permille in 100u16..1001,
            available_steel in 0u16..40,
        ) {
            let mut scenario = France1936Scenario::standard();
            scenario.initial_country.laws.trade = TradeLaw::ClosedEconomy;
            for state in scenario.initial_state_defs.iter_mut() {
                state.resources = ResourceLedger::default();
            }
            scenario.initial_state_defs[0].resources = ResourceLedger {
                steel: u32::from(available_steel),
                ..ResourceLedger::default()
            };

            let mut runtime = scenario.bootstrap_runtime();
            let mut line = ProductionLine::new_with_cost(
                EquipmentKind::InfantryEquipment,
                factories,
                1,
            );
            line.efficiency_permille = efficiency_permille;
            runtime.production_lines = vec![line].into_boxed_slice();
            runtime.stockpile = Stockpile::default();
            let engine = SimulationEngine::new(SimulationConfig {
                production_efficiency_gain_permille: 0,
                ..SimulationConfig::default()
            });

            let line_before = runtime.production_lines[0];
            let available_resources = runtime.domestic_resources(&scenario.ideas);
            let resource_demand = line_before.daily_resource_demand(scenario.equipment_profiles);
            let fulfillment_bp = resource_demand.fulfillment_bp(available_resources);
            let expected_ic = engine.scale_by_bp(
                engine.production_daily_ic_centi(
                    line_before.factories,
                    line_before.efficiency_permille,
                    u32::from(runtime.military_output_bp(&scenario.ideas)),
                ),
                fulfillment_bp,
            );

            engine.advance_production(&scenario, &mut runtime);

            prop_assert_eq!(runtime.stockpile.infantry_equipment, expected_ic);
            prop_assert_eq!(runtime.production_lines[0].accumulated_ic_centi, 0);
        }

        #[test]
        fn research_progress_stays_positive_under_large_speed_bonuses(
            branch in prop_oneof![
                Just(ResearchBranch::Industry),
                Just(ResearchBranch::Construction),
                Just(ResearchBranch::Electronics),
                Just(ResearchBranch::Production),
            ],
            research_speed_bp in 0u16..u16::MAX,
        ) {
            let engine = SimulationEngine::default();
            let scenario = France1936Scenario::standard();
            let mut runtime = scenario.bootstrap_runtime();
            runtime.technology_modifiers.research_speed_bp = i32::from(research_speed_bp);
            let progress =
                engine.daily_research_progress_centi(&scenario, &runtime, None, branch);

            prop_assert!(progress > 0);
        }

        #[test]
        fn simulator_determinism_identical_runs_produce_identical_outcomes(
            day_offset in 0u16..120,
        ) {
            let scenario = France1936Scenario::standard();
            let runtime = scenario.bootstrap_runtime();
            let end = scenario.start_date.add_days(day_offset);
            let engine = SimulationEngine::default();

            let outcome_a = engine
                .simulate(&scenario, runtime.clone(), &[], end, scenario.pivot_window.start)
                .unwrap();
            let outcome_b = engine
                .simulate(&scenario, runtime, &[], end, scenario.pivot_window.start)
                .unwrap();

            prop_assert_eq!(outcome_a.country.country, outcome_b.country.country);
            prop_assert_eq!(outcome_a.country.stockpile, outcome_b.country.stockpile);
            prop_assert_eq!(outcome_a.country.states, outcome_b.country.states);
        }

        #[test]
        fn construction_progress_is_nondecreasing_over_time(
            day_count in 1u16..60,
        ) {
            let scenario = France1936Scenario::standard();
            let engine = SimulationEngine::default();
            let mut runtime = scenario.bootstrap_runtime();

            let mut previous_total_progress: u32 = runtime
                .construction_queue
                .iter()
                .map(|p| p.progress_centi)
                .sum();

            for _ in 0..day_count {
                engine.advance_construction(&scenario, &mut runtime);
                let current_total: u32 = runtime
                    .construction_queue
                    .iter()
                    .map(|p| p.progress_centi)
                    .sum();
                prop_assert!(
                    current_total >= previous_total_progress,
                    "construction progress decreased: {} -> {}",
                    previous_total_progress,
                    current_total
                );
                previous_total_progress = current_total;
            }
        }
    }

    #[test]
    fn advance_production_applies_trade_law_factory_output_bonus() {
        let mut scenario = France1936Scenario::standard();
        for state in scenario.initial_state_defs.iter_mut() {
            state.resources = ResourceLedger::default();
        }
        scenario.initial_state_defs[0].resources = ResourceLedger {
            steel: 1_000,
            ..ResourceLedger::default()
        };
        let mut closed = scenario.bootstrap_runtime();
        let mut free = scenario.bootstrap_runtime();
        closed.country.laws.trade = TradeLaw::ClosedEconomy;
        free.country.laws.trade = TradeLaw::FreeTrade;
        closed.production_lines = vec![ProductionLine::new_with_cost(
            EquipmentKind::InfantryEquipment,
            5,
            1_000_000,
        )]
        .into_boxed_slice();
        free.production_lines = closed.production_lines.clone();
        let engine = SimulationEngine::new(SimulationConfig {
            production_efficiency_gain_permille: 0,
            ..SimulationConfig::default()
        });

        let closed_expected = engine.production_daily_ic_centi(
            5,
            100,
            u32::from(closed.military_output_bp(&scenario.ideas)),
        );
        let free_expected = engine.production_daily_ic_centi(
            5,
            100,
            u32::from(free.military_output_bp(&scenario.ideas)),
        );

        engine.advance_production(&scenario, &mut closed);
        engine.advance_production(&scenario, &mut free);

        assert_eq!(
            closed.production_lines[0].accumulated_ic_centi,
            closed_expected
        );
        assert_eq!(free.production_lines[0].accumulated_ic_centi, free_expected);
        assert!(free_expected > closed_expected);
    }

    #[test]
    fn daily_research_progress_uses_trade_law_and_exact_technology_bonuses() {
        let engine = SimulationEngine::default();
        let scenario = France1936Scenario::standard();
        let mut baseline = scenario.bootstrap_runtime();
        baseline.country.laws.trade = TradeLaw::ClosedEconomy;
        baseline.completed_technologies = vec![false].into_boxed_slice();

        let mut boosted = baseline.clone();
        boosted.country.laws.trade = TradeLaw::FreeTrade;
        boosted.technology_modifiers = TechnologyModifiers {
            research_speed_bp: 500,
            ..TechnologyModifiers::default()
        };

        assert_eq!(
            engine.daily_research_progress_centi(
                &scenario,
                &baseline,
                None,
                ResearchBranch::Electronics,
            ),
            10_000
        );
        assert_eq!(
            engine.daily_research_progress_centi(
                &scenario,
                &boosted,
                None,
                ResearchBranch::Electronics,
            ),
            11_500
        );
    }

    #[test]
    fn construction_speed_scales_with_infrastructure_level() {
        let engine = SimulationEngine::default();
        let assigned_civs = 10;
        let base = engine.construction_daily_progress_centi(assigned_civs, 10_000, 0);
        let infra_five = engine.construction_daily_progress_centi(assigned_civs, 15_000, 0);
        let infra_ten = engine.construction_daily_progress_centi(assigned_civs, 20_000, 0);

        assert_eq!(base, 5_000);
        assert_eq!(infra_five, 7_500);
        assert_eq!(infra_ten, 10_000);
    }

    #[test]
    fn advance_construction_caps_daily_progress_to_remaining_cost() {
        let scenario = France1936Scenario::standard();
        let mut runtime = scenario.bootstrap_runtime();
        let initial_civs = runtime
            .state(France1936Scenario::ILE_DE_FRANCE)
            .civilian_factories;
        runtime.construction_queue = vec![ConstructionProject {
            state: France1936Scenario::ILE_DE_FRANCE,
            kind: ConstructionKind::CivilianFactory,
            total_cost_centi: 1_000,
            progress_centi: 950,
        }];
        let engine = SimulationEngine::default();

        engine.advance_construction(&scenario, &mut runtime);

        assert!(runtime.construction_queue.is_empty());
        assert_eq!(
            runtime
                .state(France1936Scenario::ILE_DE_FRANCE)
                .civilian_factories,
            initial_civs + 1,
        );
    }

    #[test]
    fn advance_construction_applies_trade_law_construction_speed_bonus() {
        let scenario = France1936Scenario::standard();
        let mut closed = scenario.bootstrap_runtime();
        let mut free = scenario.bootstrap_runtime();
        closed.country.laws.trade = TradeLaw::ClosedEconomy;
        free.country.laws.trade = TradeLaw::FreeTrade;
        closed.country.laws.economy = EconomyLaw::WarEconomy;
        free.country.laws.economy = EconomyLaw::WarEconomy;
        let project = ConstructionProject {
            state: France1936Scenario::ILE_DE_FRANCE,
            kind: ConstructionKind::CivilianFactory,
            total_cost_centi: 500_000,
            progress_centi: 0,
        };
        closed.construction_queue = vec![project];
        free.construction_queue = vec![project];
        let engine = SimulationEngine::default();
        let assigned_civs =
            usize::from(closed.available_civilian_factories(&scenario.ideas).min(15));
        let infrastructure_multiplier_bp = 10_000
            + u32::from(
                closed
                    .state(France1936Scenario::ILE_DE_FRANCE)
                    .infrastructure,
            ) * 1_000;

        let closed_expected = engine.construction_daily_progress_centi(
            assigned_civs,
            infrastructure_multiplier_bp,
            u32::from(
                closed
                    .construction_speed_bp_for(FocusBuildingKind::CivilianFactory, &scenario.ideas),
            ),
        );
        let free_expected = engine.construction_daily_progress_centi(
            assigned_civs,
            infrastructure_multiplier_bp,
            u32::from(
                free.construction_speed_bp_for(FocusBuildingKind::CivilianFactory, &scenario.ideas),
            ),
        );

        engine.advance_construction(&scenario, &mut closed);
        engine.advance_construction(&scenario, &mut free);

        assert_eq!(closed.construction_queue[0].progress_centi, closed_expected);
        assert_eq!(free.construction_queue[0].progress_centi, free_expected);
        assert!(free_expected > closed_expected);
    }

    #[test]
    fn advance_production_scales_output_linearly_under_resource_shortage() {
        let mut scenario = France1936Scenario::standard();
        scenario.initial_country.laws.trade = TradeLaw::ClosedEconomy;
        for state in scenario.initial_state_defs.iter_mut() {
            state.resources = ResourceLedger::default();
        }
        scenario.initial_state_defs[0].resources = ResourceLedger {
            steel: 10,
            ..ResourceLedger::default()
        };

        let mut runtime = scenario.bootstrap_runtime();
        let mut line = ProductionLine::new_with_cost(EquipmentKind::InfantryEquipment, 10, 100);
        line.efficiency_permille = 1_000;
        runtime.production_lines = vec![line].into_boxed_slice();
        runtime.stockpile = Stockpile::default();
        let engine = SimulationEngine::new(SimulationConfig {
            production_efficiency_gain_permille: 0,
            ..SimulationConfig::default()
        });

        engine.advance_production(&scenario, &mut runtime);

        assert_eq!(runtime.stockpile.infantry_equipment, 22);
        assert_eq!(runtime.production_lines[0].accumulated_ic_centi, 50);
    }
}
