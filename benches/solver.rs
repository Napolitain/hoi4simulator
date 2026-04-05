use criterion::{Criterion, criterion_group, criterion_main};
use hoi4simulator::scenario::france_1936::France1936Scenario;
use hoi4simulator::sim::engine::{SimulationConfig, SimulationEngine};
use hoi4simulator::solver::france::FranceBeamPlanner;
use hoi4simulator::solver::{BeamSearchConfig, PlannerWeights};

fn bench_france_beam_plan(c: &mut Criterion) {
    let scenario = France1936Scenario::standard();
    let engine = SimulationEngine::new(SimulationConfig::default());
    let config = BeamSearchConfig::new(8, 35);
    let weights = PlannerWeights::default();

    c.bench_function("france_beam_plan", |b| {
        b.iter(|| {
            let planner = FranceBeamPlanner::new(scenario.clone(), engine, config, weights);
            planner.plan().expect("plan should succeed")
        });
    });
}

fn bench_france_best_effort_plan(c: &mut Criterion) {
    let scenario = France1936Scenario::standard();
    let engine = SimulationEngine::new(SimulationConfig::default());
    let config = BeamSearchConfig::new(8, 35);
    let weights = PlannerWeights::default();

    c.bench_function("france_best_effort_plan", |b| {
        b.iter(|| {
            let planner = FranceBeamPlanner::new(scenario.clone(), engine, config, weights);
            planner.best_effort_plan().expect("plan should succeed")
        });
    });
}

criterion_group!(
    benches,
    bench_france_beam_plan,
    bench_france_best_effort_plan
);
criterion_main!(benches);
