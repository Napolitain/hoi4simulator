use std::env;
use std::process::ExitCode;

use hoi4simulator::data::{DataProfilePaths, load_france_1936_dataset};
use hoi4simulator::scenario::France1936Scenario;
use hoi4simulator::sim::{SimulationConfig, SimulationEngine};
use hoi4simulator::solver::{BeamSearchConfig, FranceBeamPlanner, PlannerWeights};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut profile = "vanilla".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                profile = args
                    .next()
                    .ok_or_else(|| "missing value for --profile".to_string())?;
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(format!("unknown argument: {other}\n\n{}", usage_text()));
            }
        }
    }

    let paths = DataProfilePaths::new(env!("CARGO_MANIFEST_DIR"), profile.clone());
    let dataset = load_france_1936_dataset(&paths).map_err(|error| {
        format!(
            "{error}\n\nRun `cargo run --bin ingest_data -- --game-dir <PATH> --profile {profile}` first."
        )
    })?;
    let warnings = dataset.warnings.clone();
    let scenario = France1936Scenario::from_dataset(dataset).map_err(|error| error.to_string())?;
    let planner = FranceBeamPlanner::new(
        scenario.clone(),
        SimulationEngine::new(SimulationConfig::default()),
        BeamSearchConfig::new(8, 35),
        PlannerWeights::default(),
    );
    let plan = planner
        .plan()
        .map_err(|error| format!("planner failed: {error:?}"))?;
    let per_division_demand = scenario.force_plan.template.per_division_demand();

    println!("profile: {}", profile);
    println!(
        "scenario: {} {}",
        scenario.reference_tag, scenario.start_date.year
    );
    println!("states loaded: {}", scenario.initial_state_defs.len());
    println!("template: {:?}", plan.template);
    println!("force template: {}", scenario.force_plan.template.name);
    println!("pivot date: {}", plan.pivot_date);
    println!("score: {}", plan.score);
    println!("actions: {}", plan.actions.len());
    println!("final date: {}", plan.final_state.country.date);
    println!(
        "factories: {} civilian, {} military",
        plan.final_state.total_civilian_factories(),
        plan.final_state.total_military_factories()
    );
    println!(
        "derived force plan: {} divisions, {} required mils, {} resource utilization bp",
        scenario.force_plan.frontline_divisions,
        scenario.force_plan.required_military_factories,
        scenario.force_plan.resource_utilization_bp
    );
    println!(
        "supported divisions: {}",
        plan.final_state.supported_divisions(per_division_demand)
    );
    println!(
        "frontier forts complete: {}",
        plan.final_state
            .frontier_forts_complete(&scenario.frontier_forts)
    );
    if warnings.is_empty() {
        println!("dataset warnings: none");
    } else {
        println!("dataset warnings:");
        for warning in warnings {
            println!("  - {warning}");
        }
    }
    println!("first actions:");

    for action in plan.actions.iter().take(12) {
        println!("  - {:?}", action);
    }

    if plan.actions.len() > 12 {
        println!("  - ... {} more actions", plan.actions.len() - 12);
    }

    Ok(())
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage: cargo run --bin france_1936 -- [--profile <NAME>]\n\nRuns the France 1936 scenario from the Apache Fory dataset in data/structured/<profile>/."
}
