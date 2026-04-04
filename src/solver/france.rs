use crate::domain::{EquipmentKind, GameDate, StrategicGoalWeights};
use crate::scenario::France1936Scenario;
use crate::sim::{
    Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, CountryRuntime,
    FocusAction, FocusBranch, LawAction, LawTarget, ResearchAction, ResearchBranch,
    SimulationEngine, SimulationError, StrategicPhase,
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
        let end_date = self.scenario.milestones[3].date;
        let mut frontier = self.seed_nodes();

        while frontier
            .iter()
            .any(|node| node.runtime.country.date < end_date)
        {
            let mut next_frontier = Vec::with_capacity(frontier.len());

            for node in frontier.into_iter() {
                if node.runtime.country.date >= end_date {
                    next_frontier.push(node);
                    continue;
                }

                let window_actions = self.generate_window_actions(&node);
                let window_end = min_date(
                    node.runtime.country.date.add_days(self.config.replan_days),
                    end_date,
                );

                let mut child_actions = node.actions.clone();
                child_actions.extend(window_actions.iter().copied());

                let outcome = self.simulator.simulate(
                    &self.scenario,
                    node.runtime.clone(),
                    &window_actions,
                    window_end,
                    node.pivot_date,
                )?;

                next_frontier.push(PlannerNode {
                    template: node.template,
                    pivot_date: node.pivot_date,
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
            next_frontier.truncate(self.config.beam_width);
            frontier = next_frontier;
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

    fn seed_nodes(&self) -> Vec<PlannerNode> {
        let pivot_dates = self.pivot_dates();
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

    fn pivot_dates(&self) -> Vec<GameDate> {
        let mut dates = Vec::new();
        let mut date = self.scenario.pivot_window.start;

        loop {
            dates.push(date);
            if date >= self.scenario.pivot_window.end {
                break;
            }

            let next = date.add_days(self.config.replan_days);
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

    fn generate_window_actions(&self, node: &PlannerNode) -> Vec<Action> {
        let mut actions = Vec::with_capacity(16);
        let date = node.runtime.country.date;
        let phase = self.phase(node, date);
        let mut reserved_research = self.reserved_research_branches(&node.runtime);

        if node.runtime.focus.is_none() {
            actions.push(Action::Focus(FocusAction {
                date,
                branch: self.next_focus_branch(node, phase),
            }));
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

    fn next_focus_branch(&self, node: &PlannerNode, phase: StrategicPhase) -> FocusBranch {
        match phase {
            StrategicPhase::PrePivot => {
                if node.runtime.completed_focuses.economy <= node.runtime.completed_focuses.industry
                {
                    FocusBranch::Economy
                } else {
                    FocusBranch::Industry
                }
            }
            StrategicPhase::PostPivot => FocusBranch::MilitaryIndustry,
        }
    }

    fn next_research_branch(
        &self,
        node: &PlannerNode,
        phase: StrategicPhase,
        reserved: &[bool; ResearchBranch::COUNT],
    ) -> Option<ResearchBranch> {
        let research = node.runtime.completed_research;
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
                    StrategicPhase::PrePivot => 2_u8,
                    StrategicPhase::PostPivot => 3_u8,
                },
            ),
            (
                ResearchBranch::Production,
                research.production,
                match phase {
                    StrategicPhase::PrePivot => 3_u8,
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

        let demand = self
            .scenario
            .readiness_demand_for(self.scenario.readiness_band.min);
        let equipment = [
            EquipmentKind::InfantryEquipment,
            EquipmentKind::SupportEquipment,
            EquipmentKind::Artillery,
            EquipmentKind::AntiTank,
            EquipmentKind::AntiAir,
        ]
        .into_iter()
        .max_by_key(|equipment| {
            let demand_amount = demand.get(*equipment);
            let stockpile_amount = node.runtime.stockpile.get(*equipment);
            demand_amount.saturating_sub(stockpile_amount)
        })?;

        let slot = node
            .runtime
            .production_lines
            .iter()
            .position(|line| line.equipment == equipment)?;
        let factories = node.runtime.production_lines[slot]
            .factories
            .saturating_add(u8::try_from(unassigned.min(2)).unwrap_or(2));

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
                if node.runtime.total_military_factories() < self.scenario.military_factory_target {
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
        let definition = node.runtime.state_defs[usize::from(state.0)];
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
        let definition = node.runtime.state_defs[usize::from(state.0)];
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
        let civilian = i64::from(runtime.total_civilian_factories());
        let military = i64::from(runtime.total_military_factories());
        let ready_divisions = i64::from(
            runtime.ready_divisions(self.scenario.canonical_template.per_division_demand()),
        );
        let completed_focuses = i64::from(
            runtime.completed_focuses.economy
                + runtime.completed_focuses.industry
                + runtime.completed_focuses.military_industry,
        );
        let completed_research = i64::from(
            runtime.completed_research.industry
                + runtime.completed_research.construction
                + runtime.completed_research.electronics
                + runtime.completed_research.production,
        );

        let mut score = 0_i64;
        score += civilian * i64::from(self.weights.civilian_growth) * 100;
        score += military * i64::from(self.weights.military_factories) * 120;
        score += ready_divisions * i64::from(self.weights.military_output) * 250;
        score += civilian * i64::from(self.strategic_goals.industry) * 20;
        score += ready_divisions * i64::from(self.strategic_goals.readiness) * 60;
        score += completed_focuses * i64::from(self.strategic_goals.politics) * 120;
        score += completed_research * i64::from(self.strategic_goals.research) * 100;

        if runtime.country.date >= self.scenario.milestones[0].date {
            score += civilian * 50;
        }
        if runtime.country.date >= self.scenario.milestones[1].date {
            score += civilian * 75;
        }
        if runtime.country.date >= self.scenario.milestones[2].date {
            let military_gap =
                i64::from(self.scenario.military_factory_target).saturating_sub(military);
            let readiness_gap =
                i64::from(self.scenario.readiness_band.min).saturating_sub(ready_divisions);

            score -= military_gap.max(0) * 800;
            score -= readiness_gap.max(0) * 1_200;
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
}

fn min_date(left: GameDate, right: GameDate) -> GameDate {
    if left <= right { left } else { right }
}

#[cfg(test)]
mod tests {
    use crate::domain::StrategicGoalWeights;
    use crate::scenario::France1936Scenario;
    use crate::sim::{Action, ResearchBranch, SimulationConfig, SimulationEngine};

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
            .seed_nodes()
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
}
