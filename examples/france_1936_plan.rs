use hoi4simulator::scenario::France1936Scenario;
use hoi4simulator::sim::{SimulationConfig, SimulationEngine};
use hoi4simulator::solver::{BeamSearchConfig, FranceBeamPlanner, PlannerWeights};

fn main() {
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
        BeamSearchConfig::new(8, 35),
        PlannerWeights::default(),
    );

    let plan = planner.plan().expect("France 1936 planning should succeed");
    let per_division_demand = scenario.canonical_template.per_division_demand();
    let ready_divisions = plan.final_state.ready_divisions(per_division_demand);

    println!("scenario: {} 1936", scenario.reference_tag);
    println!("template: {:?}", plan.template);
    println!("pivot date: {}", plan.pivot_date);
    println!("score: {}", plan.score);
    println!("actions: {}", plan.actions.len());
    println!("final date: {}", plan.final_state.country.date);
    println!(
        "factories: {} civilian, {} military",
        plan.final_state.total_civilian_factories(),
        plan.final_state.total_military_factories()
    );
    println!("ready divisions: {}", ready_divisions);
    println!(
        "frontier forts complete: {}",
        plan.final_state
            .frontier_forts_complete(&scenario.frontier_forts)
    );
    println!("first actions:");

    for action in plan.actions.iter().take(12) {
        println!("  - {:?}", action);
    }

    if plan.actions.len() > 12 {
        println!("  - ... {} more actions", plan.actions.len() - 12);
    }
}
