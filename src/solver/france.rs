use crate::domain::{
    EquipmentDemand, EquipmentFactoryAllocation, EquipmentKind, GameDate, NationalFocus,
    StrategicGoalWeights,
};
use crate::scenario::France1936Scenario;
use crate::sim::{
    Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, CountryRuntime,
    FocusAction, LawAction, LawTarget, ResearchAction, ResearchBranch, SimulationEngine,
    SimulationError, StrategicPhase,
};

use super::{BeamSearchConfig, PlannerWeights};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StrategyTemplateKind {
    CivFirst,
    InfraAssisted,
    EarlyMilitaryPivot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedSolution {
    pub template: StrategyTemplateKind,
    pub pivot_date: GameDate,
    pub actions: Vec<Action>,
    pub score: i64,
    pub final_state: CountryRuntime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlannerNode {
    template: StrategyTemplateKind,
    pivot_date: GameDate,
    actions: Vec<Action>,
    runtime: CountryRuntime,
    score: i64,
}

pub struct FranceBeamPlanner {
    pub scenario: France1936Scenario,
    pub simulator: SimulationEngine,
    pub config: BeamSearchConfig,
    pub weights: PlannerWeights,
    pub strategic_goals: StrategicGoalWeights,
}

impl FranceBeamPlanner {
    pub fn new(
        scenario: France1936Scenario,
        simulator: SimulationEngine,
        config: BeamSearchConfig,
        weights: PlannerWeights,
    ) -> Self {
        Self {
            scenario,
            simulator,
            config,
            weights,
            strategic_goals: StrategicGoalWeights::new(8, 8, 3, 3),
        }
    }

    pub fn with_strategic_goals(mut self, strategic_goals: StrategicGoalWeights) -> Self {
        self.strategic_goals = strategic_goals;
        self
    }

    pub fn plan(&self) -> Result<PlannedSolution, SimulationError> {
        let balanced = self.search(self.config)?;
        if self.hard_requirements_met(&balanced.final_state) {
            return Ok(balanced);
        }

        if let Some(repaired) = self.repair_hard_requirements(&balanced)? {
            return Ok(repaired);
        }

        Err(SimulationError::HardRequirementsUnsatisfied)
    }

    pub fn best_effort_plan(&self) -> Result<PlannedSolution, SimulationError> {
        self.search(self.config)
    }

    fn search(&self, config: BeamSearchConfig) -> Result<PlannedSolution, SimulationError> {
        let end_date = self.scenario.milestones[3].date;
        let mut frontier = self.seed_nodes(config);
        let seed_count = frontier.len();
        debug_assert!(
            !frontier.is_empty(),
            "beam search must have at least one seed node"
        );

        while frontier
            .iter()
            .any(|node| node.runtime.country.date < end_date)
        {
            debug_assert!(
                !frontier.is_empty(),
                "frontier must not be empty during search"
            );
            let mut next_frontier = Vec::with_capacity(frontier.len());
            let mut last_error = None;

            for node in frontier.into_iter() {
                if node.runtime.country.date >= end_date {
                    next_frontier.push(node);
                    continue;
                }

                let window_actions = self.generate_window_actions(&node);
                let window_end = min_date(
                    node.runtime.country.date.add_days(config.replan_days),
                    end_date,
                );

                let mut child_actions = node.actions.clone();
                child_actions.extend(window_actions.iter().cloned());

                let outcome = match self.simulator.simulate(
                    &self.scenario,
                    node.runtime.clone(),
                    &window_actions,
                    window_end,
                    node.pivot_date,
                ) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        last_error = Some(error);
                        continue;
                    }
                };

                let mut next_runtime = outcome.country;
                if window_end < end_date {
                    let stability_drift_bp =
                        next_runtime.next_daily_stability_drift_bp(&self.scenario.ideas);
                    next_runtime.country.advance_day(
                        next_runtime.political_power_daily_bonus_centi(&self.scenario.ideas),
                        stability_drift_bp,
                    );
                    next_runtime.tick_active_ideas();
                    next_runtime.apply_timeline_events(&self.scenario.timeline_events);
                    next_runtime.prune_expired_country_flags();
                }

                next_frontier.push(PlannerNode {
                    template: node.template,
                    pivot_date: node.pivot_date,
                    actions: child_actions,
                    score: self.score(&next_runtime),
                    runtime: next_runtime,
                });
            }
            if next_frontier.is_empty() {
                return Err(last_error.unwrap_or(SimulationError::HardRequirementsUnsatisfied));
            }

            next_frontier.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then_with(|| left.template.cmp(&right.template))
                    .then_with(|| left.pivot_date.cmp(&right.pivot_date))
            });
            next_frontier.truncate(config.beam_width.max(seed_count));
            frontier = next_frontier;
        }

        let best = frontier
            .iter()
            .filter(|node| self.hard_requirements_met(&node.runtime))
            .max_by(|left, right| left.score.cmp(&right.score))
            .cloned()
            .or_else(|| {
                frontier
                    .into_iter()
                    .max_by(|left, right| left.score.cmp(&right.score))
            })
            .expect("planner seeds at least one node");

        Ok(PlannedSolution {
            template: best.template,
            pivot_date: best.pivot_date,
            actions: best.actions,
            score: best.score,
            final_state: best.runtime,
        })
    }

    fn seed_nodes(&self, config: BeamSearchConfig) -> Vec<PlannerNode> {
        let pivot_dates = self.pivot_dates(config);
        let mut nodes = Vec::with_capacity(pivot_dates.len() * 3);

        for template in [
            StrategyTemplateKind::CivFirst,
            StrategyTemplateKind::InfraAssisted,
            StrategyTemplateKind::EarlyMilitaryPivot,
        ] {
            for pivot_date in pivot_dates.iter().copied() {
                let runtime = self.scenario.bootstrap_runtime();
                nodes.push(PlannerNode {
                    template,
                    pivot_date,
                    actions: Vec::with_capacity(256),
                    score: 0,
                    runtime,
                });
            }
        }

        nodes
    }

    fn pivot_dates(&self, config: BeamSearchConfig) -> Vec<GameDate> {
        let mut dates = Vec::new();
        let mut date = self.scenario.pivot_window.start;

        loop {
            dates.push(date);
            if date >= self.scenario.pivot_window.end {
                break;
            }

            let next = date.add_days(config.replan_days);
            if next > self.scenario.pivot_window.end {
                dates.push(self.scenario.pivot_window.end);
                break;
            }

            date = next;
        }

        dates.sort_unstable();
        dates.dedup();
        dates
    }

    fn generate_window_actions(&self, node: &PlannerNode) -> Vec<Action> {
        let mut actions = Vec::with_capacity(16);
        let date = node.runtime.country.date;
        let phase = self.phase(node, date);
        let mut reserved_research = self.reserved_research_branches(&node.runtime);

        if node.runtime.focus.is_none()
            && let Some(action) = self.next_focus_action(node, phase, date)
        {
            actions.push(Action::Focus(action));
        }

        for slot in 0..node.runtime.research_slots.len() {
            if node.runtime.research_slots[slot].branch.is_none()
                && let Some(branch) = self.next_research_branch(node, phase, &reserved_research)
            {
                reserved_research[branch.index()] = true;
                actions.push(Action::Research(ResearchAction {
                    date,
                    slot: u8::try_from(slot).unwrap_or(u8::MAX),
                    branch,
                }));
            }
        }

        if let Some(action) = self.next_law_action(node, phase, date) {
            actions.push(Action::Law(action));
        } else if let Some(action) = self.next_advisor_action(node, phase, date) {
            actions.push(Action::Advisor(action));
        }

        if let Some(action) = self.next_production_action(node, date) {
            actions.push(Action::Production(action));
        }

        let queue_fill = 4_usize.saturating_sub(node.runtime.construction_queue.len());
        for _ in 0..queue_fill {
            if let Some(action) = self.next_construction_action(node, phase, date, &actions) {
                actions.push(Action::Construction(action));
            }
        }

        actions.sort_by_key(|action| action.date());
        actions
    }

    fn next_focus_action(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        date: GameDate,
    ) -> Option<FocusAction> {
        self.scenario
            .focuses
            .iter()
            .filter(|focus| !node.runtime.has_completed_focus(&focus.id))
            .filter(|focus| self.focus_is_supported(focus))
            .filter(|focus| {
                self.simulator
                    .focus_is_available(
                        &node.runtime,
                        self.scenario.reference_tag,
                        &self.scenario.ideas,
                        focus,
                    )
                    .unwrap_or(false)
            })
            .max_by_key(|focus| self.focus_priority(node, phase, focus))
            .map(|focus| FocusAction {
                date,
                focus_id: focus.id.clone(),
            })
    }

    fn focus_priority(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        focus: &NationalFocus,
    ) -> i64 {
        let mut score = self.focus_effect_score(focus);

        if self.focus_advances_hard_goal(&focus.id, &node.runtime) {
            score += 1_000_000;
        }
        if !node
            .runtime
            .frontier_forts_complete(&self.scenario.frontier_forts)
            && self.focus_builds_land_forts(focus)
        {
            score += 750_000;
        }

        match phase {
            StrategicPhase::PrePivot => {
                if focus.has_filter("FOCUS_FILTER_INDUSTRY") {
                    score += 8_000;
                }
                if focus.has_filter("FOCUS_FILTER_RESEARCH") {
                    score += 7_000;
                }
                if focus.has_filter("FOCUS_FILTER_STABILITY") {
                    score += 4_000;
                }
                if focus.has_filter("FOCUS_FILTER_POLITICAL") {
                    score += 2_000;
                }
            }
            StrategicPhase::PostPivot => {
                if focus.has_filter("FOCUS_FILTER_INDUSTRY") {
                    score += 6_000;
                }
                if focus.has_filter("FOCUS_FILTER_MANPOWER") {
                    score += 6_000;
                }
                if focus.has_filter("FOCUS_FILTER_WAR_SUPPORT") {
                    score += 5_000;
                }
                if focus.has_filter("FOCUS_FILTER_RESEARCH") {
                    score += 3_000;
                }
            }
        }

        score
    }

    fn focus_is_supported(&self, focus: &NationalFocus) -> bool {
        self.focus_condition_supported(&focus.available)
            && self.focus_condition_supported(&focus.bypass)
            && focus
                .effects
                .iter()
                .all(|effect| self.focus_effect_supported(effect))
    }

    fn focus_condition_supported(&self, condition: &crate::domain::FocusCondition) -> bool {
        match condition {
            crate::domain::FocusCondition::Always => true,
            crate::domain::FocusCondition::All(conditions)
            | crate::domain::FocusCondition::Any(conditions) => conditions
                .iter()
                .all(|condition| self.focus_condition_supported(condition)),
            crate::domain::FocusCondition::Not(condition) => {
                self.focus_condition_supported(condition)
            }
            crate::domain::FocusCondition::AnyControlledState(condition)
            | crate::domain::FocusCondition::AnyOwnedState(condition)
            | crate::domain::FocusCondition::AnyState(condition) => {
                self.state_condition_supported(condition)
            }
            crate::domain::FocusCondition::Unsupported(_) => false,
            crate::domain::FocusCondition::HasCompletedFocus(_)
            | crate::domain::FocusCondition::HasCountryFlag(_)
            | crate::domain::FocusCondition::HasDlc(_)
            | crate::domain::FocusCondition::HasGameRule { .. }
            | crate::domain::FocusCondition::HasIdea(_)
            | crate::domain::FocusCondition::Timeline(_)
            | crate::domain::FocusCondition::HasWarSupportAtLeast(_)
            | crate::domain::FocusCondition::NumOfFactoriesAtLeast(_)
            | crate::domain::FocusCondition::NumOfMilitaryFactoriesAtLeast(_)
            | crate::domain::FocusCondition::AmountResearchSlotsGreaterThan(_)
            | crate::domain::FocusCondition::AmountResearchSlotsLessThan(_) => true,
        }
    }

    fn state_condition_supported(&self, condition: &crate::domain::StateCondition) -> bool {
        match condition {
            crate::domain::StateCondition::Always => true,
            crate::domain::StateCondition::All(conditions)
            | crate::domain::StateCondition::Any(conditions) => conditions
                .iter()
                .all(|condition| self.state_condition_supported(condition)),
            crate::domain::StateCondition::Not(condition) => {
                self.state_condition_supported(condition)
            }
            crate::domain::StateCondition::Unsupported(_) => false,
            crate::domain::StateCondition::RawStateId(_)
            | crate::domain::StateCondition::IsControlledByRoot
            | crate::domain::StateCondition::IsCoreOfRoot
            | crate::domain::StateCondition::IsOwnedByRoot
            | crate::domain::StateCondition::OwnerIsRootOrSubject
            | crate::domain::StateCondition::HasStateFlag(_)
            | crate::domain::StateCondition::InfrastructureLessThan(_)
            | crate::domain::StateCondition::FreeSharedBuildingSlotsGreaterThan(_) => true,
        }
    }

    fn focus_effect_supported(&self, effect: &crate::domain::FocusEffect) -> bool {
        match effect {
            crate::domain::FocusEffect::Unsupported(_) => false,
            crate::domain::FocusEffect::StateScoped(scope) => {
                scope
                    .operations
                    .iter()
                    .all(|operation| self.state_operation_supported(operation))
                    && self.state_condition_supported(&scope.limit)
            }
            crate::domain::FocusEffect::AddIdea(id)
            | crate::domain::FocusEffect::AddTimedIdea { id, .. } => {
                self.scenario.idea_by_id(id).is_some()
            }
            crate::domain::FocusEffect::SwapIdea { add, .. } => {
                self.scenario.idea_by_id(add).is_some()
            }
            crate::domain::FocusEffect::RemoveIdea(_)
            | crate::domain::FocusEffect::AddArmyExperience(_)
            | crate::domain::FocusEffect::AddCountryLeaderTrait(_)
            | crate::domain::FocusEffect::AddDoctrineCostReduction(_)
            | crate::domain::FocusEffect::AddManpower(_)
            | crate::domain::FocusEffect::AddPoliticalPower(_)
            | crate::domain::FocusEffect::AddResearchSlot(_)
            | crate::domain::FocusEffect::AddStability(_)
            | crate::domain::FocusEffect::AddWarSupport(_)
            | crate::domain::FocusEffect::AddEquipmentToStockpile { .. }
            | crate::domain::FocusEffect::SetCountryFlag { .. } => true,
        }
    }

    fn state_operation_supported(&self, operation: &crate::domain::StateOperation) -> bool {
        match operation {
            crate::domain::StateOperation::NestedScope(scope) => {
                self.state_condition_supported(&scope.limit)
                    && scope
                        .operations
                        .iter()
                        .all(|operation| self.state_operation_supported(operation))
            }
            crate::domain::StateOperation::AddBuildingConstruction { instant, .. } => *instant,
            crate::domain::StateOperation::AddExtraSharedBuildingSlots(_)
            | crate::domain::StateOperation::SetStateFlag(_) => true,
        }
    }

    fn focus_advances_hard_goal(&self, candidate: &str, runtime: &CountryRuntime) -> bool {
        self.scenario.hard_focus_goals.iter().any(|goal| {
            !runtime.completed_focus_by(&goal.id, goal.deadline)
                && self.focus_is_prerequisite_of(candidate, &goal.id, 0)
        })
    }

    fn focus_is_prerequisite_of(&self, candidate: &str, target: &str, depth: u8) -> bool {
        if candidate == target {
            return true;
        }
        if depth >= 32 {
            return false;
        }

        self.scenario
            .focus_by_id(target)
            .map(|focus| {
                focus.prerequisites.iter().any(|prerequisite| {
                    prerequisite.as_ref() == candidate
                        || self.focus_is_prerequisite_of(candidate, prerequisite, depth + 1)
                })
            })
            .unwrap_or(false)
    }

    fn focus_effect_score(&self, focus: &NationalFocus) -> i64 {
        focus.effects.iter().fold(0_i64, |score, effect| {
            score
                + match effect {
                    crate::domain::FocusEffect::AddResearchSlot(amount) => {
                        i64::from(*amount) * 12_000
                    }
                    crate::domain::FocusEffect::AddIdea(id) => self.idea_effect_score(id).max(500),
                    crate::domain::FocusEffect::AddTimedIdea { id, days } => {
                        let base = self.idea_effect_score(id).max(500);
                        base * i64::from((*days).max(35)) / 70
                    }
                    crate::domain::FocusEffect::RemoveIdea(id) => -self.idea_effect_score(id),
                    crate::domain::FocusEffect::AddArmyExperience(amount) => {
                        i64::from(*amount) * 50
                    }
                    crate::domain::FocusEffect::AddDoctrineCostReduction(reduction) => {
                        i64::from(reduction.cost_reduction_bp) * i64::from(reduction.uses)
                    }
                    crate::domain::FocusEffect::AddCountryLeaderTrait(_) => 250,
                    crate::domain::FocusEffect::AddPoliticalPower(amount) => {
                        i64::from(*amount / 100) * 20
                    }
                    crate::domain::FocusEffect::AddStability(amount)
                    | crate::domain::FocusEffect::AddWarSupport(amount) => i64::from(*amount) * 2,
                    crate::domain::FocusEffect::AddManpower(amount) => {
                        i64::try_from(*amount / 1_000).unwrap_or(i64::MAX)
                    }
                    crate::domain::FocusEffect::AddEquipmentToStockpile { amount, .. } => {
                        i64::from(*amount / 10)
                    }
                    crate::domain::FocusEffect::SetCountryFlag { .. } => 500,
                    crate::domain::FocusEffect::SwapIdea { remove, add } => {
                        self.idea_effect_score(add) - self.idea_effect_score(remove)
                    }
                    crate::domain::FocusEffect::StateScoped(scope) => scope
                        .operations
                        .iter()
                        .map(|operation| match operation {
                            crate::domain::StateOperation::AddBuildingConstruction {
                                kind: crate::domain::FocusBuildingKind::LandFort,
                                level,
                                ..
                            } => i64::from(*level) * 8_000,
                            crate::domain::StateOperation::AddBuildingConstruction {
                                kind: crate::domain::FocusBuildingKind::CivilianFactory,
                                level,
                                ..
                            } => i64::from(*level) * 5_000,
                            crate::domain::StateOperation::AddBuildingConstruction {
                                kind: crate::domain::FocusBuildingKind::MilitaryFactory,
                                level,
                                ..
                            } => i64::from(*level) * 5_500,
                            crate::domain::StateOperation::AddBuildingConstruction {
                                level,
                                ..
                            } => i64::from(*level) * 1_500,
                            crate::domain::StateOperation::AddExtraSharedBuildingSlots(amount) => {
                                i64::from(*amount) * 2_000
                            }
                            crate::domain::StateOperation::NestedScope(scope) => scope
                                .operations
                                .iter()
                                .map(|nested| match nested {
                                    crate::domain::StateOperation::AddBuildingConstruction {
                                        kind: crate::domain::FocusBuildingKind::LandFort,
                                        level,
                                        ..
                                    } => i64::from(*level) * 4_000,
                                    crate::domain::StateOperation::AddBuildingConstruction {
                                        level,
                                        ..
                                    } => i64::from(*level) * 1_500,
                                    crate::domain::StateOperation::AddExtraSharedBuildingSlots(
                                        amount,
                                    ) => i64::from(*amount) * 1_000,
                                    crate::domain::StateOperation::NestedScope(_) => 500,
                                    crate::domain::StateOperation::SetStateFlag(_) => 50,
                                })
                                .sum::<i64>(),
                            crate::domain::StateOperation::SetStateFlag(_) => 100,
                        })
                        .sum::<i64>(),
                    crate::domain::FocusEffect::Unsupported(_) => -100_000,
                }
        })
    }

    fn idea_effect_score(&self, id: &str) -> i64 {
        let Some(idea) = self.scenario.idea_by_id(id) else {
            return 0;
        };
        let modifiers = idea.modifiers;
        i64::from(-modifiers.consumer_goods_bp) * 4
            + i64::from(modifiers.stability_bp) * 2
            + i64::from(modifiers.stability_weekly_bp) * 20
            + i64::from(modifiers.war_support_bp) * 2
            + i64::from(modifiers.political_power_daily_centi) * 200
            + i64::from(modifiers.factory_output_bp) * 4
            + i64::from(modifiers.research_speed_bp) * 5
            + i64::from(modifiers.recruitable_population_bp) * 2
            + i64::from(modifiers.manpower_bp) * 2
            + i64::from(modifiers.resource_factor_bp) * 2
            + i64::from(
                modifiers.civilian_factory_construction_bp
                    + modifiers.military_factory_construction_bp
                    + modifiers.infrastructure_construction_bp
                    + modifiers.land_fort_construction_bp,
            ) * 4
    }

    fn focus_builds_land_forts(&self, focus: &NationalFocus) -> bool {
        focus.effects.iter().any(|effect| match effect {
            crate::domain::FocusEffect::StateScoped(scope) => scope
                .operations
                .iter()
                .any(Self::state_operation_builds_land_forts),
            _ => false,
        })
    }

    fn state_operation_builds_land_forts(operation: &crate::domain::StateOperation) -> bool {
        match operation {
            crate::domain::StateOperation::AddBuildingConstruction {
                kind: crate::domain::FocusBuildingKind::LandFort,
                ..
            } => true,
            crate::domain::StateOperation::NestedScope(scope) => scope
                .operations
                .iter()
                .any(Self::state_operation_builds_land_forts),
            _ => false,
        }
    }

    fn next_research_branch(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        reserved: &[bool; ResearchBranch::COUNT],
    ) -> Option<ResearchBranch> {
        let research = node.runtime.completed_research;
        let force_plan = self.scenario.force_plan;
        let military_gap = force_plan
            .required_military_factories
            .saturating_sub(node.runtime.total_military_factories());
        let support_intensity = force_plan
            .factory_allocation
            .support_equipment
            .saturating_add(force_plan.factory_allocation.artillery)
            .saturating_add(force_plan.factory_allocation.anti_tank)
            .saturating_add(force_plan.factory_allocation.anti_air);
        let mut priorities = [
            (
                ResearchBranch::Construction,
                research.construction,
                match phase {
                    StrategicPhase::PrePivot => 0_u8,
                    StrategicPhase::PostPivot => 1_u8,
                },
            ),
            (
                ResearchBranch::Industry,
                research.industry,
                match phase {
                    StrategicPhase::PrePivot => 1_u8,
                    StrategicPhase::PostPivot => 2_u8,
                },
            ),
            (
                ResearchBranch::Electronics,
                research.electronics,
                match phase {
                    StrategicPhase::PrePivot => {
                        if support_intensity >= 10 {
                            2_u8
                        } else {
                            3_u8
                        }
                    }
                    StrategicPhase::PostPivot => 3_u8,
                },
            ),
            (
                ResearchBranch::Production,
                research.production,
                match phase {
                    StrategicPhase::PrePivot => {
                        if military_gap >= 12 {
                            0_u8
                        } else {
                            2_u8
                        }
                    }
                    StrategicPhase::PostPivot => 0_u8,
                },
            ),
        ];
        priorities.sort_by_key(|(_, completed, phase_bias)| (*completed, *phase_bias));

        priorities
            .into_iter()
            .map(|(branch, _, _)| branch)
            .find(|branch: &ResearchBranch| {
                !reserved[branch.index()] && self.research_branch_available(&node.runtime, *branch)
            })
    }

    fn reserved_research_branches(
        &self,
        runtime: &CountryRuntime,
    ) -> [bool; ResearchBranch::COUNT] {
        let mut reserved = [false; ResearchBranch::COUNT];

        for slot in &runtime.research_slots {
            if let Some(branch) = slot.branch {
                reserved[branch.index()] = true;
            }
        }

        reserved
    }

    fn research_branch_available(&self, runtime: &CountryRuntime, branch: ResearchBranch) -> bool {
        if self.scenario.technology_tree.is_empty() {
            return true;
        }

        self.scenario
            .technology_tree
            .next_available(
                branch,
                &runtime.completed_technologies,
                runtime
                    .research_slots
                    .iter()
                    .filter_map(|slot| slot.technology),
            )
            .is_some()
    }

    fn next_law_action(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        date: GameDate,
    ) -> Option<LawAction> {
        if !node
            .runtime
            .country
            .can_spend_political_power(150 * crate::sim::POLITICAL_POWER_UNIT)
        {
            return None;
        }

        match phase {
            StrategicPhase::PrePivot => {
                if matches!(
                    node.runtime.country.laws.economy,
                    crate::domain::EconomyLaw::CivilianEconomy
                ) {
                    return Some(LawAction {
                        date,
                        target: LawTarget::Economy(crate::domain::EconomyLaw::EarlyMobilization),
                    });
                }

                if matches!(
                    node.runtime.country.laws.trade,
                    crate::domain::TradeLaw::ExportFocus
                ) {
                    return Some(LawAction {
                        date,
                        target: LawTarget::Trade(crate::domain::TradeLaw::LimitedExports),
                    });
                }

                None
            }
            StrategicPhase::PostPivot => {
                if !matches!(
                    node.runtime.country.laws.mobilization,
                    crate::domain::MobilizationLaw::ExtensiveConscription
                ) {
                    return Some(LawAction {
                        date,
                        target: LawTarget::Mobilization(
                            crate::domain::MobilizationLaw::ExtensiveConscription,
                        ),
                    });
                }

                None
            }
        }
    }

    fn next_advisor_action(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        date: GameDate,
    ) -> Option<AdvisorAction> {
        if !node
            .runtime
            .country
            .can_spend_political_power(150 * crate::sim::POLITICAL_POWER_UNIT)
        {
            return None;
        }

        match phase {
            StrategicPhase::PrePivot => {
                if !node.runtime.advisors.industry {
                    return Some(AdvisorAction {
                        date,
                        kind: AdvisorKind::IndustryConcern,
                    });
                }

                if !node.runtime.advisors.research {
                    return Some(AdvisorAction {
                        date,
                        kind: AdvisorKind::ResearchInstitute,
                    });
                }

                None
            }
            StrategicPhase::PostPivot => {
                if !node.runtime.advisors.military_industry {
                    return Some(AdvisorAction {
                        date,
                        kind: AdvisorKind::MilitaryIndustrialist,
                    });
                }

                None
            }
        }
    }

    fn next_production_action(
        &self,
        node: &PlannerNode,
        date: GameDate,
    ) -> Option<crate::sim::ProductionAction> {
        let (demand_gap, desired_allocation) = self.compute_production_demand_and_allocation(node);
        let equipment =
            self.select_production_equipment(&node.runtime, &demand_gap, &desired_allocation)?;
        self.resolve_production_slot(node, date, equipment, &demand_gap, &desired_allocation)
    }

    fn compute_production_demand_and_allocation(
        &self,
        node: &PlannerNode,
    ) -> (EquipmentDemand, EquipmentFactoryAllocation) {
        let minimum_supported = self.scenario.force_goal.division_band().min;
        let current_supported = node.runtime.supported_divisions(
            self.scenario.force_plan.template.per_division_demand(),
            &self.scenario.ideas,
        );
        if current_supported < minimum_supported {
            let shortfall = self.readiness_shortfall_demand(&node.runtime);
            let days_remaining = u16::try_from(
                node.runtime
                    .country
                    .date
                    .days_until(self.scenario.milestones[3].date)
                    .max(1),
            )
            .unwrap_or(u16::MAX);
            (
                shortfall,
                self.factory_allocation_for_demand(
                    shortfall,
                    node.runtime.equipment_profiles,
                    node.runtime.military_output_bp(&self.scenario.ideas),
                    days_remaining,
                ),
            )
        } else {
            (
                self.scenario
                    .force_plan
                    .stockpile_target_demand
                    .saturating_sub(EquipmentDemand {
                        infantry_equipment: node.runtime.stockpile.infantry_equipment,
                        support_equipment: node.runtime.stockpile.support_equipment,
                        artillery: node.runtime.stockpile.artillery,
                        anti_tank: node.runtime.stockpile.anti_tank,
                        anti_air: node.runtime.stockpile.anti_air,
                        motorized_equipment: node.runtime.stockpile.motorized_equipment,
                        armor: node.runtime.stockpile.armor,
                        fighters: node.runtime.stockpile.fighters,
                        bombers: node.runtime.stockpile.bombers,
                        manpower: 0,
                    }),
                self.scenario.force_plan.factory_allocation,
            )
        }
    }

    fn select_production_equipment(
        &self,
        runtime: &CountryRuntime,
        demand_gap: &EquipmentDemand,
        desired_allocation: &EquipmentFactoryAllocation,
    ) -> Option<EquipmentKind> {
        [
            EquipmentKind::InfantryEquipment,
            EquipmentKind::SupportEquipment,
            EquipmentKind::Artillery,
            EquipmentKind::AntiTank,
            EquipmentKind::AntiAir,
            EquipmentKind::MotorizedEquipment,
            EquipmentKind::Armor,
            EquipmentKind::Fighter,
            EquipmentKind::Bomber,
        ]
        .into_iter()
        .filter(|equipment| demand_gap.get(*equipment) > 0)
        .max_by_key(|equipment| {
            let target_factories = desired_allocation.get(*equipment);
            let assigned_factories = self.assigned_factories_for_equipment(runtime, *equipment);
            let stockpile_gap = demand_gap.get(*equipment);
            (
                target_factories.saturating_sub(assigned_factories),
                stockpile_gap,
            )
        })
    }

    fn resolve_production_slot(
        &self,
        node: &PlannerNode,
        date: GameDate,
        equipment: EquipmentKind,
        demand_gap: &EquipmentDemand,
        desired_allocation: &EquipmentFactoryAllocation,
    ) -> Option<crate::sim::ProductionAction> {
        let current_assigned = self.assigned_factories_for_equipment(&node.runtime, equipment);
        let target_factories = desired_allocation
            .get(equipment)
            .max(current_assigned.saturating_add(1));
        let unassigned = node.runtime.unassigned_military_factories();

        if let Some(slot) = node
            .runtime
            .production_lines
            .iter()
            .position(|line| line.equipment == equipment)
        {
            let current_factories = node.runtime.production_lines[slot].factories;
            let factories = u8::try_from(
                u16::from(current_factories)
                    .saturating_add(unassigned.min(2))
                    .min(target_factories),
            )
            .unwrap_or(u8::MAX);
            if factories != current_factories {
                return Some(crate::sim::ProductionAction {
                    date,
                    slot: u8::try_from(slot).unwrap_or(u8::MAX),
                    equipment,
                    factories,
                });
            }
        }

        let donor_slot =
            self.production_donor_slot(node, equipment, *desired_allocation, *demand_gap)?;
        let donor_line = node.runtime.production_lines[donor_slot];
        let needed_factories = target_factories
            .saturating_sub(current_assigned)
            .max(u16::from(donor_line.factories).min(1));
        let factories = u8::try_from(
            u16::from(donor_line.factories)
                .saturating_add(unassigned.min(2))
                .min(needed_factories.max(1)),
        )
        .unwrap_or(u8::MAX);

        Some(crate::sim::ProductionAction {
            date,
            slot: u8::try_from(donor_slot).unwrap_or(u8::MAX),
            equipment,
            factories,
        })
    }

    fn assigned_factories_for_equipment(
        &self,
        runtime: &CountryRuntime,
        equipment: EquipmentKind,
    ) -> u16 {
        runtime
            .production_lines
            .iter()
            .filter(|line| line.equipment == equipment)
            .map(|line| u16::from(line.factories))
            .sum()
    }

    fn production_donor_slot(
        &self,
        node: &PlannerNode,
        target: EquipmentKind,
        desired_allocation: EquipmentFactoryAllocation,
        demand_gap: EquipmentDemand,
    ) -> Option<usize> {
        node.runtime
            .production_lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.equipment != target && line.factories > 0)
            .filter(|(_, line)| {
                let assigned = self.assigned_factories_for_equipment(&node.runtime, line.equipment);
                let desired = desired_allocation.get(line.equipment);
                line.equipment == EquipmentKind::Unmodeled
                    || assigned > desired
                    || demand_gap.get(line.equipment) == 0
            })
            .max_by_key(|(_, line)| {
                let assigned = self.assigned_factories_for_equipment(&node.runtime, line.equipment);
                let desired = desired_allocation.get(line.equipment);
                (
                    line.equipment == EquipmentKind::Unmodeled,
                    assigned.saturating_sub(desired),
                    node.runtime.stockpile.get(line.equipment),
                    line.factories,
                )
            })
            .map(|(slot, _)| slot)
    }

    fn readiness_shortfall_demand(&self, runtime: &CountryRuntime) -> EquipmentDemand {
        let mut remaining_stockpile = runtime.stockpile;
        let mut ready_divisions = 0_u16;
        let target_ready = self.scenario.force_goal.division_band().min;
        let mut shortfall = EquipmentDemand::default();

        for division in runtime.fielded_force.iter() {
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
                self.scenario
                    .force_plan
                    .template
                    .per_division_demand()
                    .without_manpower()
                    .scale(target_ready.saturating_sub(ready_divisions)),
            );
        }

        shortfall
    }

    fn military_base_covers_readiness_shortfall(&self, runtime: &CountryRuntime) -> bool {
        let shortfall = self.readiness_shortfall_demand(runtime);
        if !shortfall.has_equipment() {
            return true;
        }

        let days_remaining = u16::try_from(
            runtime
                .country
                .date
                .days_until(self.scenario.milestones[3].date)
                .max(1),
        )
        .unwrap_or(u16::MAX);
        let allocation = self.factory_allocation_for_demand(
            shortfall,
            runtime.equipment_profiles,
            runtime.military_output_bp(&self.scenario.ideas),
            days_remaining,
        );
        runtime.total_military_factories() >= allocation.total()
    }

    fn factory_allocation_for_demand(
        &self,
        demand: EquipmentDemand,
        equipment_profiles: crate::domain::ModeledEquipmentProfiles,
        output_bonus_bp: u16,
        days_remaining: u16,
    ) -> EquipmentFactoryAllocation {
        let factory_capacity_centi = self
            .estimated_factory_capacity_centi(days_remaining, output_bonus_bp)
            .max(1);
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

    fn estimated_factory_capacity_centi(&self, days: u16, output_bonus_bp: u16) -> u64 {
        let config = self.simulator.config;
        let mut efficiency = 100_u16;
        let mut total = 0_u64;
        let output_multiplier_bp = 10_000_u64 + u64::from(output_bonus_bp);

        for _ in 0..days {
            let daily_output = u64::from(config.production_output_centi_per_factory)
                * u64::from(efficiency)
                / 1_000;
            total += daily_output * output_multiplier_bp / 10_000;
            if efficiency < config.production_efficiency_cap_permille {
                efficiency = (efficiency + config.production_efficiency_gain_permille)
                    .min(config.production_efficiency_cap_permille);
            }
        }

        total
    }

    fn next_construction_action(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        date: GameDate,
        pending_actions: &[Action],
    ) -> Option<ConstructionAction> {
        match phase {
            StrategicPhase::PrePivot => match node.template {
                StrategyTemplateKind::InfraAssisted => self
                    .next_infrastructure_action(node, date, pending_actions)
                    .or_else(|| self.next_civilian_action(node, date, pending_actions)),
                StrategyTemplateKind::CivFirst => {
                    self.next_civilian_action(node, date, pending_actions)
                }
                StrategyTemplateKind::EarlyMilitaryPivot => self
                    .next_military_action(node, date, pending_actions)
                    .or_else(|| self.next_civilian_action(node, date, pending_actions)),
            },
            StrategicPhase::PostPivot => {
                let minimum_force_target_met =
                    node.runtime.supported_divisions(
                        self.scenario.force_plan.template.per_division_demand(),
                        &self.scenario.ideas,
                    ) >= self.scenario.force_goal.fort_construction_division_floor();
                let frontier_fort_priority = date >= self.scenario.milestones[2].date
                    || self.military_base_covers_readiness_shortfall(&node.runtime);
                let frontier_forts_complete = node
                    .runtime
                    .frontier_forts_complete(&self.scenario.frontier_forts);
                if !frontier_forts_complete && (minimum_force_target_met || frontier_fort_priority)
                {
                    self.next_fort_action(node, date, pending_actions)
                } else if node.runtime.total_military_factories()
                    < self.scenario.force_plan.required_military_factories
                {
                    self.next_military_action(node, date, pending_actions)
                } else {
                    self.next_fort_action(node, date, pending_actions)
                }
            }
        }
    }

    fn next_civilian_action(
        &self,
        node: &PlannerNode,
        date: GameDate,
        pending_actions: &[Action],
    ) -> Option<ConstructionAction> {
        self.scenario
            .economic_construction_order
            .iter()
            .copied()
            .find(|state| self.state_accepts_factory(node, *state, pending_actions))
            .map(|state| ConstructionAction {
                date,
                state,
                kind: ConstructionKind::CivilianFactory,
            })
    }

    fn next_infrastructure_action(
        &self,
        node: &PlannerNode,
        date: GameDate,
        pending_actions: &[Action],
    ) -> Option<ConstructionAction> {
        self.scenario
            .infrastructure_order
            .iter()
            .copied()
            .find(|state| self.state_accepts_infrastructure(node, *state, pending_actions))
            .map(|state| ConstructionAction {
                date,
                state,
                kind: ConstructionKind::Infrastructure,
            })
    }

    fn next_military_action(
        &self,
        node: &PlannerNode,
        date: GameDate,
        pending_actions: &[Action],
    ) -> Option<ConstructionAction> {
        self.scenario
            .military_construction_order
            .iter()
            .copied()
            .find(|state| self.state_accepts_factory(node, *state, pending_actions))
            .map(|state| ConstructionAction {
                date,
                state,
                kind: ConstructionKind::MilitaryFactory,
            })
    }

    fn next_fort_action(
        &self,
        node: &PlannerNode,
        date: GameDate,
        pending_actions: &[Action],
    ) -> Option<ConstructionAction> {
        self.scenario
            .frontier_fort_order
            .iter()
            .copied()
            .find(|state| self.state_accepts_fort(node, *state, pending_actions))
            .map(|state| ConstructionAction {
                date,
                state,
                kind: ConstructionKind::LandFort,
            })
    }

    fn state_accepts_factory(
        &self,
        node: &PlannerNode,
        state: crate::sim::StateId,
        pending_actions: &[Action],
    ) -> bool {
        let definition = &node.runtime.state_defs[usize::from(state.0)];
        let runtime = node.runtime.state(state);
        let pending_for_state = node.runtime.queued_factory_projects(state)
            + pending_actions
                .iter()
                .filter(|action| {
                    matches!(
                        action,
                        Action::Construction(ConstructionAction {
                            state: pending_state,
                            kind: ConstructionKind::CivilianFactory | ConstructionKind::MilitaryFactory,
                            ..
                        }) if *pending_state == state
                    )
                })
                .count() as u8;

        runtime.free_slots(definition) > pending_for_state
    }

    fn state_accepts_infrastructure(
        &self,
        node: &PlannerNode,
        state: crate::sim::StateId,
        pending_actions: &[Action],
    ) -> bool {
        let definition = &node.runtime.state_defs[usize::from(state.0)];
        let runtime = node.runtime.state(state);
        let queued = node
            .runtime
            .queued_kind_projects(state, ConstructionKind::Infrastructure)
            + pending_actions
                .iter()
                .filter(|action| {
                    matches!(
                        action,
                        Action::Construction(ConstructionAction {
                            state: pending_state,
                            kind: ConstructionKind::Infrastructure,
                            ..
                        }) if *pending_state == state
                    )
                })
                .count() as u8;

        runtime.infrastructure + queued < definition.infrastructure_target
    }

    fn state_accepts_fort(
        &self,
        node: &PlannerNode,
        state: crate::sim::StateId,
        pending_actions: &[Action],
    ) -> bool {
        let runtime = node.runtime.state(state);
        let queued = node
            .runtime
            .queued_kind_projects(state, ConstructionKind::LandFort)
            + pending_actions
                .iter()
                .filter(|action| {
                    matches!(
                        action,
                        Action::Construction(ConstructionAction {
                            state: pending_state,
                            kind: ConstructionKind::LandFort,
                            ..
                        }) if *pending_state == state
                    )
                })
                .count() as u8;

        runtime.land_fort_level + queued < 5
    }

    fn phase(&self, node: &PlannerNode, date: GameDate) -> StrategicPhase {
        if date < node.pivot_date {
            StrategicPhase::PrePivot
        } else {
            StrategicPhase::PostPivot
        }
    }

    fn score(&self, runtime: &CountryRuntime) -> i64 {
        let force_plan = self.scenario.force_plan;
        let civilian = i64::from(runtime.total_civilian_factories());
        let military = i64::from(runtime.total_military_factories());
        let ready_divisions = i64::from(runtime.supported_divisions(
            force_plan.template.per_division_demand(),
            &self.scenario.ideas,
        ));
        let completed_focuses = i64::try_from(runtime.completed_focuses.len()).unwrap_or(i64::MAX);
        let completed_research = i64::from(
            runtime.completed_research.industry
                + runtime.completed_research.construction
                + runtime.completed_research.electronics
                + runtime.completed_research.production,
        );
        let stockpile_gap = [
            EquipmentKind::InfantryEquipment,
            EquipmentKind::SupportEquipment,
            EquipmentKind::Artillery,
            EquipmentKind::AntiTank,
            EquipmentKind::AntiAir,
            EquipmentKind::MotorizedEquipment,
            EquipmentKind::Armor,
            EquipmentKind::Fighter,
            EquipmentKind::Bomber,
        ]
        .into_iter()
        .map(|equipment| {
            i64::from(
                force_plan
                    .stockpile_target_demand
                    .get(equipment)
                    .saturating_sub(runtime.stockpile.get(equipment)),
            )
        })
        .sum::<i64>();
        let factory_gap = i64::from(
            force_plan
                .required_military_factories
                .saturating_sub(runtime.total_military_factories()),
        );
        let available_resources = runtime.domestic_resources(&self.scenario.ideas);
        let current_resource_use = runtime.daily_resource_demand(runtime.equipment_profiles);
        let resource_fulfillment_bp = current_resource_use.fulfillment_bp(available_resources);
        let resource_utilization = i64::from(
            current_resource_use
                .scale_bp(resource_fulfillment_bp)
                .utilization_bp(available_resources),
        );
        let resource_fulfillment = i64::from(resource_fulfillment_bp);
        let minimum_ready_gap =
            (i64::from(self.scenario.force_goal.division_band().min) - ready_divisions).max(0);
        let manpower_headroom = runtime
            .available_manpower(&self.scenario.ideas)
            .saturating_sub(u64::from(force_plan.frontline_demand.manpower));

        let mut score = 0_i64;
        score += civilian * i64::from(self.weights.civilian_growth) * 100;
        score += military * i64::from(self.weights.military_factories) * 120;
        score += ready_divisions * i64::from(self.weights.military_output) * 250;
        score += civilian * i64::from(self.strategic_goals.industry) * 20;
        score += ready_divisions * i64::from(self.strategic_goals.readiness) * 60;
        score += completed_focuses * i64::from(self.strategic_goals.politics) * 120;
        score += completed_research * i64::from(self.strategic_goals.research) * 100;
        score += resource_utilization * 3;
        score += resource_fulfillment * 3;
        score += i64::try_from(manpower_headroom / 1_000).unwrap_or(i64::MAX) * 2;

        if runtime.country.date >= self.scenario.milestones[0].date {
            score += civilian * 50;
        }
        if runtime.country.date >= self.scenario.milestones[1].date {
            score += civilian * 75;
        }
        if runtime.country.date >= self.scenario.milestones[2].date {
            score -= factory_gap.max(0) * 800;
            score -= stockpile_gap * 4;
            score -= minimum_ready_gap * 25_000;
        }
        if runtime.country.date >= self.scenario.milestones[3].date {
            score -= minimum_ready_gap * 50_000;
            if runtime.frontier_forts_complete(&self.scenario.frontier_forts) {
                score += 20_000;
            } else {
                score -= 20_000;
            }
        }

        score
    }

    fn hard_requirements_met(&self, runtime: &CountryRuntime) -> bool {
        runtime.frontier_forts_complete(&self.scenario.frontier_forts)
            && runtime.supported_divisions(
                self.scenario.force_plan.template.per_division_demand(),
                &self.scenario.ideas,
            ) >= self.scenario.force_goal.division_band().min
            && self
                .scenario
                .hard_focus_goals
                .iter()
                .all(|goal| runtime.completed_focus_by(&goal.id, goal.deadline))
    }

    fn repair_hard_requirements(
        &self,
        plan: &PlannedSolution,
    ) -> Result<Option<PlannedSolution>, SimulationError> {
        let mut repaired_actions = plan.actions.clone();
        let candidate_indices = repaired_actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| match *action {
                Action::Construction(ConstructionAction { date, kind, .. })
                    if kind != ConstructionKind::LandFort && date >= plan.pivot_date =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for index in candidate_indices.into_iter().rev() {
            let Action::Construction(action) = repaired_actions[index] else {
                continue;
            };
            let Some(replacement) =
                self.repair_fort_action(&repaired_actions, index, plan.pivot_date)?
            else {
                continue;
            };
            if replacement == action {
                continue;
            }

            let original = repaired_actions[index].clone();
            repaired_actions[index] = Action::Construction(replacement);
            let Ok(repaired_plan) =
                self.evaluate_actions(plan.template, plan.pivot_date, repaired_actions.clone())
            else {
                repaired_actions[index] = original;
                continue;
            };
            if self.hard_requirements_met(&repaired_plan.final_state) {
                return Ok(Some(repaired_plan));
            }
            // Keep the replacement even if hard requirements aren't met yet —
            // subsequent iterations may fix remaining gaps.
        }

        Ok(None)
    }

    fn repair_fort_action(
        &self,
        actions: &[Action],
        replace_index: usize,
        pivot_date: GameDate,
    ) -> Result<Option<ConstructionAction>, SimulationError> {
        let Action::Construction(candidate) = actions[replace_index] else {
            return Ok(None);
        };
        if candidate.kind == ConstructionKind::LandFort || candidate.date < pivot_date {
            return Ok(None);
        }

        let runtime = self.runtime_before_date(actions, candidate.date, pivot_date)?;
        let same_day_actions = actions
            .iter()
            .enumerate()
            .filter(|(index, action)| *index != replace_index && action.date() == candidate.date)
            .map(|(_, action)| action)
            .cloned()
            .collect::<Vec<_>>();
        let node = PlannerNode {
            template: StrategyTemplateKind::EarlyMilitaryPivot,
            pivot_date,
            actions: Vec::new(),
            runtime,
            score: 0,
        };

        Ok(self.next_fort_action(&node, candidate.date, &same_day_actions))
    }

    fn runtime_before_date(
        &self,
        actions: &[Action],
        date: GameDate,
        pivot_date: GameDate,
    ) -> Result<CountryRuntime, SimulationError> {
        if date == self.scenario.start_date {
            return Ok(self.scenario.bootstrap_runtime());
        }

        let prefix = actions
            .iter()
            .filter(|action| action.date() < date)
            .cloned()
            .collect::<Vec<_>>();
        let outcome = self.simulator.simulate(
            &self.scenario,
            self.scenario.bootstrap_runtime(),
            &prefix,
            date.previous_day(),
            pivot_date,
        )?;

        Ok(outcome.country)
    }

    fn evaluate_actions(
        &self,
        template: StrategyTemplateKind,
        pivot_date: GameDate,
        actions: Vec<Action>,
    ) -> Result<PlannedSolution, SimulationError> {
        let outcome = self.simulator.simulate(
            &self.scenario,
            self.scenario.bootstrap_runtime(),
            &actions,
            self.scenario.milestones[3].date,
            pivot_date,
        )?;

        Ok(PlannedSolution {
            template,
            pivot_date,
            score: self.score(&outcome.country),
            actions,
            final_state: outcome.country,
        })
    }
}

fn min_date(left: GameDate, right: GameDate) -> GameDate {
    if left <= right { left } else { right }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        FieldedDivision, FocusCondition, FocusEffect, GameDate, HardFocusGoal, NationalFocus,
        StrategicGoalWeights,
    };
    use crate::scenario::France1936Scenario;
    use crate::sim::{
        Action, ConstructionKind, ResearchBranch, SimulationConfig, SimulationEngine,
        SimulationError, Stockpile, StrategicPhase,
    };

    use super::{FranceBeamPlanner, StrategyTemplateKind};

    #[test]
    fn france_beam_planner_produces_a_plan_to_the_final_milestone() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::new(SimulationConfig {
                civilian_factory_cost_centi: 200_000,
                military_factory_cost_centi: 180_000,
                infrastructure_cost_centi: 90_000,
                land_fort_cost_centi: 90_000,
                ..SimulationConfig::default()
            }),
            crate::solver::BeamSearchConfig::new(8, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.plan().unwrap();

        assert!(matches!(
            plan.template,
            StrategyTemplateKind::CivFirst
                | StrategyTemplateKind::InfraAssisted
                | StrategyTemplateKind::EarlyMilitaryPivot
        ));
        assert_eq!(plan.final_state.country.date, scenario.milestones[3].date);
        assert!(!plan.actions.is_empty());
    }

    #[test]
    fn france_beam_planner_plan_replays_to_the_same_final_state() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );
        let plan = planner.plan().unwrap();
        let replay = SimulationEngine::default()
            .simulate(
                &scenario,
                scenario.bootstrap_runtime(),
                &plan.actions,
                scenario.milestones[3].date,
                plan.pivot_date,
            )
            .unwrap();

        assert_eq!(replay.country, plan.final_state);
    }

    #[test]
    fn france_beam_planner_respects_the_pivot_window() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.plan().unwrap();

        assert!(scenario.pivot_window.contains(plan.pivot_date));
    }

    #[test]
    fn france_beam_planner_accepts_custom_strategic_goals() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario,
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        )
        .with_strategic_goals(StrategicGoalWeights::new(4, 10, 6, 7));

        assert_eq!(
            planner.strategic_goals,
            StrategicGoalWeights::new(4, 10, 6, 7)
        );
    }

    #[test]
    fn france_beam_planner_assigns_distinct_research_branches_with_two_open_slots() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario,
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );
        let node = planner
            .seed_nodes(planner.config)
            .into_iter()
            .next()
            .expect("planner seeds nodes");
        let research_actions = planner
            .generate_window_actions(&node)
            .into_iter()
            .filter_map(|action| match action {
                Action::Research(action) => Some(action.branch),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(research_actions.len(), 2);
        assert_ne!(research_actions[0], research_actions[1]);
        assert!(research_actions.contains(&ResearchBranch::Construction));
    }

    #[test]
    fn france_beam_planner_returns_hard_requirement_compliant_plan_when_feasible() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::new(SimulationConfig {
                civilian_factory_cost_centi: 200_000,
                military_factory_cost_centi: 180_000,
                infrastructure_cost_centi: 90_000,
                land_fort_cost_centi: 20_000,
                ..SimulationConfig::default()
            }),
            crate::solver::BeamSearchConfig::new(16, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.plan().unwrap();

        assert!(
            plan.final_state
                .frontier_forts_complete(&scenario.frontier_forts)
        );
        assert!(
            plan.final_state.supported_divisions(
                scenario.force_plan.template.per_division_demand(),
                &scenario.ideas,
            ) >= scenario.force_goal.division_band().min
        );
    }

    #[test]
    fn post_pivot_construction_switches_to_forts_once_current_military_base_can_close_gap() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );
        let demand = scenario.force_plan.template.per_division_demand();
        let mut fielded_force = vec![
            FieldedDivision {
                target_demand: demand,
                equipped_demand: demand
            };
            72
        ];
        fielded_force[0].equipped_demand.infantry_equipment -= 1;

        let mut runtime = scenario
            .bootstrap_runtime()
            .with_exact_fielded_force(fielded_force.into_boxed_slice());
        runtime.country.date = scenario.pivot_window.start.next_day();
        runtime.stockpile = Stockpile::default();

        let node = super::PlannerNode {
            template: StrategyTemplateKind::EarlyMilitaryPivot,
            pivot_date: scenario.pivot_window.start,
            actions: Vec::new(),
            runtime,
            score: 0,
        };

        let action = planner
            .next_construction_action(
                &node,
                StrategicPhase::PostPivot,
                node.runtime.country.date,
                &[],
            )
            .expect("planner chooses a post-pivot construction action");

        assert_eq!(action.kind, ConstructionKind::LandFort);
    }

    #[test]
    fn france_beam_planner_fails_when_hard_requirements_are_impossible() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario,
            SimulationEngine::new(SimulationConfig {
                land_fort_cost_centi: 4_000_000_000,
                ..SimulationConfig::default()
            }),
            crate::solver::BeamSearchConfig::new(8, 35),
            crate::solver::PlannerWeights::default(),
        );

        let result = planner.plan();

        assert_eq!(result, Err(SimulationError::HardRequirementsUnsatisfied));
    }

    #[test]
    fn france_beam_planner_satisfies_feasible_hard_focus_goals() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_industrial_modernization".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                effects: vec![FocusEffect::AddPoliticalPower(10)],
            }],
            Vec::new(),
            vec![HardFocusGoal {
                id: "FRA_industrial_modernization".into(),
                deadline: GameDate::new(1936, 1, 1),
            }],
        );
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.plan().unwrap();

        assert!(
            plan.final_state
                .completed_focus_by("FRA_industrial_modernization", GameDate::new(1936, 1, 1))
        );
        assert!(plan.actions.iter().any(|action| {
            matches!(
                action,
                Action::Focus(action) if action.focus_id.as_ref() == "FRA_industrial_modernization"
            )
        }));
    }

    #[test]
    fn france_beam_planner_fails_impossible_hard_focus_deadlines() {
        let scenario = France1936Scenario::standard().with_exact_focus_data(
            2,
            Vec::new(),
            Vec::new(),
            vec![NationalFocus {
                id: "FRA_long_industrial_program".into(),
                days: 2,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                effects: vec![FocusEffect::AddPoliticalPower(10)],
            }],
            Vec::new(),
            vec![HardFocusGoal {
                id: "FRA_long_industrial_program".into(),
                deadline: GameDate::new(1936, 1, 1),
            }],
        );
        let planner = FranceBeamPlanner::new(
            scenario,
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );

        let result = planner.plan();

        assert_eq!(result, Err(SimulationError::HardRequirementsUnsatisfied));
    }

    #[test]
    fn france_best_effort_plan_produces_a_valid_plan() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.best_effort_plan().unwrap();

        assert!(!plan.actions.is_empty());
        assert_eq!(plan.final_state.country.date, scenario.milestones[3].date);
        assert!(scenario.pivot_window.contains(plan.pivot_date));
    }

    #[test]
    fn france_plan_includes_production_actions_post_pivot() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario.clone(),
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.plan().unwrap();

        let has_production = plan
            .actions
            .iter()
            .any(|a| matches!(a, Action::Production(_)));
        assert!(
            has_production,
            "plan should include production actions for equipment"
        );
    }

    #[test]
    fn france_plan_includes_research_actions() {
        let scenario = France1936Scenario::standard();
        let planner = FranceBeamPlanner::new(
            scenario,
            SimulationEngine::default(),
            crate::solver::BeamSearchConfig::new(4, 35),
            crate::solver::PlannerWeights::default(),
        );

        let plan = planner.plan().unwrap();

        let research_actions: Vec<_> = plan
            .actions
            .iter()
            .filter(|a| matches!(a, Action::Research(_)))
            .collect();
        assert!(
            research_actions.len() >= 2,
            "plan should include at least 2 research actions, got {}",
            research_actions.len()
        );
    }
}
