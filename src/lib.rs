#![forbid(unsafe_code)]

//! HOI4 simulator and solver foundations.
//!
//! The crate starts with a country-extensible domain model, a zero-cost
//! scenario interface, a France 1936 reference scenario, and the first layer
//! of heuristic rule validation for rolling-horizon planning.

pub mod data;
pub mod domain;
pub mod scenario;
pub mod sim;
pub mod solver;
