pub mod calendar;
pub mod laws;
pub mod planning;
pub mod resources;
pub mod templates;

pub use calendar::{GameDate, PivotWindow};
pub use laws::{CountryLaws, EconomyLaw, MobilizationLaw, TradeLaw};
pub use planning::{
    EquipmentFactoryAllocation, ForceGoalSpec, ForcePlan, Milestone, MilestoneKind,
    StrategicGoalWeights, TargetBand,
};
pub use resources::{ResourceKind, ResourceLedger};
pub use templates::{
    DivisionTemplate, EquipmentDemand, EquipmentKind, EquipmentProfile, EquipmentReserveRatios,
    ModeledEquipmentProfiles, SupportCompanies, TemplateDesignConstraints, TemplateFitness,
};
