pub mod calendar;
pub mod laws;
pub mod planning;
pub mod templates;

pub use calendar::{GameDate, PivotWindow};
pub use laws::{CountryLaws, EconomyLaw, MobilizationLaw, TradeLaw};
pub use planning::{Milestone, MilestoneKind, StrategicGoalWeights, TargetBand};
pub use templates::{
    DivisionTemplate, EquipmentDemand, EquipmentKind, SupportCompanies, TemplateDesignConstraints,
    TemplateFitness,
};
