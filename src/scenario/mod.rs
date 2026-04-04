pub mod france_1936;

use crate::domain::{EquipmentDemand, GameDate, Milestone, PivotWindow};
use crate::sim::CountryRuntime;

pub trait CountryScenario {
    fn reference_tag(&self) -> &'static str;
    fn start_date(&self) -> GameDate;
    fn pivot_window(&self) -> PivotWindow;
    fn milestones(&self) -> &[Milestone];
    fn bootstrap_runtime(&self) -> CountryRuntime;
    fn readiness_demand_for(&self, divisions: u16) -> EquipmentDemand;
}

pub use france_1936::{France1936Scenario, Frontier, FrontierFortRequirement};
