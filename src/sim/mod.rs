pub mod actions;
pub mod engine;
pub mod rules;
pub mod state;

pub use actions::{
    Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, FocusAction,
    FocusBranch, LawAction, LawCategory, LawTarget, ProductionAction, ResearchAction,
    ResearchBranch, StateId,
};
pub use engine::{SimulationConfig, SimulationEngine, SimulationError, SimulationOutcome};
pub use rules::{
    ConstructionDecisionContext, FranceHeuristicRules, ProductionDecisionContext, RuleViolation,
};
pub use state::{
    ActiveIdea, AdvisorRoster, CompletedFocus, ConstructionProject, CountryRuntime, CountryState,
    FocusProgress, FrancePlanningState, POLITICAL_POWER_UNIT, ProductionLine, ResearchSlotState,
    ResearchSummary, StateDefinition, StateRuntime, Stockpile, StrategicPhase,
};
