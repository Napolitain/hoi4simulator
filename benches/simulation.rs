use criterion::{Criterion, criterion_group, criterion_main};
use hoi4simulator::scenario::france_1936::France1936Scenario;
use hoi4simulator::sim::engine::{SimulationConfig, SimulationEngine};

fn bench_simulate_365_days_no_actions(c: &mut Criterion) {
    let scenario = France1936Scenario::standard();
    let engine = SimulationEngine::new(SimulationConfig::default());
    let runtime = scenario.bootstrap_runtime();
    let end = scenario.start_date.add_days(365);
    let pivot_date = scenario.pivot_window.start;

    c.bench_function("simulate_365d_no_actions", |b| {
        b.iter(|| {
            engine
                .simulate(&scenario, runtime.clone(), &[], end, pivot_date)
                .expect("simulation should succeed")
        });
    });
}

fn bench_simulate_full_duration_no_actions(c: &mut Criterion) {
    let scenario = France1936Scenario::standard();
    let engine = SimulationEngine::new(SimulationConfig::default());
    let runtime = scenario.bootstrap_runtime();
    let end = scenario.milestones[3].date;
    let pivot_date = scenario.pivot_window.start;

    c.bench_function("simulate_full_no_actions", |b| {
        b.iter(|| {
            engine
                .simulate(&scenario, runtime.clone(), &[], end, pivot_date)
                .expect("simulation should succeed")
        });
    });
}

criterion_group!(
    benches,
    bench_simulate_365_days_no_actions,
    bench_simulate_full_duration_no_actions
);
criterion_main!(benches);
