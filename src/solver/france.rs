use arrayvec::ArrayVec;

use crate::domain::{EquipmentKind, GameDate, NationalFocus, StrategicGoalWeights};
use crate::scenario::France1936Scenario;
use crate::sim::{
    Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, CountryRuntime,
    FocusAction, LawAction, LawTarget, ResearchAction, ResearchBranch, SimulationEngine,
    SimulationError, StrategicPhase,
};

use super::{BeamSearchConfig, PlannerWeights};

const MAX_PLANNED_ACTIONS: usize = 256;
const MAX_WINDOW_ACTIONS: usize = 32;

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

    fn search(&self, config: BeamSearchConfig) -> Result<PlannedSolution, SimulationError> {
        let end_date = self.scenario.milestones[3].date;
        let mut frontier = self.seed_nodes(config);
        let mut next_frontier = Vec::with_capacity(frontier.len().max(config.beam_width));

        while frontier
            .iter()
            .any(|node| node.runtime.country.date < end_date)
        {
            next_frontier.clear();

            for node in frontier.drain(..) {
                if node.runtime.country.date >= end_date {
                    next_frontier.push(node);
                    continue;
                }

                let mut window_actions = self.generate_window_actions(&node);
                let window_end = min_date(
                    node.runtime.country.date.add_days(config.replan_days),
                    end_date,
                );
                let PlannerNode {
                    template,
                    pivot_date,
                    actions,
                    runtime,
                    score: _,
                } = node;

                let outcome = self.simulator.simulate(
                    &self.scenario,
                    runtime,
                    window_actions.as_slice(),
                    window_end,
                    pivot_date,
                )?;
                let mut child_actions = actions;
                assert!(child_actions.len() + window_actions.len() <= MAX_PLANNED_ACTIONS);
                child_actions.extend(window_actions.drain(..));

                next_frontier.push(PlannerNode {
                    template,
                    pivot_date,
                    actions: child_actions,
                    score: self.score(&outcome.country),
                    runtime: outcome.country,
                });
            }

            next_frontier.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then_with(|| left.template.cmp(&right.template))
                    .then_with(|| left.pivot_date.cmp(&right.pivot_date))
            });
            next_frontier.truncate(config.beam_width);
            std::mem::swap(&mut frontier, &mut next_frontier);
        }

        let best = frontier
            .into_iter()
            .max_by(|left, right| left.score.cmp(&right.score))
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
                    actions: Vec::with_capacity(MAX_PLANNED_ACTIONS),
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

        dates.sort();
        dates.dedup();
        dates
    }

    fn generate_window_actions(&self, node: &PlannerNode) -> ArrayVec<Action, MAX_WINDOW_ACTIONS> {
        let mut actions = ArrayVec::<Action, MAX_WINDOW_ACTIONS>::new();
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
                    .focus_is_available(&node.runtime, &self.scenario.ideas, focus)
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
            | crate::domain::FocusCondition::HasIdea(_)
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
            crate::domain::FocusEffect::AddManpower(_)
            | crate::domain::FocusEffect::AddPoliticalPower(_)
            | crate::domain::FocusEffect::AddResearchSlot(_)
            | crate::domain::FocusEffect::AddStability(_)
            | crate::domain::FocusEffect::AddWarSupport(_)
            | crate::domain::FocusEffect::AddEquipmentToStockpile { .. }
            | crate::domain::FocusEffect::SetCountryFlag(_) => true,
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
                    crate::domain::FocusEffect::AddIdea(_)
                    | crate::domain::FocusEffect::AddTimedIdea { .. } => 5_000,
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
                    crate::domain::FocusEffect::SetCountryFlag(_) => 500,
                    crate::domain::FocusEffect::SwapIdea { .. } => 4_000,
                    crate::domain::FocusEffect::StateScoped(scope) => scope
                        .operations
                        .iter()
                        .map(|operation| match operation {
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
            .find(|branch| !reserved[branch.index()])
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
        let unassigned = node.runtime.unassigned_military_factories();
        if unassigned == 0 {
            return None;
        }

        let demand = self.scenario.force_plan.stockpile_target_demand;
        let desired_allocation = self.scenario.force_plan.factory_allocation;
        let equipment = [
            EquipmentKind::InfantryEquipment,
            EquipmentKind::SupportEquipment,
            EquipmentKind::Artillery,
            EquipmentKind::AntiTank,
            EquipmentKind::AntiAir,
        ]
        .into_iter()
        .max_by_key(|equipment| {
            let target_factories = desired_allocation.get(*equipment);
            let assigned_factories = node
                .runtime
                .production_lines
                .iter()
                .find(|line| line.equipment == *equipment)
                .map(|line| u16::from(line.factories))
                .unwrap_or(0);
            let stockpile_gap = demand
                .get(*equipment)
                .saturating_sub(node.runtime.stockpile.get(*equipment));

            (
                target_factories.saturating_sub(assigned_factories),
                stockpile_gap,
            )
        })?;

        let slot = node
            .runtime
            .production_lines
            .iter()
            .position(|line| line.equipment == equipment)?;
        let current_factories = node.runtime.production_lines[slot].factories;
        let target_factories = desired_allocation
            .get(equipment)
            .max(u16::from(current_factories) + 1);
        let factories = u8::try_from(
            u16::from(current_factories)
                .saturating_add(unassigned.min(2))
                .min(target_factories),
        )
        .unwrap_or(u8::MAX);

        Some(crate::sim::ProductionAction {
            date,
            slot: u8::try_from(slot).unwrap_or(u8::MAX),
            equipment,
            factories,
        })
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
                StrategyTemplateKind::CivFirst | StrategyTemplateKind::EarlyMilitaryPivot => {
                    self.next_civilian_action(node, date, pending_actions)
                }
            },
            StrategicPhase::PostPivot => {
                let minimum_force_target_met = node.runtime.supported_divisions(
                    self.scenario.force_plan.template.per_division_demand(),
                    &self.scenario.ideas,
                ) >= self.scenario.force_goal.division_band().min;
                if minimum_force_target_met
                    && !node
                        .runtime
                        .frontier_forts_complete(&self.scenario.frontier_forts)
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
        let current_resource_use = [
            EquipmentKind::InfantryEquipment,
            EquipmentKind::SupportEquipment,
            EquipmentKind::Artillery,
            EquipmentKind::AntiTank,
            EquipmentKind::AntiAir,
        ]
        .into_iter()
        .fold(
            crate::domain::ResourceLedger::default(),
            |total, equipment| {
                let assigned = runtime
                    .production_lines
                    .iter()
                    .find(|line| line.equipment == equipment)
                    .map(|line| u16::from(line.factories))
                    .unwrap_or(0);
                total.plus(
                    self.scenario
                        .equipment_profiles
                        .profile(equipment)
                        .resources
                        .scale(assigned),
                )
            },
        );
        let resource_utilization = i64::from(
            current_resource_use.utilization_bp(runtime.domestic_resources(&self.scenario.ideas)),
        );
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
        score += resource_utilization * 6;
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
        }
        if runtime.country.date >= self.scenario.milestones[3].date {
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

            repaired_actions[index] = Action::Construction(replacement);
            let Ok(repaired_plan) =
                self.evaluate_actions(plan.template, plan.pivot_date, repaired_actions.clone())
            else {
                continue;
            };
            if self.hard_requirements_met(&repaired_plan.final_state) {
                return Ok(Some(repaired_plan));
            }
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
        let mut same_day_actions = ArrayVec::<Action, MAX_WINDOW_ACTIONS>::new();
        for (index, action) in actions.iter().enumerate() {
            if index == replace_index || action.date() != candidate.date {
                continue;
            }
            same_day_actions.push(action.clone());
        }
        let node = PlannerNode {
            template: StrategyTemplateKind::EarlyMilitaryPivot,
            pivot_date,
            actions: Vec::new(),
            runtime,
            score: 0,
        };

        Ok(self.next_fort_action(&node, candidate.date, same_day_actions.as_slice()))
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
        FocusCondition, FocusEffect, GameDate, HardFocusGoal, NationalFocus, StrategicGoalWeights,
    };
    use crate::scenario::France1936Scenario;
    use crate::sim::{Action, ResearchBranch, SimulationConfig, SimulationEngine, SimulationError};

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
}
