#![no_main]
use libfuzzer_sys::fuzz_target;
use hoi4simulator::data::france_1936::parse_dot_game_date;

fuzz_target!(|data: &str| {
    // Must never panic regardless of input.
    let _ = parse_dot_game_date(data);
});
