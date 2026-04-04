pub mod clausewitz;
pub mod france_1936;

pub use france_1936::{
    DataError, DataProfilePaths, MirroredFile, StructuredDataManifest, StructuredFrance1936Dataset,
    StructuredProductionLine, StructuredState, ingest_profile, load_france_1936_dataset,
    load_france_1936_scenario,
};
