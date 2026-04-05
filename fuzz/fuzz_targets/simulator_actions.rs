#![no_main]

use hoi4simulator::domain::{
    DoctrineCostReduction, EconomyLaw, EquipmentKind, FocusCondition, FocusEffect, IdeaDefinition,
    IdeaModifiers, MobilizationLaw, NationalFocus, TradeLaw,
};
use hoi4simulator::scenario::France1936Scenario;
use hoi4simulator::sim::{
    Action, AdvisorAction, AdvisorKind, ConstructionAction, ConstructionKind, FocusAction,
    LawAction, LawTarget, ProductionAction, ResearchAction, ResearchBranch, SimulationEngine,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let scenario = fuzz_scenario();
    let runtime = scenario.bootstrap_runtime();
    runtime.assert_invariants();

    let mut cursor = 0_usize;
    let action_count = usize::from(next_byte(data, &mut cursor) % 24);
    let mut actions = Vec::with_capacity(action_count);
    for _ in 0..action_count {
        actions.push(next_action(&scenario, data, &mut cursor));
    }
    actions.sort_by_key(Action::date);

    let end = actions
        .last()
        .map(Action::date)
        .unwrap_or(scenario.start_date);

    let engine = SimulationEngine::default();
    if let Ok(outcome) = engine.simulate(
        &scenario,
        runtime,
        &actions,
        end,
        scenario.pivot_window.start,
    ) {
        outcome.country.assert_invariants();
    }
});

fn next_action(scenario: &France1936Scenario, data: &[u8], cursor: &mut usize) -> Action {
    let kind = next_byte(data, cursor) % 6;
    let date = scenario
        .start_date
        .add_days(u16::from(next_byte(data, cursor) % 90));
    let a = next_byte(data, cursor);
    let b = next_byte(data, cursor);
    let c = next_byte(data, cursor);

    match kind {
        0 => {
            let focus = &scenario.focuses[usize::from(a) % scenario.focuses.len()];
            Action::Focus(FocusAction {
                date,
                focus_id: focus.id.clone(),
            })
        }
        1 => Action::Research(ResearchAction {
            date,
            slot: a % 4,
            branch: match b % 4 {
                0 => ResearchBranch::Industry,
                1 => ResearchBranch::Construction,
                2 => ResearchBranch::Electronics,
                _ => ResearchBranch::Production,
            },
        }),
        2 => Action::Law(LawAction {
            date,
            target: match a % 7 {
                0 => LawTarget::Economy(EconomyLaw::CivilianEconomy),
                1 => LawTarget::Economy(EconomyLaw::EarlyMobilization),
                2 => LawTarget::Economy(EconomyLaw::PartialMobilization),
                3 => LawTarget::Economy(EconomyLaw::WarEconomy),
                4 => LawTarget::Trade(TradeLaw::ExportFocus),
                5 => LawTarget::Trade(TradeLaw::LimitedExports),
                _ => LawTarget::Mobilization(match b % 3 {
                    0 => MobilizationLaw::VolunteerOnly,
                    1 => MobilizationLaw::LimitedConscription,
                    _ => MobilizationLaw::ExtensiveConscription,
                }),
            },
        }),
        3 => Action::Construction(ConstructionAction {
            date,
            state: hoi4simulator::sim::StateId(a % 12),
            kind: match b % 4 {
                0 => ConstructionKind::CivilianFactory,
                1 => ConstructionKind::MilitaryFactory,
                2 => ConstructionKind::Infrastructure,
                _ => ConstructionKind::LandFort,
            },
        }),
        4 => Action::Production(ProductionAction {
            date,
            slot: a % 6,
            equipment: match b % 5 {
                0 => EquipmentKind::InfantryEquipment,
                1 => EquipmentKind::SupportEquipment,
                2 => EquipmentKind::Artillery,
                3 => EquipmentKind::AntiTank,
                _ => EquipmentKind::AntiAir,
            },
            factories: c % 12,
        }),
        _ => Action::Advisor(AdvisorAction {
            date,
            kind: match a % 3 {
                0 => AdvisorKind::IndustryConcern,
                1 => AdvisorKind::ResearchInstitute,
                _ => AdvisorKind::MilitaryIndustrialist,
            },
        }),
    }
}

fn next_byte(data: &[u8], cursor: &mut usize) -> u8 {
    if data.is_empty() {
        return 0;
    }

    let value = data[*cursor % data.len()];
    *cursor = cursor.saturating_add(1);
    value
}

fn fuzz_scenario() -> France1936Scenario {
    France1936Scenario::standard().with_exact_focus_data(
        3,
        vec!["FRA_victors_of_wwi".into()],
        Vec::new(),
        vec![
            NationalFocus {
                id: "FRA_devalue_the_franc".into(),
                days: 1,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                effects: vec![FocusEffect::AddTimedIdea {
                    id: "FRA_devalued_currency".into(),
                    days: 14,
                }],
            },
            NationalFocus {
                id: "FRA_begin_rearmament".into(),
                days: 1,
                prerequisites: vec!["FRA_devalue_the_franc".into()],
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_INDUSTRY".into()],
                effects: vec![
                    FocusEffect::AddResearchSlot(1),
                    FocusEffect::RemoveIdea("FRA_victors_of_wwi".into()),
                ],
            },
            NationalFocus {
                id: "FRA_army_reform".into(),
                days: 1,
                prerequisites: vec!["FRA_begin_rearmament".into()],
                mutually_exclusive: Vec::new(),
                available: FocusCondition::Always,
                bypass: FocusCondition::Not(Box::new(FocusCondition::Always)),
                search_filters: vec!["FOCUS_FILTER_RESEARCH".into()],
                effects: vec![
                    FocusEffect::AddArmyExperience(5),
                    FocusEffect::AddDoctrineCostReduction(DoctrineCostReduction {
                        name: "FRA_army_reform".into(),
                        category: "land_doctrine".into(),
                        cost_reduction_bp: 5_000,
                        uses: 2,
                    }),
                ],
            },
        ],
        vec![
            IdeaDefinition {
                id: "FRA_victors_of_wwi".into(),
                modifiers: IdeaModifiers {
                    research_speed_bp: -500,
                    ..IdeaModifiers::default()
                },
            },
            IdeaDefinition {
                id: "FRA_devalued_currency".into(),
                modifiers: IdeaModifiers {
                    consumer_goods_bp: -1_000,
                    ..IdeaModifiers::default()
                },
            },
        ],
        Vec::new(),
    )
}
