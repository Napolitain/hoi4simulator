pub mod calendar;
pub mod focus;
pub mod laws;
pub mod planning;
pub mod research;
pub mod resources;
pub mod templates;

pub use calendar::{GameDate, PivotWindow};
pub use focus::{
    DoctrineCostReduction, FocusBuildingKind, FocusCondition, FocusEffect, FocusStateScope,
    HardFocusGoal, IdeaDefinition, IdeaModifiers, NationalFocus, StateCondition, StateOperation,
    StateScopedEffects,
};
pub use laws::{CountryLaws, EconomyLaw, MobilizationLaw, TradeLaw};
pub use planning::{
    EquipmentFactoryAllocation, ForceGoalSpec, ForcePlan, Milestone, MilestoneKind,
    StrategicGoalWeights, TargetBand,
};
pub use research::{
    EquipmentUnlock, ResearchBranch, TechId, TechnologyModifiers, TechnologyNode, TechnologyTree,
};
pub use resources::{ResourceKind, ResourceLedger};
pub use templates::{
    DivisionTemplate, EquipmentDemand, EquipmentKind, EquipmentProfile, EquipmentReserveRatios,
    ModeledEquipmentProfiles, SupportCompanies, TemplateDesignConstraints, TemplateFitness,
};
