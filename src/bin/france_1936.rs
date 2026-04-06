use std::env;
use std::fs;
use std::process::ExitCode;

use hoi4simulator::data::{DataProfilePaths, load_france_1936_dataset, load_france_1936_scenario};
use hoi4simulator::domain::{FocusBuildingKind, GameDate, HardFocusGoal, ResourceLedger};
use hoi4simulator::scenario::France1936Scenario;
use hoi4simulator::sim::{
    Action, AdvisorKind, ConstructionKind, CountryRuntime, LawTarget, SimulationConfig,
    SimulationEngine,
};
use hoi4simulator::solver::{BeamSearchConfig, FranceBeamPlanner, PlannedSolution, PlannerWeights};

#[derive(Debug)]
struct RunOptions {
    profile: String,
    hard_focus_goals: Vec<HardFocusGoal>,
    full_actions: bool,
    export_actions: Option<String>,
    include_state: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActionStateSnapshot {
    economy: hoi4simulator::domain::EconomyLaw,
    trade: hoi4simulator::domain::TradeLaw,
    mobilization: hoi4simulator::domain::MobilizationLaw,
    political_power_centi: u32,
    stability_bp: u16,
    war_support_bp: u16,
    total_civilian_factories: u16,
    consumer_goods_ratio_bp: u16,
    consumer_goods_factories: u16,
    available_civilian_factories: u16,
    total_military_factories: u16,
    civilian_factory_construction_speed_bp: i32,
    military_factory_construction_speed_bp: i32,
    research_speed_bp: u16,
    factory_output_bp: u16,
    available_manpower: u64,
    supported_divisions: u16,
    queued_projects: usize,
    production_lines: usize,
    convoys: u16,
    resource_fulfillment_bp: u16,
    domestic_resources: ResourceLedger,
    daily_resource_demand: ResourceLedger,
}

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
    let options = match parse_args(env::args().skip(1)) {
        Ok(options) => options,
        Err(message) if message == "__help__" => {
            print_usage();
            return Ok(());
        }
        Err(message) => return Err(message),
    };

    let paths = DataProfilePaths::new(env!("CARGO_MANIFEST_DIR"), options.profile.clone());
    let dataset = load_france_1936_dataset(&paths).map_err(|error| {
        format!(
            "{error}\n\nRun `cargo run --bin ingest_data -- --game-dir <PATH> --profile {}` first.",
            options.profile
        )
    })?;
    let warnings = dataset.warnings.clone();
    let scenario = load_france_1936_scenario(&paths)
        .map_err(|error| error.to_string())?
        .with_hard_focus_goals(options.hard_focus_goals);
    let simulator = SimulationEngine::new(SimulationConfig::default());
    let planner = FranceBeamPlanner::new(
        scenario.clone(),
        simulator,
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

    println!("profile: {}", options.profile);
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

    let action_snapshots = if options.include_state {
        Some(build_action_state_snapshots(
            &scenario,
            &plan.actions,
            plan.pivot_date,
            simulator,
        )?)
    } else {
        None
    };
    let action_table = render_action_table(&scenario, &plan, action_snapshots.as_deref());
    if let Some(path) = options.export_actions {
        fs::write(&path, &action_table)
            .map_err(|error| format!("failed to write action export to {path}: {error}"))?;
        println!("exported actions: {path}");
    }

    if options.full_actions {
        println!("all actions:");
        let mut lines = action_table.lines();
        if options.include_state {
            if let Some(header) = lines.next() {
                println!("  {header}");
            }
        } else {
            let _ = lines.next();
        }
        for line in lines {
            println!("  {line}");
        }
    } else {
        println!("first actions:");
        let mut lines = action_table.lines();
        if options.include_state {
            if let Some(header) = lines.next() {
                println!("  {header}");
            }
        } else {
            let _ = lines.next();
        }

        for line in lines.take(12) {
            if options.include_state {
                println!("  {line}");
            } else {
                println!("  - {line}");
            }
        }

        if plan.actions.len() > 12 {
            println!("  - ... {} more actions", plan.actions.len() - 12);
        }
    }

    Ok(())
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage: cargo run --bin france_1936 -- [--profile <NAME>] [--hard-focus <FOCUS_ID[@YYYY-MM-DD]>] [--full-actions] [--export-actions <PATH>] [--include-state]\n\nRuns the France 1936 scenario from the Apache Fory dataset in data/structured/<profile>/ and exact mirrored focus data in data/raw/<profile/>, with optional full action output, TSV export, and end-of-day state snapshots for each action date."
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<RunOptions, String> {
    let mut args = args.into_iter();
    let mut profile = "vanilla".to_string();
    let mut hard_focus_goals = Vec::new();
    let mut full_actions = false;
    let mut export_actions = None;
    let mut include_state = false;

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
            "--full-actions" => full_actions = true,
            "--export-actions" => {
                export_actions = Some(
                    args.next()
                        .ok_or_else(|| "missing value for --export-actions".to_string())?,
                );
            }
            "--include-state" => include_state = true,
            "--help" | "-h" => return Err("__help__".to_string()),
            other => {
                return Err(format!("unknown argument: {other}\n\n{}", usage_text()));
            }
        }
    }

    Ok(RunOptions {
        profile,
        hard_focus_goals,
        full_actions,
        export_actions,
        include_state,
    })
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

fn render_action_table(
    scenario: &France1936Scenario,
    plan: &PlannedSolution,
    snapshots: Option<&[ActionStateSnapshot]>,
) -> String {
    let mut lines = Vec::with_capacity(plan.actions.len() + 1);
    lines.push(action_table_header(snapshots.is_some()));

    for (index, action) in plan.actions.iter().enumerate() {
        lines.push(render_action_row(
            index + 1,
            scenario,
            action,
            snapshots.and_then(|states| states.get(index)),
        ));
    }

    lines.join("\n") + "\n"
}

fn action_table_header(include_state: bool) -> String {
    if include_state {
        "step\tdate\tkind\tdetails\teconomy\ttrade\tmobilization\tpp\tstability\twar_support\ttotal_civs\tconsumer_goods_ratio\tconsumer_goods\tusable_civs\ttotal_mils\tciv_build_speed\tmil_build_speed\tresearch_speed\tfactory_output\tmanpower\tsupported_divisions\tqueued_projects\tproduction_lines\tconvoys\tresource_fulfillment\tresources\tdemand".to_string()
    } else {
        "step\tdate\tkind\tdetails".to_string()
    }
}

fn render_action_row(
    step: usize,
    scenario: &France1936Scenario,
    action: &Action,
    snapshot: Option<&ActionStateSnapshot>,
) -> String {
    let mut row = format!(
        "{step}\t{}\t{}\t{}",
        action.date(),
        action_kind_name(action),
        action_details(scenario, action)
    );
    if let Some(snapshot) = snapshot {
        row.push_str(&format!(
            "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            economy_law_name(snapshot.economy),
            trade_law_name(snapshot.trade),
            mobilization_law_name(snapshot.mobilization),
            format_centi(snapshot.political_power_centi),
            format_basis_points(snapshot.stability_bp),
            format_basis_points(snapshot.war_support_bp),
            snapshot.total_civilian_factories,
            format_basis_points(snapshot.consumer_goods_ratio_bp),
            snapshot.consumer_goods_factories,
            snapshot.available_civilian_factories,
            snapshot.total_military_factories,
            format_signed_basis_points(snapshot.civilian_factory_construction_speed_bp),
            format_signed_basis_points(snapshot.military_factory_construction_speed_bp),
            format_basis_points(snapshot.research_speed_bp),
            format_basis_points(snapshot.factory_output_bp),
            snapshot.available_manpower,
            snapshot.supported_divisions,
            snapshot.queued_projects,
            snapshot.production_lines,
            snapshot.convoys,
            format_basis_points(snapshot.resource_fulfillment_bp),
            format_resource_ledger(snapshot.domestic_resources),
            format_resource_ledger(snapshot.daily_resource_demand),
        ));
    }
    row
}

fn build_action_state_snapshots(
    scenario: &France1936Scenario,
    actions: &[Action],
    pivot_date: GameDate,
    simulator: SimulationEngine,
) -> Result<Vec<ActionStateSnapshot>, String> {
    let mut snapshots = Vec::with_capacity(actions.len());
    let mut applied = 0_usize;

    while applied < actions.len() {
        let date = actions[applied].date();
        while applied < actions.len() && actions[applied].date() == date {
            applied += 1;
        }

        let outcome = simulator
            .simulate(
                scenario,
                scenario.bootstrap_runtime(),
                &actions[..applied],
                date,
                pivot_date,
            )
            .map_err(|error| format!("failed to replay action state through {date}: {error:?}"))?;
        let snapshot = capture_action_state_snapshot(scenario, &outcome.country);

        while snapshots.len() < applied {
            snapshots.push(snapshot);
        }
    }

    Ok(snapshots)
}

fn capture_action_state_snapshot(
    scenario: &France1936Scenario,
    runtime: &CountryRuntime,
) -> ActionStateSnapshot {
    let domestic_resources = runtime.domestic_resources(&scenario.ideas);
    let daily_resource_demand = runtime.daily_resource_demand(runtime.equipment_profiles);

    ActionStateSnapshot {
        economy: runtime.country.laws.economy,
        trade: runtime.country.laws.trade,
        mobilization: runtime.country.laws.mobilization,
        political_power_centi: runtime.country.political_power_centi,
        stability_bp: runtime.current_stability_bp(&scenario.ideas),
        war_support_bp: runtime.current_war_support_bp(&scenario.ideas),
        total_civilian_factories: runtime.total_civilian_factories(),
        consumer_goods_ratio_bp: effective_consumer_goods_ratio_bp(runtime, scenario),
        consumer_goods_factories: runtime.consumer_goods_factories(&scenario.ideas),
        available_civilian_factories: runtime.available_civilian_factories(&scenario.ideas),
        total_military_factories: runtime.total_military_factories(),
        civilian_factory_construction_speed_bp: runtime
            .construction_speed_bp_for(FocusBuildingKind::CivilianFactory, &scenario.ideas),
        military_factory_construction_speed_bp: runtime
            .construction_speed_bp_for(FocusBuildingKind::MilitaryFactory, &scenario.ideas),
        research_speed_bp: runtime.research_speed_bp(&scenario.ideas),
        factory_output_bp: runtime.military_output_bp(&scenario.ideas),
        available_manpower: runtime.available_manpower(&scenario.ideas),
        supported_divisions: runtime.supported_divisions(
            scenario.force_plan.template.per_division_demand(),
            &scenario.ideas,
        ),
        queued_projects: runtime.construction_queue.len(),
        production_lines: runtime.production_lines.len(),
        convoys: runtime.convoys,
        resource_fulfillment_bp: daily_resource_demand.fulfillment_bp(domestic_resources),
        domestic_resources,
        daily_resource_demand,
    }
}

fn effective_consumer_goods_ratio_bp(
    runtime: &CountryRuntime,
    scenario: &France1936Scenario,
) -> u16 {
    let mut ratio_bp = i32::from(runtime.country.laws.economy.consumer_goods_ratio_bp());
    ratio_bp += runtime.idea_modifiers(&scenario.ideas).consumer_goods_bp;
    u16::try_from(ratio_bp.clamp(0, 10_000)).unwrap_or(10_000)
}

fn format_centi(value: u32) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}

fn format_basis_points(value: u16) -> String {
    format!("{}.{:02}%", value / 100, value % 100)
}

fn format_signed_basis_points(value: i32) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let magnitude = value.unsigned_abs();
    format!("{sign}{}.{:02}%", magnitude / 100, magnitude % 100)
}

fn format_resource_ledger(resources: ResourceLedger) -> String {
    format!(
        "steel={};aluminium={};tungsten={};chromium={};oil={};rubber={}",
        resources.steel,
        resources.aluminium,
        resources.tungsten,
        resources.chromium,
        resources.oil,
        resources.rubber,
    )
}

fn action_kind_name(action: &Action) -> &'static str {
    match action {
        Action::Construction(_) => "construction",
        Action::Production(_) => "production",
        Action::Focus(_) => "focus",
        Action::Law(_) => "law",
        Action::Advisor(_) => "advisor",
        Action::Research(_) => "research",
    }
}

fn action_details(scenario: &France1936Scenario, action: &Action) -> String {
    match action {
        Action::Construction(action) => {
            let state = scenario
                .initial_state_defs
                .get(usize::from(action.state.0))
                .map(|state| {
                    format!(
                        "dense_state={};raw_state={};state={}",
                        action.state.0, state.raw_state_id, state.name
                    )
                })
                .unwrap_or_else(|| format!("dense_state={}", action.state.0));
            format!("{state};kind={}", construction_kind_name(action.kind))
        }
        Action::Production(action) => format!(
            "slot={};equipment={};factories={}",
            action.slot,
            equipment_kind_name(action.equipment),
            action.factories
        ),
        Action::Focus(action) => format!("id={}", action.focus_id),
        Action::Law(action) => format!("target={}", law_target_name(action.target)),
        Action::Advisor(action) => format!("kind={}", advisor_kind_name(action.kind)),
        Action::Research(action) => {
            format!(
                "slot={};branch={}",
                action.slot,
                research_branch_name(action.branch)
            )
        }
    }
}

fn construction_kind_name(kind: ConstructionKind) -> &'static str {
    match kind {
        ConstructionKind::CivilianFactory => "civilian_factory",
        ConstructionKind::MilitaryFactory => "military_factory",
        ConstructionKind::Infrastructure => "infrastructure",
        ConstructionKind::LandFort => "land_fort",
    }
}

fn equipment_kind_name(kind: hoi4simulator::domain::EquipmentKind) -> &'static str {
    match kind {
        hoi4simulator::domain::EquipmentKind::InfantryEquipment => "infantry_equipment",
        hoi4simulator::domain::EquipmentKind::SupportEquipment => "support_equipment",
        hoi4simulator::domain::EquipmentKind::Artillery => "artillery",
        hoi4simulator::domain::EquipmentKind::AntiTank => "anti_tank",
        hoi4simulator::domain::EquipmentKind::AntiAir => "anti_air",
        hoi4simulator::domain::EquipmentKind::MotorizedEquipment => "motorized_equipment",
        hoi4simulator::domain::EquipmentKind::Armor => "armor",
        hoi4simulator::domain::EquipmentKind::Fighter => "fighter",
        hoi4simulator::domain::EquipmentKind::Bomber => "bomber",
        hoi4simulator::domain::EquipmentKind::Unmodeled => "unmodeled",
    }
}

fn law_target_name(target: LawTarget) -> String {
    match target {
        LawTarget::Economy(law) => format!("economy:{}", economy_law_name(law)),
        LawTarget::Trade(law) => format!("trade:{}", trade_law_name(law)),
        LawTarget::Mobilization(law) => format!("mobilization:{}", mobilization_law_name(law)),
    }
}

fn economy_law_name(law: hoi4simulator::domain::EconomyLaw) -> &'static str {
    match law {
        hoi4simulator::domain::EconomyLaw::CivilianEconomy => "civilian_economy",
        hoi4simulator::domain::EconomyLaw::EarlyMobilization => "early_mobilization",
        hoi4simulator::domain::EconomyLaw::PartialMobilization => "partial_mobilization",
        hoi4simulator::domain::EconomyLaw::WarEconomy => "war_economy",
        hoi4simulator::domain::EconomyLaw::TotalMobilization => "total_mobilization",
    }
}

fn trade_law_name(law: hoi4simulator::domain::TradeLaw) -> &'static str {
    match law {
        hoi4simulator::domain::TradeLaw::ExportFocus => "export_focus",
        hoi4simulator::domain::TradeLaw::LimitedExports => "limited_exports",
        hoi4simulator::domain::TradeLaw::ClosedEconomy => "closed_economy",
        hoi4simulator::domain::TradeLaw::FreeTrade => "free_trade",
    }
}

fn mobilization_law_name(law: hoi4simulator::domain::MobilizationLaw) -> &'static str {
    match law {
        hoi4simulator::domain::MobilizationLaw::VolunteerOnly => "volunteer_only",
        hoi4simulator::domain::MobilizationLaw::LimitedConscription => "limited_conscription",
        hoi4simulator::domain::MobilizationLaw::ExtensiveConscription => "extensive_conscription",
    }
}

fn advisor_kind_name(kind: AdvisorKind) -> &'static str {
    match kind {
        AdvisorKind::IndustryConcern => "industry_concern",
        AdvisorKind::ResearchInstitute => "research_institute",
        AdvisorKind::MilitaryIndustrialist => "military_industrialist",
    }
}

fn research_branch_name(branch: hoi4simulator::domain::ResearchBranch) -> &'static str {
    match branch {
        hoi4simulator::domain::ResearchBranch::Industry => "industry",
        hoi4simulator::domain::ResearchBranch::Construction => "construction",
        hoi4simulator::domain::ResearchBranch::Electronics => "electronics",
        hoi4simulator::domain::ResearchBranch::Production => "production",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Action, ActionStateSnapshot, ConstructionKind, France1936Scenario, action_table_header,
        build_action_state_snapshots, parse_args, render_action_row,
    };
    use hoi4simulator::domain::{GameDate, ResourceLedger};
    use hoi4simulator::sim::{
        ConstructionAction, FocusAction, SimulationConfig, SimulationEngine, StateId,
    };

    #[test]
    fn parse_args_supports_full_exported_and_debug_actions() {
        let options = parse_args([
            "--profile".to_string(),
            "vanilla".to_string(),
            "--hard-focus".to_string(),
            "FRA_devalue_the_franc@1936-02-15".to_string(),
            "--full-actions".to_string(),
            "--include-state".to_string(),
            "--export-actions".to_string(),
            "plan.tsv".to_string(),
        ])
        .unwrap();

        assert_eq!(options.profile, "vanilla");
        assert_eq!(options.hard_focus_goals.len(), 1);
        assert!(options.full_actions);
        assert!(options.include_state);
        assert_eq!(options.export_actions.as_deref(), Some("plan.tsv"));
    }

    #[test]
    fn render_action_row_includes_raw_state_metadata() {
        let scenario = France1936Scenario::standard();
        let row = render_action_row(
            1,
            &scenario,
            &Action::Construction(ConstructionAction {
                date: GameDate::new(1936, 1, 1),
                state: StateId(0),
                kind: ConstructionKind::MilitaryFactory,
            }),
            None,
        );

        assert!(row.contains("1\t1936-01-01\tconstruction\t"));
        assert!(row.contains("dense_state=0"));
        assert!(row.contains("raw_state="));
        assert!(row.contains("state="));
        assert!(row.contains("kind=military_factory"));
    }

    #[test]
    fn render_action_row_formats_focus_actions() {
        let scenario = France1936Scenario::standard();
        let row = render_action_row(
            3,
            &scenario,
            &Action::Focus(FocusAction {
                date: GameDate::new(1936, 2, 6),
                focus_id: "FRA_devalue_the_franc".into(),
            }),
            None,
        );

        assert_eq!(row, "3\t1936-02-06\tfocus\tid=FRA_devalue_the_franc");
    }

    #[test]
    fn render_action_row_appends_state_snapshot_columns() {
        let scenario = France1936Scenario::standard();
        let row = render_action_row(
            1,
            &scenario,
            &Action::Focus(FocusAction {
                date: GameDate::new(1936, 2, 6),
                focus_id: "FRA_devalue_the_franc".into(),
            }),
            Some(&ActionStateSnapshot {
                economy: hoi4simulator::domain::EconomyLaw::CivilianEconomy,
                trade: hoi4simulator::domain::TradeLaw::ExportFocus,
                mobilization: hoi4simulator::domain::MobilizationLaw::LimitedConscription,
                political_power_centi: 12_345,
                stability_bp: 5_500,
                war_support_bp: 4_000,
                total_civilian_factories: 18,
                consumer_goods_ratio_bp: 3_500,
                consumer_goods_factories: 7,
                available_civilian_factories: 11,
                total_military_factories: 9,
                civilian_factory_construction_speed_bp: -2_000,
                military_factory_construction_speed_bp: 1_000,
                research_speed_bp: 500,
                factory_output_bp: 1_000,
                available_manpower: 1_250_000,
                supported_divisions: 42,
                queued_projects: 3,
                production_lines: 5,
                convoys: 12,
                resource_fulfillment_bp: 7_500,
                domestic_resources: ResourceLedger {
                    steel: 20,
                    aluminium: 8,
                    tungsten: 4,
                    chromium: 0,
                    oil: 1,
                    rubber: 0,
                },
                daily_resource_demand: ResourceLedger {
                    steel: 16,
                    aluminium: 8,
                    tungsten: 3,
                    chromium: 0,
                    oil: 2,
                    rubber: 0,
                },
            }),
        );

        assert_eq!(
            action_table_header(true),
            "step\tdate\tkind\tdetails\teconomy\ttrade\tmobilization\tpp\tstability\twar_support\ttotal_civs\tconsumer_goods_ratio\tconsumer_goods\tusable_civs\ttotal_mils\tciv_build_speed\tmil_build_speed\tresearch_speed\tfactory_output\tmanpower\tsupported_divisions\tqueued_projects\tproduction_lines\tconvoys\tresource_fulfillment\tresources\tdemand"
        );
        assert!(row.contains("\tcivilian_economy\texport_focus\tlimited_conscription\t123.45\t55.00%\t40.00%\t18\t35.00%\t7\t11\t9\t-20.00%\t10.00%\t5.00%\t10.00%\t1250000\t42\t3\t5\t12\t75.00%\t"));
        assert!(row.contains("steel=20;aluminium=8;tungsten=4;chromium=0;oil=1;rubber=0"));
        assert!(row.contains("steel=16;aluminium=8;tungsten=3;chromium=0;oil=2;rubber=0"));
    }

    #[test]
    fn build_action_state_snapshots_reuses_same_day_end_state() {
        let scenario = France1936Scenario::standard();
        let buildable_states: Vec<_> = scenario
            .initial_state_defs
            .iter()
            .zip(scenario.initial_states.iter())
            .filter_map(|(definition, state)| {
                if state.free_slots(definition) > 0 {
                    Some(definition.id)
                } else {
                    None
                }
            })
            .take(2)
            .collect();
        assert_eq!(buildable_states.len(), 2);

        let actions = vec![
            Action::Construction(ConstructionAction {
                date: GameDate::new(1936, 1, 1),
                state: buildable_states[0],
                kind: ConstructionKind::CivilianFactory,
            }),
            Action::Construction(ConstructionAction {
                date: GameDate::new(1936, 1, 1),
                state: buildable_states[1],
                kind: ConstructionKind::MilitaryFactory,
            }),
        ];

        let snapshots = build_action_state_snapshots(
            &scenario,
            &actions,
            scenario.pivot_window.start,
            SimulationEngine::new(SimulationConfig::default()),
        )
        .unwrap();

        assert_eq!(snapshots.len(), actions.len());
        assert_eq!(snapshots[0], snapshots[1]);
        assert_eq!(snapshots[0].queued_projects, 2);
    }
}
