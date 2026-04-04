use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fory::{Fory, ForyDefault, ForyObject, Serializer};
use fory_core::StructSerializer;

use crate::domain::{
    CountryLaws, EconomyLaw, EquipmentKind, EquipmentProfile, MobilizationLaw,
    ModeledEquipmentProfiles, ResourceLedger, TradeLaw,
};
use crate::scenario::{France1936Scenario, Frontier};

use super::clausewitz::{ClausewitzBlock, ClausewitzItem, ClausewitzValue, parse_clausewitz};

const DATA_LAYOUT_VERSION: u32 = 2;
const REQUIRED_RAW_DIRS: &[&str] = &[
    "common/country_tags",
    "common/ideas",
    "common/national_focus",
    "common/state_category",
    "common/technologies",
    "common/units/equipment",
    "history/countries",
    "history/states",
    "history/units",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataProfilePaths {
    pub repo_root: PathBuf,
    pub profile: Box<str>,
}

impl DataProfilePaths {
    pub fn new(repo_root: impl Into<PathBuf>, profile: impl Into<String>) -> Self {
        let profile = profile.into();
        assert!(!profile.is_empty());

        Self {
            repo_root: repo_root.into(),
            profile: profile.into_boxed_str(),
        }
    }

    pub fn raw_root(&self) -> PathBuf {
        self.repo_root.join("data/raw").join(self.profile.as_ref())
    }

    pub fn structured_root(&self) -> PathBuf {
        self.repo_root
            .join("data/structured")
            .join(self.profile.as_ref())
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.structured_root().join("manifest.fory")
    }

    pub fn france_1936_path(&self) -> PathBuf {
        self.structured_root().join("scenarios/france_1936.fory")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ForyObject)]
pub struct MirroredFile {
    pub relative_path: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, ForyObject)]
pub struct StructuredDataManifest {
    pub version: u32,
    pub profile: String,
    pub source_game_dir: String,
    pub generated_at_unix: u64,
    pub mirrored_files: Vec<MirroredFile>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, ForyObject)]
pub struct StructuredFrance1936Dataset {
    pub version: u32,
    pub profile: String,
    pub tag: String,
    pub start_date: String,
    pub laws: CountryLaws,
    pub population: u64,
    pub starting_fielded_divisions: u16,
    pub equipment_profiles: ModeledEquipmentProfiles,
    pub states: Vec<StructuredState>,
    pub production_lines: Vec<StructuredProductionLine>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, ForyObject)]
pub struct StructuredState {
    pub raw_state_id: u32,
    pub name_token: String,
    pub source_name: String,
    pub building_slots: u8,
    pub economic_weight: u16,
    pub infrastructure_target: u8,
    pub frontier: Option<Frontier>,
    pub resources: ResourceLedger,
    pub civilian_factories: u8,
    pub military_factories: u8,
    pub infrastructure: u8,
    pub land_fort_level: u8,
    pub manpower: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, ForyObject)]
pub struct StructuredProductionLine {
    pub raw_equipment_token: String,
    pub equipment: EquipmentKind,
    pub factories: u8,
    pub unit_cost_centi: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EquipmentDefinition {
    token: String,
    kind: EquipmentKind,
    year: u16,
    parent: Option<String>,
    archetype: Option<String>,
    is_archetype: bool,
    unit_cost_centi: Option<u32>,
    resources: Option<ResourceLedger>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResolvedEquipmentDefinition {
    kind: EquipmentKind,
    year: u16,
    is_archetype: bool,
    profile: EquipmentProfile,
}

#[derive(Debug)]
pub enum DataError {
    Io { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    Codec { path: PathBuf, message: String },
    Validation(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O error at {}: {source}", path.display()),
            Self::Parse { path, message } => {
                write!(f, "parse error at {}: {message}", path.display())
            }
            Self::Codec { path, message } => {
                write!(f, "Fory codec error at {}: {message}", path.display())
            }
            Self::Validation(message) => write!(f, "validation error: {message}"),
        }
    }
}

impl std::error::Error for DataError {}

pub fn ingest_profile(
    paths: &DataProfilePaths,
    game_dir: &Path,
) -> Result<StructuredDataManifest, DataError> {
    let raw_root = paths.raw_root();
    let structured_root = paths.structured_root();

    if raw_root.exists() {
        fs::remove_dir_all(&raw_root).map_err(|source| DataError::Io {
            path: raw_root.clone(),
            source,
        })?;
    }
    if structured_root.exists() {
        fs::remove_dir_all(&structured_root).map_err(|source| DataError::Io {
            path: structured_root.clone(),
            source,
        })?;
    }

    fs::create_dir_all(&raw_root).map_err(|source| DataError::Io {
        path: raw_root.clone(),
        source,
    })?;
    fs::create_dir_all(structured_root.join("scenarios")).map_err(|source| DataError::Io {
        path: structured_root.join("scenarios"),
        source,
    })?;

    let mirrored_files = mirror_required_directories(game_dir, &raw_root)?;
    let mut warnings = Vec::new();
    let dataset = build_france_1936_dataset(paths, &mut warnings)?;

    let manifest = StructuredDataManifest {
        version: DATA_LAYOUT_VERSION,
        profile: paths.profile.to_string(),
        source_game_dir: game_dir.display().to_string(),
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        mirrored_files,
        warnings: warnings.clone(),
    };

    write_fory(&paths.manifest_path(), &manifest)?;
    write_fory(&paths.france_1936_path(), &dataset)?;

    Ok(manifest)
}

pub fn load_france_1936_dataset(
    paths: &DataProfilePaths,
) -> Result<StructuredFrance1936Dataset, DataError> {
    let path = paths.france_1936_path();
    let bytes = fs::read(&path).map_err(|source| DataError::Io {
        path: path.clone(),
        source,
    })?;
    let fory = structured_data_fory()?;

    fory.deserialize(&bytes).map_err(|source| DataError::Codec {
        path,
        message: source.to_string(),
    })
}

pub fn load_france_1936_scenario(
    paths: &DataProfilePaths,
) -> Result<France1936Scenario, DataError> {
    France1936Scenario::from_dataset(load_france_1936_dataset(paths)?)
}

fn build_france_1936_dataset(
    paths: &DataProfilePaths,
    warnings: &mut Vec<String>,
) -> Result<StructuredFrance1936Dataset, DataError> {
    let raw_root = paths.raw_root();
    let state_categories = parse_state_categories(&raw_root)?;
    let equipment_definitions = parse_equipment_definitions(&raw_root)?;
    let equipment_catalog = resolve_equipment_catalog(&equipment_definitions)?;
    let country_history = parse_country_history(&raw_root, "FRA")?;
    let oob = load_france_1936_oob(&raw_root, &country_history, warnings)?;
    let laws = extract_country_laws(&country_history, warnings);
    let production_lines =
        extract_production_lines(&country_history, oob.as_ref(), &equipment_catalog, warnings)?;
    let equipment_profiles =
        derive_modeled_equipment_profiles(&equipment_catalog, &production_lines, warnings);
    let starting_fielded_divisions = oob
        .as_ref()
        .map(count_division_instances)
        .unwrap_or_default();
    let mut states = extract_owned_states(&raw_root, "FRA", &state_categories)?;
    states.sort_by_key(|state| state.raw_state_id);

    if states.is_empty() {
        return Err(DataError::Validation(
            "France 1936 dataset contains no FRA-owned states".to_string(),
        ));
    }

    let population = states.iter().map(|state| state.manpower).sum();

    Ok(StructuredFrance1936Dataset {
        version: DATA_LAYOUT_VERSION,
        profile: paths.profile.to_string(),
        tag: "FRA".to_string(),
        start_date: "1936-01-01".to_string(),
        laws,
        population,
        starting_fielded_divisions,
        equipment_profiles,
        states,
        production_lines,
        warnings: warnings.clone(),
    })
}

fn mirror_required_directories(
    game_dir: &Path,
    raw_root: &Path,
) -> Result<Vec<MirroredFile>, DataError> {
    let mut mirrored = Vec::new();

    for relative_dir in REQUIRED_RAW_DIRS {
        let source_dir = game_dir.join(relative_dir);
        if !source_dir.exists() {
            return Err(DataError::Validation(format!(
                "required HOI4 data directory is missing: {}",
                source_dir.display()
            )));
        }

        mirror_tree(
            &source_dir,
            &raw_root.join(relative_dir),
            raw_root,
            &mut mirrored,
        )?;
    }

    mirrored.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mirrored)
}

fn mirror_tree(
    source: &Path,
    destination: &Path,
    raw_root: &Path,
    mirrored: &mut Vec<MirroredFile>,
) -> Result<(), DataError> {
    fs::create_dir_all(destination).map_err(|source_err| DataError::Io {
        path: destination.to_path_buf(),
        source: source_err,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| DataError::Io {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| DataError::Io {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|source_err| DataError::Io {
            path: source_path.clone(),
            source: source_err,
        })?;

        if file_type.is_dir() {
            mirror_tree(&source_path, &destination_path, raw_root, mirrored)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        fs::copy(&source_path, &destination_path).map_err(|source_err| DataError::Io {
            path: source_path.clone(),
            source: source_err,
        })?;

        let relative_path = destination_path
            .strip_prefix(raw_root)
            .map_err(|_| {
                DataError::Validation(format!(
                    "mirrored file escaped raw root: {}",
                    destination_path.display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let size_bytes = fs::metadata(&destination_path)
            .map_err(|source_err| DataError::Io {
                path: destination_path.clone(),
                source: source_err,
            })?
            .len();

        mirrored.push(MirroredFile {
            relative_path,
            size_bytes,
        });
    }

    Ok(())
}

fn parse_state_categories(raw_root: &Path) -> Result<Vec<(String, u8)>, DataError> {
    let mut categories = Vec::new();
    let mut files = collect_txt_files(&raw_root.join("common/state_category"))?;
    files.sort();

    for path in files {
        let root = parse_clausewitz_file(&path)?;
        collect_state_categories(&root, &mut categories);
    }

    if categories.is_empty() {
        return Err(DataError::Validation(
            "no state categories with local_building_slots were found".to_string(),
        ));
    }

    Ok(categories)
}

fn collect_state_categories(block: &ClausewitzBlock, categories: &mut Vec<(String, u8)>) {
    for item in &block.items {
        let ClausewitzItem::Assignment(assignment) = item else {
            continue;
        };
        let Some(category) = assignment.value.as_block() else {
            continue;
        };
        if let Some(slots) = category
            .first_assignment("local_building_slots")
            .and_then(ClausewitzValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
        {
            categories.push((assignment.key.to_string(), slots));
        }
        collect_state_categories(category, categories);
    }
}

fn parse_equipment_definitions(raw_root: &Path) -> Result<Vec<EquipmentDefinition>, DataError> {
    let mut definitions = Vec::new();
    let mut files = collect_txt_files(&raw_root.join("common/units/equipment"))?;
    files.sort();

    for path in files {
        let root = parse_clausewitz_file(&path)?;
        collect_equipment_definitions(&root, &mut definitions);
    }

    if definitions.is_empty() {
        return Err(DataError::Validation(
            "no equipment definitions were found".to_string(),
        ));
    }

    definitions.sort_by(|left, right| left.token.cmp(&right.token));
    definitions.dedup_by(|left, right| left.token == right.token);
    Ok(definitions)
}

fn collect_equipment_definitions(
    block: &ClausewitzBlock,
    definitions: &mut Vec<EquipmentDefinition>,
) {
    for item in &block.items {
        let ClausewitzItem::Assignment(assignment) = item else {
            continue;
        };
        if let Some(definition) = assignment.value.as_block() {
            let looks_like_equipment = definition.first_assignment("year").is_some()
                || definition.first_assignment("is_archetype").is_some()
                || definition.first_assignment("archetype").is_some()
                || definition.first_assignment("parent").is_some()
                || definition.first_assignment("build_cost_ic").is_some()
                || definition.first_assignment("resources").is_some();
            if looks_like_equipment {
                let unit_cost_centi = definition
                    .first_assignment("build_cost_ic")
                    .and_then(ClausewitzValue::as_f64)
                    .map(|cost| (cost * 100.0).round().max(1.0) as u32);
                let resources = definition
                    .first_assignment("resources")
                    .and_then(ClausewitzValue::as_block)
                    .map(parse_resource_ledger);

                definitions.push(EquipmentDefinition {
                    token: assignment.key.to_string(),
                    kind: map_equipment_token(assignment.key.as_ref()),
                    year: definition
                        .first_assignment("year")
                        .and_then(ClausewitzValue::as_u64)
                        .and_then(|value| u16::try_from(value).ok())
                        .unwrap_or(0),
                    parent: definition
                        .first_assignment("parent")
                        .and_then(ClausewitzValue::as_str)
                        .map(ToOwned::to_owned),
                    archetype: definition
                        .first_assignment("archetype")
                        .and_then(ClausewitzValue::as_str)
                        .map(ToOwned::to_owned),
                    is_archetype: definition
                        .first_assignment("is_archetype")
                        .and_then(ClausewitzValue::as_bool)
                        .unwrap_or(false),
                    unit_cost_centi,
                    resources,
                });
            }
            collect_equipment_definitions(definition, definitions);
        }
    }
}

fn resolve_equipment_catalog(
    definitions: &[EquipmentDefinition],
) -> Result<Vec<(String, ResolvedEquipmentDefinition)>, DataError> {
    let mut resolved = Vec::with_capacity(definitions.len());
    let mut cache = Vec::<(String, EquipmentProfile)>::with_capacity(definitions.len());

    for definition in definitions {
        let Some(profile) =
            resolve_equipment_profile(&definition.token, definitions, &mut cache, &mut Vec::new())?
        else {
            continue;
        };

        resolved.push((
            definition.token.clone(),
            ResolvedEquipmentDefinition {
                kind: definition.kind,
                year: definition.year,
                is_archetype: definition.is_archetype,
                profile,
            },
        ));
    }

    resolved.sort_by(|left, right| left.0.cmp(&right.0));
    resolved.dedup_by(|left, right| left.0 == right.0);
    Ok(resolved)
}

fn resolve_equipment_profile(
    token: &str,
    definitions: &[EquipmentDefinition],
    cache: &mut Vec<(String, EquipmentProfile)>,
    stack: &mut Vec<String>,
) -> Result<Option<EquipmentProfile>, DataError> {
    if let Some((_, profile)) = cache.iter().find(|(cached, _)| cached == token) {
        return Ok(Some(*profile));
    }

    let Some(definition) = definitions
        .iter()
        .find(|definition| definition.token == token)
    else {
        return Ok(None);
    };
    if stack.iter().any(|current| current == token) {
        return Err(DataError::Validation(format!(
            "equipment definition cycle detected while resolving {token}"
        )));
    }

    stack.push(token.to_string());
    let inherited = if let Some(parent) = definition.parent.as_deref() {
        resolve_equipment_profile(parent, definitions, cache, stack)?
    } else if let Some(archetype) = definition.archetype.as_deref() {
        resolve_equipment_profile(archetype, definitions, cache, stack)?
    } else {
        None
    };
    stack.pop();

    let unit_cost_centi = definition
        .unit_cost_centi
        .or_else(|| inherited.map(|profile| profile.unit_cost_centi));
    let resources = definition
        .resources
        .or_else(|| inherited.map(|profile| profile.resources))
        .unwrap_or_default();
    let Some(unit_cost_centi) = unit_cost_centi else {
        return Ok(None);
    };
    let profile = EquipmentProfile::new(unit_cost_centi, resources);
    cache.push((token.to_string(), profile));

    Ok(Some(profile))
}

fn parse_country_history(raw_root: &Path, tag: &str) -> Result<ClausewitzBlock, DataError> {
    let mut files = collect_txt_files(&raw_root.join("history/countries"))?;
    files.sort();

    let path = files
        .into_iter()
        .find(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.starts_with(tag))
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            DataError::Validation(format!("could not find a history/countries file for {tag}"))
        })?;

    parse_clausewitz_file(&path)
}

fn extract_country_laws(
    country_history: &ClausewitzBlock,
    warnings: &mut Vec<String>,
) -> CountryLaws {
    let mut tokens = Vec::new();
    collect_string_tokens(country_history, &mut tokens);

    let economy = tokens
        .iter()
        .rev()
        .find_map(|token| match token.as_str() {
            "civilian_economy" => Some(EconomyLaw::CivilianEconomy),
            "early_mobilization" => Some(EconomyLaw::EarlyMobilization),
            "partial_mobilization" => Some(EconomyLaw::PartialMobilization),
            "war_economy" => Some(EconomyLaw::WarEconomy),
            _ => None,
        })
        .unwrap_or_else(|| {
            warnings.push(
                "economy law was not explicit in country history; defaulted to CivilianEconomy"
                    .to_string(),
            );
            EconomyLaw::CivilianEconomy
        });
    let trade = tokens
        .iter()
        .rev()
        .find_map(|token| match token.as_str() {
            "export_focus" => Some(TradeLaw::ExportFocus),
            "limited_exports" => Some(TradeLaw::LimitedExports),
            "closed_economy" => Some(TradeLaw::ClosedEconomy),
            _ => None,
        })
        .unwrap_or_else(|| {
            warnings.push(
                "trade law was not explicit in country history; defaulted to ExportFocus"
                    .to_string(),
            );
            TradeLaw::ExportFocus
        });
    let mobilization = tokens
        .iter()
        .rev()
        .find_map(|token| match token.as_str() {
            "volunteer_only" => Some(MobilizationLaw::VolunteerOnly),
            "limited_conscription" => Some(MobilizationLaw::LimitedConscription),
            "extensive_conscription" => Some(MobilizationLaw::ExtensiveConscription),
            _ => None,
        })
        .unwrap_or_else(|| {
            warnings.push(
                "mobilization law was not explicit in country history; defaulted to LimitedConscription"
                    .to_string(),
            );
            MobilizationLaw::LimitedConscription
        });

    CountryLaws {
        economy,
        trade,
        mobilization,
    }
}

fn extract_production_lines(
    country_history: &ClausewitzBlock,
    oob: Option<&ClausewitzBlock>,
    equipment_catalog: &[(String, ResolvedEquipmentDefinition)],
    warnings: &mut Vec<String>,
) -> Result<Vec<StructuredProductionLine>, DataError> {
    let mut lines = Vec::new();
    collect_production_lines(
        country_history,
        "set_production",
        equipment_catalog,
        warnings,
        &mut lines,
    );
    if lines.is_empty()
        && let Some(oob) = oob
    {
        collect_production_lines(
            oob,
            "add_equipment_production",
            equipment_catalog,
            warnings,
            &mut lines,
        );
    }

    if lines.is_empty() {
        return Err(DataError::Validation(
            "country history and referenced OOB contain no usable production blocks".to_string(),
        ));
    }

    Ok(lines)
}

fn collect_production_lines(
    block_root: &ClausewitzBlock,
    production_key: &str,
    equipment_catalog: &[(String, ResolvedEquipmentDefinition)],
    warnings: &mut Vec<String>,
    lines: &mut Vec<StructuredProductionLine>,
) {
    let mut production_blocks = Vec::new();
    collect_named_blocks(block_root, production_key, &mut production_blocks);

    for block in production_blocks {
        let Some(raw_equipment_token) = extract_production_equipment_token(block) else {
            continue;
        };
        let Some(factories) = extract_production_factory_count(block) else {
            continue;
        };
        if factories == 0 {
            continue;
        }

        let equipment = map_equipment_token(&raw_equipment_token);
        let unit_cost_centi = equipment_catalog
            .iter()
            .find(|(token, _)| token == &raw_equipment_token)
            .map(|(_, definition)| definition.profile.unit_cost_centi)
            .unwrap_or_else(|| {
                warnings.push(format!(
                    "equipment definition for {raw_equipment_token} was missing build_cost_ic; using normalized default"
                ));
                equipment.default_unit_cost_centi()
            });

        lines.push(StructuredProductionLine {
            raw_equipment_token,
            equipment,
            factories,
            unit_cost_centi,
        });
    }
}

fn derive_modeled_equipment_profiles(
    equipment_catalog: &[(String, ResolvedEquipmentDefinition)],
    production_lines: &[StructuredProductionLine],
    warnings: &mut Vec<String>,
) -> ModeledEquipmentProfiles {
    let defaults = ModeledEquipmentProfiles::default_1936();

    let find_for_kind = |kind: EquipmentKind| {
        production_lines
            .iter()
            .find(|line| line.equipment == kind)
            .and_then(|line| {
                equipment_catalog
                    .iter()
                    .find(|(token, _)| token == &line.raw_equipment_token)
                    .map(|(_, definition)| definition.profile)
            })
            .or_else(|| select_fallback_profile(kind, equipment_catalog))
    };

    ModeledEquipmentProfiles {
        infantry_equipment: find_for_kind(EquipmentKind::InfantryEquipment).unwrap_or_else(|| {
            warnings.push(
                "infantry equipment profile was missing from exact data; using normalized default"
                    .to_string(),
            );
            defaults.infantry_equipment
        }),
        support_equipment: find_for_kind(EquipmentKind::SupportEquipment).unwrap_or_else(|| {
            warnings.push(
                "support equipment profile was missing from exact data; using normalized default"
                    .to_string(),
            );
            defaults.support_equipment
        }),
        artillery: find_for_kind(EquipmentKind::Artillery).unwrap_or_else(|| {
            warnings.push(
                "artillery profile was missing from exact data; using normalized default"
                    .to_string(),
            );
            defaults.artillery
        }),
        anti_tank: find_for_kind(EquipmentKind::AntiTank).unwrap_or_else(|| {
            warnings.push(
                "anti-tank profile was missing from exact data; using normalized default"
                    .to_string(),
            );
            defaults.anti_tank
        }),
        anti_air: find_for_kind(EquipmentKind::AntiAir).unwrap_or_else(|| {
            warnings.push(
                "anti-air profile was missing from exact data; using normalized default"
                    .to_string(),
            );
            defaults.anti_air
        }),
    }
}

fn select_fallback_profile(
    kind: EquipmentKind,
    equipment_catalog: &[(String, ResolvedEquipmentDefinition)],
) -> Option<EquipmentProfile> {
    equipment_catalog
        .iter()
        .filter(|(_, definition)| definition.kind == kind)
        .filter(|(_, definition)| !definition.is_archetype)
        .filter(|(_, definition)| definition.year <= 1936)
        .max_by_key(|(_, definition)| definition.year)
        .or_else(|| {
            equipment_catalog
                .iter()
                .filter(|(_, definition)| definition.kind == kind)
                .filter(|(_, definition)| !definition.is_archetype)
                .min_by_key(|(_, definition)| definition.year)
        })
        .map(|(_, definition)| definition.profile)
}

fn load_france_1936_oob(
    raw_root: &Path,
    country_history: &ClausewitzBlock,
    warnings: &mut Vec<String>,
) -> Result<Option<ClausewitzBlock>, DataError> {
    let mut oob_names = Vec::new();
    collect_string_assignments(country_history, "set_oob", &mut oob_names);

    let opening_names: Vec<&str> = oob_names
        .iter()
        .map(String::as_str)
        .filter(|name| name.contains("_1936"))
        .collect();
    let Some(selected) = opening_names.first() else {
        return Ok(None);
    };
    if opening_names.len() > 1 {
        warnings.push(format!(
            "multiple 1936 land OOB references were present ({:?}); using {}",
            opening_names, selected
        ));
    }

    parse_clausewitz_file(
        &raw_root
            .join("history/units")
            .join(format!("{selected}.txt")),
    )
    .map(Some)
}

fn extract_production_equipment_token(block: &ClausewitzBlock) -> Option<String> {
    let equipment = block.first_assignment("equipment")?;
    match equipment {
        ClausewitzValue::String(string) => Some(string.to_string()),
        ClausewitzValue::Block(equipment_block) => equipment_block
            .first_assignment("type")
            .and_then(ClausewitzValue::as_str)
            .map(ToOwned::to_owned),
        ClausewitzValue::Integer(_) | ClausewitzValue::Decimal(_) | ClausewitzValue::Bool(_) => {
            None
        }
    }
}

fn extract_production_factory_count(block: &ClausewitzBlock) -> Option<u8> {
    block
        .first_assignment("amount")
        .or_else(|| block.first_assignment("requested_factories"))
        .and_then(ClausewitzValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
}

fn extract_owned_states(
    raw_root: &Path,
    owner_tag: &str,
    state_categories: &[(String, u8)],
) -> Result<Vec<StructuredState>, DataError> {
    let mut files = collect_txt_files(&raw_root.join("history/states"))?;
    files.sort();

    let mut states = Vec::new();
    for path in files {
        let root = parse_clausewitz_file(&path)?;
        let Some(state) = root
            .first_assignment("state")
            .and_then(ClausewitzValue::as_block)
        else {
            continue;
        };
        let Some(history) = state
            .first_assignment("history")
            .and_then(ClausewitzValue::as_block)
        else {
            continue;
        };
        let Some(owner) = history
            .first_assignment("owner")
            .and_then(ClausewitzValue::as_str)
        else {
            continue;
        };
        if owner != owner_tag {
            continue;
        }

        let raw_state_id = state
            .first_assignment("id")
            .and_then(ClausewitzValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                DataError::Validation(format!(
                    "state file {} is missing a valid id",
                    path.display()
                ))
            })?;
        let name_token = state
            .first_assignment("name")
            .and_then(ClausewitzValue::as_str)
            .unwrap_or("UNKNOWN_STATE")
            .to_string();
        let manpower = state
            .first_assignment("manpower")
            .and_then(ClausewitzValue::as_u64)
            .unwrap_or(0);
        let state_category = state
            .first_assignment("state_category")
            .and_then(ClausewitzValue::as_str)
            .ok_or_else(|| {
                DataError::Validation(format!(
                    "state file {} is missing state_category",
                    path.display()
                ))
            })?;
        let building_slots = state_categories
            .iter()
            .find(|(category, _)| category == state_category)
            .map(|(_, slots)| *slots)
            .ok_or_else(|| {
                DataError::Validation(format!(
                    "state category {state_category} from {} is unknown",
                    path.display()
                ))
            })?;
        let resources = state
            .first_assignment("resources")
            .and_then(ClausewitzValue::as_block)
            .map(parse_resource_ledger)
            .unwrap_or_default();

        let buildings = history
            .first_assignment("buildings")
            .and_then(ClausewitzValue::as_block);
        let civilian_factories = buildings
            .and_then(|block| block.first_assignment("industrial_complex"))
            .and_then(ClausewitzValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let military_factories = buildings
            .and_then(|block| block.first_assignment("arms_factory"))
            .and_then(ClausewitzValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let infrastructure = buildings
            .and_then(|block| block.first_assignment("infrastructure"))
            .and_then(ClausewitzValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let land_fort_level = buildings
            .and_then(|block| block.first_assignment("bunker"))
            .or_else(|| buildings.and_then(|block| block.first_assignment("land_fort")))
            .and_then(ClausewitzValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let source_name = normalize_state_source_name(
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("unknown_state"),
        );
        let frontier = infer_frontier(&source_name);
        let economic_weight = compute_economic_weight(
            building_slots,
            civilian_factories,
            military_factories,
            infrastructure,
            frontier,
        );
        let infrastructure_target =
            compute_infrastructure_target(building_slots, infrastructure, frontier);

        states.push(StructuredState {
            raw_state_id,
            name_token,
            source_name,
            building_slots,
            economic_weight,
            infrastructure_target,
            frontier,
            resources,
            civilian_factories,
            military_factories,
            infrastructure,
            land_fort_level,
            manpower,
        });
    }

    Ok(states)
}

fn parse_resource_ledger(block: &ClausewitzBlock) -> ResourceLedger {
    let mut resources = ResourceLedger::default();

    for item in &block.items {
        let ClausewitzItem::Assignment(assignment) = item else {
            continue;
        };
        let amount = assignment
            .value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .or_else(|| {
                assignment
                    .value
                    .as_f64()
                    .map(|value| value.round().max(0.0) as u32)
            });
        if let Some(amount) = amount {
            resources.add_named(assignment.key.as_ref(), amount);
        }
    }

    resources
}

fn parse_clausewitz_file(path: &Path) -> Result<ClausewitzBlock, DataError> {
    let content = fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    parse_clausewitz(&content).map_err(|message| DataError::Parse {
        path: path.to_path_buf(),
        message,
    })
}

fn collect_txt_files(root: &Path) -> Result<Vec<PathBuf>, DataError> {
    let mut files = Vec::new();
    collect_txt_files_recursive(root, &mut files)?;
    Ok(files)
}

fn collect_txt_files_recursive(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), DataError> {
    for entry in fs::read_dir(root).map_err(|source| DataError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| DataError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| DataError::Io {
            path: path.clone(),
            source,
        })?;

        if file_type.is_dir() {
            collect_txt_files_recursive(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("txt")
        {
            files.push(path);
        }
    }

    Ok(())
}

fn collect_named_blocks<'a>(
    block: &'a ClausewitzBlock,
    key: &str,
    output: &mut Vec<&'a ClausewitzBlock>,
) {
    for item in &block.items {
        let ClausewitzItem::Assignment(assignment) = item else {
            continue;
        };
        if assignment.key.as_ref() == key
            && let Some(value_block) = assignment.value.as_block()
        {
            output.push(value_block);
        }
        if let Some(value_block) = assignment.value.as_block() {
            collect_named_blocks(value_block, key, output);
        }
    }
}

fn collect_string_tokens(block: &ClausewitzBlock, output: &mut Vec<String>) {
    for item in &block.items {
        match item {
            ClausewitzItem::Assignment(assignment) => {
                collect_value_strings(&assignment.value, output)
            }
            ClausewitzItem::Value(value) => collect_value_strings(value, output),
        }
    }
}

fn collect_string_assignments(block: &ClausewitzBlock, key: &str, output: &mut Vec<String>) {
    for item in &block.items {
        let ClausewitzItem::Assignment(assignment) = item else {
            continue;
        };
        if assignment.key.as_ref() == key
            && let Some(string) = assignment.value.as_str()
        {
            output.push(string.to_string());
        }
        if let Some(value_block) = assignment.value.as_block() {
            collect_string_assignments(value_block, key, output);
        }
    }
}

fn collect_value_strings(value: &ClausewitzValue, output: &mut Vec<String>) {
    match value {
        ClausewitzValue::String(string) => output.push(string.to_string()),
        ClausewitzValue::Block(block) => collect_string_tokens(block, output),
        ClausewitzValue::Integer(_) | ClausewitzValue::Decimal(_) | ClausewitzValue::Bool(_) => {}
    }
}

fn count_division_instances(block: &ClausewitzBlock) -> u16 {
    let mut divisions = Vec::new();
    collect_named_blocks(block, "division", &mut divisions);
    u16::try_from(divisions.len()).unwrap_or(u16::MAX)
}

fn map_equipment_token(token: &str) -> EquipmentKind {
    if token.starts_with("infantry_equipment") {
        EquipmentKind::InfantryEquipment
    } else if token.starts_with("support_equipment") {
        EquipmentKind::SupportEquipment
    } else if token.starts_with("artillery_equipment") {
        EquipmentKind::Artillery
    } else if token.starts_with("anti_tank_equipment") {
        EquipmentKind::AntiTank
    } else if token.starts_with("anti_air_equipment") {
        EquipmentKind::AntiAir
    } else {
        EquipmentKind::Unmodeled
    }
}

fn normalize_state_source_name(file_stem: &str) -> String {
    let stem = file_stem
        .split_once('-')
        .map(|(_, tail)| tail)
        .unwrap_or(file_stem);
    let mut normalized = String::with_capacity(stem.len());
    let mut last_was_underscore = false;

    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            normalized.push('_');
            last_was_underscore = true;
        }
    }

    normalized.trim_matches('_').to_string()
}

fn infer_frontier(source_name: &str) -> Option<Frontier> {
    if source_name.contains("lorraine") || source_name.contains("alsace") {
        return Some(Frontier::Germany);
    }
    if source_name.contains("nord") || source_name.contains("picard") {
        return Some(Frontier::Belgium);
    }

    None
}

fn compute_economic_weight(
    building_slots: u8,
    civilian_factories: u8,
    military_factories: u8,
    infrastructure: u8,
    frontier: Option<Frontier>,
) -> u16 {
    u16::from(building_slots)
        + u16::from(civilian_factories) * 2
        + u16::from(military_factories) * 3
        + u16::from(infrastructure)
        + u16::from(frontier.is_some()) * 2
}

fn compute_infrastructure_target(
    building_slots: u8,
    infrastructure: u8,
    frontier: Option<Frontier>,
) -> u8 {
    let extra = u8::from(frontier.is_some() || building_slots >= 8);
    infrastructure.saturating_add(extra).min(10)
}

fn write_fory<T>(path: &Path, value: &T) -> Result<(), DataError>
where
    T: Serializer,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| DataError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let fory = structured_data_fory()?;
    let bytes = fory.serialize(value).map_err(|source| DataError::Codec {
        path: path.to_path_buf(),
        message: source.to_string(),
    })?;
    fs::write(path, bytes).map_err(|source| DataError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn structured_data_fory() -> Result<Fory, DataError> {
    let mut fory = Fory::default();

    register_type::<MirroredFile>(&mut fory, 1_000)?;
    register_type::<StructuredDataManifest>(&mut fory, 1_001)?;
    register_type::<StructuredFrance1936Dataset>(&mut fory, 1_002)?;
    register_type::<StructuredState>(&mut fory, 1_003)?;
    register_type::<StructuredProductionLine>(&mut fory, 1_004)?;
    register_type::<CountryLaws>(&mut fory, 1_005)?;
    register_type::<EconomyLaw>(&mut fory, 1_006)?;
    register_type::<TradeLaw>(&mut fory, 1_007)?;
    register_type::<MobilizationLaw>(&mut fory, 1_008)?;
    register_type::<EquipmentKind>(&mut fory, 1_009)?;
    register_type::<Frontier>(&mut fory, 1_010)?;
    register_type::<ResourceLedger>(&mut fory, 1_011)?;
    register_type::<EquipmentProfile>(&mut fory, 1_012)?;
    register_type::<ModeledEquipmentProfiles>(&mut fory, 1_013)?;

    Ok(fory)
}

fn register_type<T>(fory: &mut Fory, id: u32) -> Result<(), DataError>
where
    T: 'static + StructSerializer + Serializer + ForyDefault,
{
    fory.register::<T>(id).map_err(|source| {
        DataError::Validation(format!("failed to register Fory type {id}: {source}"))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use crate::domain::{CountryLaws, EconomyLaw, EquipmentKind, MobilizationLaw, TradeLaw};

    use super::{DataProfilePaths, ingest_profile, load_france_1936_dataset};

    #[test]
    fn ingest_profile_mirrors_raw_files_and_generates_structured_france_dataset() {
        let repo_root = tempdir().unwrap();
        let game_root = tempdir().unwrap();

        write_fixture(
            game_root.path(),
            "common/country_tags/00_countries.txt",
            r#"FRA = "countries/France.txt""#,
        );
        write_fixture(
            game_root.path(),
            "common/ideas/00_laws.txt",
            "ideas = { economy = { civilian_economy = {} } }",
        );
        write_fixture(
            game_root.path(),
            "common/national_focus/fra.txt",
            "focus_tree = { id = FRA_focus }",
        );
        write_fixture(
            game_root.path(),
            "common/technologies/industry.txt",
            "technologies = {}",
        );
        write_fixture(
            game_root.path(),
            "common/state_category/00_state_category.txt",
            r#"
            metropolis = { local_building_slots = 12 }
            city = { local_building_slots = 8 }
            rural = { local_building_slots = 4 }
            "#,
        );
        write_fixture(
            game_root.path(),
            "common/units/equipment/00_equipment.txt",
            r#"
            infantry_equipment_1 = { build_cost_ic = 0.5 }
            support_equipment_1 = { build_cost_ic = 4 }
            artillery_equipment_1 = { build_cost_ic = 3.5 }
            anti_tank_equipment_1 = { build_cost_ic = 4 }
            anti_air_equipment_1 = { build_cost_ic = 3.5 }
            fighter_equipment_0 = { build_cost_ic = 22 }
            "#,
        );
        write_fixture(
            game_root.path(),
            "history/countries/FRA - France.txt",
            r#"
            add_ideas = { civilian_economy export_focus limited_conscription }
            set_production = { producer = FRA equipment = infantry_equipment_1 amount = 8 }
            set_production = { producer = FRA equipment = support_equipment_1 amount = 2 }
            set_production = { producer = FRA equipment = artillery_equipment_1 amount = 2 }
            set_production = { producer = FRA equipment = anti_tank_equipment_1 amount = 1 }
            set_production = { producer = FRA equipment = anti_air_equipment_1 amount = 1 }
            set_production = { producer = FRA equipment = fighter_equipment_0 amount = 3 }
            "#,
        );
        write_fixture(
            game_root.path(),
            "history/units/FRA_1936.txt",
            r#"
            instant_effect = {
                add_equipment_production = {
                    equipment = { type = infantry_equipment_1 creator = "FRA" }
                    requested_factories = 2
                }
            }
            "#,
        );
        write_fixture(
            game_root.path(),
            "history/states/01-ile_de_france.txt",
            r#"
            state = {
                id = 1
                name = "STATE_1"
                manpower = 8000000
                state_category = metropolis
                history = {
                    owner = FRA
                    buildings = {
                        infrastructure = 8
                        industrial_complex = 8
                        arms_factory = 2
                    }
                }
            }
            "#,
        );
        write_fixture(
            game_root.path(),
            "history/states/02-nord.txt",
            r#"
            state = {
                id = 2
                name = "STATE_2"
                manpower = 4000000
                state_category = city
                history = {
                    owner = FRA
                    buildings = {
                        infrastructure = 7
                        industrial_complex = 4
                        arms_factory = 2
                    }
                }
            }
            "#,
        );
        write_fixture(
            game_root.path(),
            "history/states/03-lorraine.txt",
            r#"
            state = {
                id = 3
                name = "STATE_3"
                manpower = 3000000
                state_category = city
                history = {
                    owner = FRA
                    buildings = {
                        infrastructure = 7
                        industrial_complex = 3
                        arms_factory = 2
                        bunker = 1
                    }
                }
            }
            "#,
        );
        write_fixture(
            game_root.path(),
            "history/states/04-brussels.txt",
            r#"
            state = {
                id = 4
                name = "STATE_4"
                manpower = 1000000
                state_category = city
                history = {
                    owner = BEL
                    buildings = {
                        infrastructure = 6
                        industrial_complex = 2
                    }
                }
            }
            "#,
        );

        let paths = DataProfilePaths::new(repo_root.path(), "fixture");
        let manifest = ingest_profile(&paths, game_root.path()).unwrap();
        let dataset = load_france_1936_dataset(&paths).unwrap();

        assert_eq!(manifest.version, 2);
        assert!(!manifest.mirrored_files.is_empty());
        assert_eq!(dataset.tag, "FRA");
        assert_eq!(
            dataset.laws,
            CountryLaws {
                economy: EconomyLaw::CivilianEconomy,
                trade: TradeLaw::ExportFocus,
                mobilization: MobilizationLaw::LimitedConscription,
            }
        );
        assert_eq!(dataset.population, 15_000_000);
        assert_eq!(dataset.states.len(), 3);
        assert_eq!(dataset.states[0].source_name, "ile_de_france");
        assert_eq!(
            dataset.states[1].frontier,
            Some(crate::scenario::Frontier::Belgium)
        );
        assert_eq!(
            dataset.states[2].frontier,
            Some(crate::scenario::Frontier::Germany)
        );
        assert_eq!(dataset.production_lines.len(), 6);
        assert_eq!(
            dataset.production_lines[0].equipment,
            EquipmentKind::InfantryEquipment
        );
        assert_eq!(
            dataset.production_lines[5].equipment,
            EquipmentKind::Unmodeled
        );

        assert!(paths.manifest_path().exists());
        assert!(paths.france_1936_path().exists());
    }

    fn write_fixture(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}
