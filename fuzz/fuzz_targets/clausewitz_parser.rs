#![no_main]

use hoi4simulator::data::clausewitz::{
    ClausewitzBlock, ClausewitzItem, ClausewitzValue, parse_clausewitz,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(block) = parse_clausewitz(&text) {
        walk_block(&block);
    }
});

fn walk_block(block: &ClausewitzBlock) {
    for item in &block.items {
        match item {
            ClausewitzItem::Assignment(assignment) => walk_value(&assignment.value),
            ClausewitzItem::Value(value) => walk_value(value),
        }
    }
}

fn walk_value(value: &ClausewitzValue) {
    let _ = value.as_str();
    let _ = value.as_i64();
    let _ = value.as_u64();
    let _ = value.as_f64();
    if let Some(block) = value.as_block() {
        walk_block(block);
    }
}
