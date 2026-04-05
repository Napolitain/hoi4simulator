use std::env;
use std::process::ExitCode;

use hoi4simulator::data::{DataProfilePaths, load_france_1936_dataset, load_france_1936_scenario};
use hoi4simulator::domain::{GameDate, HardFocusGoal};
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
    let mut hard_focus_goals = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                profile = args
                    .next()
                    .ok_or_else(|| "missing value for --profile".to_string())?;
            }
            "--hard-focus" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --hard-focus".to_string())?;
                hard_focus_goals.push(parse_hard_focus_goal(&value)?);
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
    let scenario = load_france_1936_scenario(&paths)
        .map_err(|error| error.to_string())?
        .with_hard_focus_goals(hard_focus_goals);
    let planner = FranceBeamPlanner::new(
        scenario.clone(),
        SimulationEngine::new(SimulationConfig::default()),
        BeamSearchConfig::new(8, 35),
        PlannerWeights::default(),
    );
    let (plan, hard_requirements_met) = match planner.plan() {
        Ok(plan) => (plan, true),
        Err(hoi4simulator::sim::SimulationError::HardRequirementsUnsatisfied) => (
            planner
                .best_effort_plan()
                .map_err(|error| format!("planner failed: {error:?}"))?,
            false,
        ),
        Err(error) => return Err(format!("planner failed: {error:?}")),
    };
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
        plan.final_state
            .supported_divisions(per_division_demand, &scenario.ideas)
    );
    println!("hard requirements met: {}", hard_requirements_met);
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
    "Usage: cargo run --bin france_1936 -- [--profile <NAME>] [--hard-focus <FOCUS_ID[@YYYY-MM-DD]>]\n\nRuns the France 1936 scenario from the Apache Fory dataset in data/structured/<profile>/ and exact mirrored focus data in data/raw/<profile>/."
}

fn parse_hard_focus_goal(value: &str) -> Result<HardFocusGoal, String> {
    let (id, deadline) = match value.split_once('@') {
        Some((id, deadline)) => (id, parse_game_date(deadline)?),
        None => (value, GameDate::new(1940, 5, 10)),
    };
    if id.is_empty() {
        return Err("hard focus goal id must not be empty".to_string());
    }

    Ok(HardFocusGoal {
        id: id.into(),
        deadline,
    })
}

fn parse_game_date(value: &str) -> Result<GameDate, String> {
    let mut parts = value.split('-');
    let Some(year) = parts.next().and_then(|part| part.parse::<u16>().ok()) else {
        return Err(format!("invalid date: {value}"));
    };
    let Some(month) = parts.next().and_then(|part| part.parse::<u8>().ok()) else {
        return Err(format!("invalid date: {value}"));
    };
    let Some(day) = parts.next().and_then(|part| part.parse::<u8>().ok()) else {
        return Err(format!("invalid date: {value}"));
    };

    Ok(GameDate::new(year, month, day))
}
