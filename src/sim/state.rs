use crate::domain::{
    CountryLaws, DoctrineCostReduction, EquipmentDemand, EquipmentKind, FieldedDivision,
    FocusBuildingKind, GameDate, GovernmentIdeology, IdeaDefinition, IdeaModifiers,
    ModeledEquipmentProfiles, ResourceLedger, TechId, TechnologyBonus, TechnologyModifiers,
    TechnologyNode, TechnologyTree, TimelineEvent, WorldState,
};
use crate::scenario::{Frontier, FrontierFortRequirement};

use super::actions::{ConstructionKind, ResearchBranch, StateId};

pub const POLITICAL_POWER_UNIT: u32 = 100;
pub const BASE_PRODUCTION_EFFICIENCY_PERMILLE: u16 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategicPhase {
    PrePivot,
    PostPivot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountryState {
    pub date: GameDate,
    pub population: u64,
    pub political_power_centi: u32,
    pub political_power_daily_centi: u16,
    pub stability_bp: u16,
    pub war_support_bp: u16,
    pub government: GovernmentIdeology,
    pub elections_allowed: bool,
    pub last_election: Option<GameDate>,
    pub laws: CountryLaws,
}

impl CountryState {
    pub fn new(date: GameDate, population: u64, laws: CountryLaws) -> Self {
        Self::with_support_levels(date, population, laws, 5_000, 5_000)
    }

    pub fn with_support_levels(
        date: GameDate,
        population: u64,
        laws: CountryLaws,
        stability_bp: u16,
        war_support_bp: u16,
    ) -> Self {
        assert!(population > 0);
        assert!(stability_bp <= 10_000);
        assert!(war_support_bp <= 10_000);

        Self {
            date,
            population,
            political_power_centi: 0,
            political_power_daily_centi: 2 * POLITICAL_POWER_UNIT as u16,
            stability_bp,
            war_support_bp,
            government: GovernmentIdeology::Democratic,
            elections_allowed: true,
            last_election: None,
            laws,
        }
    }

    pub fn advance_day(&mut self, daily_bonus_centi: u16, stability_bp_delta: i32) {
        self.date = self.date.next_day();
        self.political_power_centi += u32::from(self.political_power_daily_centi);
        self.political_power_centi += u32::from(daily_bonus_centi);
        let stability_bp = i32::from(self.stability_bp) + stability_bp_delta;
        self.stability_bp = u16::try_from(stability_bp.clamp(0, 10_000)).unwrap_or(10_000);
    }

    pub fn available_manpower(&self) -> u64 {
        self.laws.mobilization.available_manpower(self.population)
    }

    pub fn can_spend_political_power(&self, cost_centi: u32) -> bool {
        self.political_power_centi >= cost_centi
    }

    pub fn spend_political_power(&mut self, cost_centi: u32) -> bool {
        if !self.can_spend_political_power(cost_centi) {
            return false;
        }

        self.political_power_centi -= cost_centi;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDefinition {
    pub id: StateId,
    pub raw_state_id: u32,
    pub name: Box<str>,
    pub building_slots: u8,
    pub economic_weight: u16,
    pub infrastructure_target: u8,
    pub is_core_of_root: bool,
    pub frontier: Option<Frontier>,
    pub resources: ResourceLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRuntime {
    pub civilian_factories: u8,
    pub military_factories: u8,
    pub infrastructure: u8,
    pub land_fort_level: u8,
}

impl StateRuntime {
    pub fn used_slots(self) -> u8 {
        self.civilian_factories + self.military_factories
    }

    pub fn free_slots(self, definition: &StateDefinition) -> u8 {
        definition.building_slots.saturating_sub(self.used_slots())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stockpile {
    pub infantry_equipment: u32,
    pub support_equipment: u32,
    pub artillery: u32,
    pub anti_tank: u32,
    pub anti_air: u32,
    pub motorized_equipment: u32,
    pub armor: u32,
    pub fighters: u32,
    pub bombers: u32,
    pub unmodeled_equipment: u32,
}

impl Stockpile {
    pub fn add(&mut self, equipment: EquipmentKind, amount: u32) {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment += amount,
            EquipmentKind::SupportEquipment => self.support_equipment += amount,
            EquipmentKind::Artillery => self.artillery += amount,
            EquipmentKind::AntiTank => self.anti_tank += amount,
            EquipmentKind::AntiAir => self.anti_air += amount,
            EquipmentKind::MotorizedEquipment => self.motorized_equipment += amount,
            EquipmentKind::Armor => self.armor += amount,
            EquipmentKind::Fighter => self.fighters += amount,
            EquipmentKind::Bomber => self.bombers += amount,
            EquipmentKind::Unmodeled => self.unmodeled_equipment += amount,
        }
    }

    pub fn get(self, equipment: EquipmentKind) -> u32 {
        match equipment {
            EquipmentKind::InfantryEquipment => self.infantry_equipment,
            EquipmentKind::SupportEquipment => self.support_equipment,
            EquipmentKind::Artillery => self.artillery,
            EquipmentKind::AntiTank => self.anti_tank,
            EquipmentKind::AntiAir => self.anti_air,
            EquipmentKind::MotorizedEquipment => self.motorized_equipment,
            EquipmentKind::Armor => self.armor,
            EquipmentKind::Fighter => self.fighters,
            EquipmentKind::Bomber => self.bombers,
            EquipmentKind::Unmodeled => self.unmodeled_equipment,
        }
    }

    pub fn covers(self, demand: EquipmentDemand) -> bool {
        self.infantry_equipment >= demand.infantry_equipment
            && self.support_equipment >= demand.support_equipment
            && self.artillery >= demand.artillery
            && self.anti_tank >= demand.anti_tank
            && self.anti_air >= demand.anti_air
            && self.motorized_equipment >= demand.motorized_equipment
            && self.armor >= demand.armor
            && self.fighters >= demand.fighters
            && self.bombers >= demand.bombers
    }

    pub fn saturating_sub_demand(self, demand: EquipmentDemand) -> Self {
        Self {
            infantry_equipment: self
                .infantry_equipment
                .saturating_sub(demand.infantry_equipment),
            support_equipment: self
                .support_equipment
                .saturating_sub(demand.support_equipment),
            artillery: self.artillery.saturating_sub(demand.artillery),
            anti_tank: self.anti_tank.saturating_sub(demand.anti_tank),
            anti_air: self.anti_air.saturating_sub(demand.anti_air),
            motorized_equipment: self
                .motorized_equipment
                .saturating_sub(demand.motorized_equipment),
            armor: self.armor.saturating_sub(demand.armor),
            fighters: self.fighters.saturating_sub(demand.fighters),
            bombers: self.bombers.saturating_sub(demand.bombers),
            unmodeled_equipment: self.unmodeled_equipment,
        }
    }

    pub fn ready_divisions(self, demand: EquipmentDemand, manpower_available: u64) -> u16 {
        assert!(demand.infantry_equipment > 0);
        assert!(demand.manpower > 0);

        let manpower_limit = manpower_available / u64::from(demand.manpower);
        let limit_for = |stockpile: u32, required: u32| {
            if required == 0 {
                u32::MAX
            } else {
                stockpile / required
            }
        };
        let equipment_limits = [
            limit_for(self.infantry_equipment, demand.infantry_equipment),
            limit_for(self.support_equipment, demand.support_equipment),
            limit_for(self.artillery, demand.artillery),
            limit_for(self.anti_tank, demand.anti_tank),
            limit_for(self.anti_air, demand.anti_air),
            limit_for(self.motorized_equipment, demand.motorized_equipment),
            limit_for(self.armor, demand.armor),
            limit_for(self.fighters, demand.fighters),
            limit_for(self.bombers, demand.bombers),
        ];

        let equipment_limit = equipment_limits.into_iter().min().unwrap_or(0);
        let divisions = equipment_limit.min(u32::try_from(manpower_limit).unwrap_or(u32::MAX));

        u16::try_from(divisions).unwrap_or(u16::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructionProject {
    pub state: StateId,
    pub kind: ConstructionKind,
    pub total_cost_centi: u32,
    pub progress_centi: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductionLine {
    pub equipment: EquipmentKind,
    pub factories: u8,
    pub unit_cost_centi: u32,
    pub efficiency_permille: u16,
    pub accumulated_ic_centi: u32,
}

impl ProductionLine {
    pub fn new(equipment: EquipmentKind, factories: u8) -> Self {
        Self::new_with_cost(equipment, factories, equipment.default_unit_cost_centi())
    }

    pub fn new_with_cost(equipment: EquipmentKind, factories: u8, unit_cost_centi: u32) -> Self {
        Self {
            equipment,
            factories,
            unit_cost_centi,
            efficiency_permille: BASE_PRODUCTION_EFFICIENCY_PERMILLE,
            accumulated_ic_centi: 0,
        }
    }

    pub fn reassign(
        &mut self,
        equipment: EquipmentKind,
        factories: u8,
        unit_cost_centi: u32,
        efficiency_floor_permille: u16,
    ) {
        if self.equipment != equipment {
            self.efficiency_permille = efficiency_floor_permille;
            self.accumulated_ic_centi = 0;
        }

        self.equipment = equipment;
        self.factories = factories;
        self.unit_cost_centi = unit_cost_centi;
    }

    pub fn daily_resource_demand(
        self,
        equipment_profiles: ModeledEquipmentProfiles,
    ) -> ResourceLedger {
        equipment_profiles
            .profile(self.equipment)
            .resources
            .scale(u16::from(self.factories))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResearchSummary {
    pub industry: u16,
    pub construction: u16,
    pub electronics: u16,
    pub production: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct AdvisorRoster {
    pub industry: bool,
    pub research: bool,
    pub military_industry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusProgress {
    pub focus_id: Box<str>,
    pub days_progress: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedFocus {
    pub id: Box<str>,
    pub completed_on: GameDate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveIdea {
    pub id: Box<str>,
    pub remaining_days: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveCountryFlag {
    pub id: Box<str>,
    pub expires_on: Option<GameDate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootWarGoal {
    pub target: Box<str>,
    pub kind: Box<str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResearchSlotState {
    pub branch: Option<ResearchBranch>,
    pub technology: Option<TechId>,
    pub progress_centi: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountryRuntime {
    pub country: CountryState,
    pub tag: Box<str>,
    pub original_tag: Box<str>,
    pub subject_of: Option<Box<str>>,
    pub enabled_dlcs: Box<[Box<str>]>,
    pub state_defs: Box<[StateDefinition]>,
    pub states: Box<[StateRuntime]>,
    pub stockpile: Stockpile,
    pub army_experience: u16,
    pub stability_weekly_accumulator_bp: i32,
    pub fielded_divisions: u16,
    pub fielded_demand: EquipmentDemand,
    pub fielded_force: Box<[FieldedDivision]>,
    pub equipment_profiles: ModeledEquipmentProfiles,
    pub technology_modifiers: TechnologyModifiers,
    pub completed_technologies: Box<[bool]>,
    pub production_lines: Box<[ProductionLine]>,
    pub construction_queue: Vec<ConstructionProject>,
    pub focus: Option<FocusProgress>,
    pub completed_focuses: Vec<CompletedFocus>,
    pub active_ideas: Vec<ActiveIdea>,
    pub doctrine_cost_reductions: Vec<DoctrineCostReduction>,
    pub technology_bonuses: Vec<TechnologyBonus>,
    pub country_flags: Vec<ActiveCountryFlag>,
    pub country_leader_traits: Vec<Box<str>>,
    pub country_rules: Vec<Box<str>>,
    pub war_goals: Vec<RootWarGoal>,
    pub world_state: WorldState,
    pub state_flags: Box<[Vec<Box<str>>]>,
    pub transferred_states: Vec<u32>,
    pub research_slots: Vec<ResearchSlotState>,
    pub completed_research: ResearchSummary,
    pub advisors: AdvisorRoster,
    pub convoys: u16,
    pub selected_naval_oob: Option<Box<str>>,
}

impl CountryRuntime {
    pub fn new(
        country: CountryState,
        state_defs: Box<[StateDefinition]>,
        states: Box<[StateRuntime]>,
        production_lines: Box<[ProductionLine]>,
    ) -> Self {
        assert_eq!(state_defs.len(), states.len());
        let state_count = states.len();

        let runtime = Self {
            country,
            tag: "FRA".into(),
            original_tag: "FRA".into(),
            subject_of: None,
            enabled_dlcs: Vec::new().into_boxed_slice(),
            state_defs,
            states,
            stockpile: Stockpile::default(),
            army_experience: 0,
            stability_weekly_accumulator_bp: 0,
            fielded_divisions: 0,
            fielded_demand: EquipmentDemand::default(),
            fielded_force: Vec::new().into_boxed_slice(),
            equipment_profiles: ModeledEquipmentProfiles::default_1936(),
            technology_modifiers: TechnologyModifiers::default(),
            completed_technologies: Vec::new().into_boxed_slice(),
            production_lines,
            construction_queue: Vec::with_capacity(64),
            focus: None,
            completed_focuses: Vec::with_capacity(64),
            active_ideas: Vec::with_capacity(32),
            doctrine_cost_reductions: Vec::with_capacity(8),
            technology_bonuses: Vec::with_capacity(16),
            country_flags: Vec::with_capacity(32),
            country_leader_traits: Vec::with_capacity(8),
            country_rules: Vec::with_capacity(8),
            war_goals: Vec::with_capacity(16),
            world_state: WorldState::default(),
            state_flags: vec![Vec::new(); state_count].into_boxed_slice(),
            transferred_states: Vec::with_capacity(32),
            research_slots: vec![ResearchSlotState::default(), ResearchSlotState::default()],
            completed_research: ResearchSummary::default(),
            advisors: AdvisorRoster::default(),
            convoys: 0,
            selected_naval_oob: None,
        };
        runtime.assert_invariants();
        runtime
    }

    pub fn with_research_slots(mut self, count: u8) -> Self {
        assert!(count > 0);
        self.research_slots = vec![ResearchSlotState::default(); usize::from(count)];
        self.assert_invariants();
        self
    }

    pub fn with_enabled_dlcs(mut self, enabled_dlcs: Box<[Box<str>]>) -> Self {
        self.enabled_dlcs = enabled_dlcs;
        self.assert_invariants();
        self
    }

    pub fn with_identity(mut self, tag: impl Into<Box<str>>) -> Self {
        let tag = tag.into();
        self.original_tag = tag.clone();
        self.tag = tag;
        self.assert_invariants();
        self
    }

    pub fn with_naval_setup(mut self, selected_naval_oob: Option<Box<str>>, convoys: u16) -> Self {
        self.selected_naval_oob = selected_naval_oob;
        self.convoys = convoys;
        self.assert_invariants();
        self
    }

    pub fn with_equipment_profiles(mut self, equipment_profiles: ModeledEquipmentProfiles) -> Self {
        self.equipment_profiles = equipment_profiles;
        self.assert_invariants();
        self
    }

    pub fn has_dlc(&self, dlc: &str) -> bool {
        self.enabled_dlcs
            .iter()
            .any(|current| current.as_ref() == dlc)
    }

    pub fn is_subject(&self) -> bool {
        self.subject_of.is_some()
    }

    pub fn in_faction(&self) -> bool {
        self.world_state.country_in_faction(self.tag.as_ref())
    }

    pub fn set_country_rule(&mut self, rule: impl Into<Box<str>>, enabled: bool) {
        let rule = rule.into();
        self.country_rules.retain(|current| current != &rule);
        if enabled {
            self.country_rules.push(rule);
        }
        self.assert_invariants();
    }

    pub fn has_country_rule(&self, rule: &str) -> bool {
        self.country_rules
            .iter()
            .any(|current| current.as_ref() == rule)
    }

    pub fn create_faction(&mut self, faction: impl Into<Box<str>>) {
        self.world_state
            .set_country_faction(self.tag.clone(), faction.into());
        self.assert_invariants();
    }

    pub fn join_faction(&mut self, target: &str) {
        let faction = self
            .world_state
            .country_faction(target)
            .map(|faction| faction.to_string())
            .unwrap_or_else(|| target.to_string());
        self.world_state
            .set_country_faction(self.tag.clone(), faction);
        self.assert_invariants();
    }

    pub fn add_war_goal(&mut self, target: impl Into<Box<str>>, kind: impl Into<Box<str>>) {
        let war_goal = RootWarGoal {
            target: target.into(),
            kind: kind.into(),
        };
        if self.war_goals.iter().any(|current| current == &war_goal) {
            return;
        }
        self.war_goals.push(war_goal);
        self.assert_invariants();
    }

    pub fn transfer_state_to_root(&mut self, raw_state_id: u32) {
        if self
            .transferred_states
            .iter()
            .any(|state| state == &raw_state_id)
        {
            return;
        }
        self.transferred_states.push(raw_state_id);
        self.assert_invariants();
    }

    pub fn add_technology_bonus(&mut self, bonus: TechnologyBonus) {
        if bonus.uses == 0 {
            return;
        }
        self.technology_bonuses.push(bonus);
        self.assert_invariants();
    }

    pub fn technology_bonus_bp(&self, technology: &TechnologyNode) -> u32 {
        self.technology_bonuses
            .iter()
            .filter(|bonus| bonus.matches(technology))
            .map(|bonus| u32::from(bonus.bonus_bp))
            .sum()
    }

    pub fn consume_technology_bonuses(&mut self, technology: &TechnologyNode) {
        for bonus in &mut self.technology_bonuses {
            if bonus.uses > 0 && bonus.matches(technology) {
                bonus.uses = bonus.uses.saturating_sub(1);
            }
        }
        self.technology_bonuses.retain(|bonus| bonus.uses > 0);
        self.assert_invariants();
    }

    pub fn with_fielded_force(
        mut self,
        divisions: u16,
        per_division_demand: EquipmentDemand,
    ) -> Self {
        self.fielded_divisions = divisions;
        self.fielded_demand = per_division_demand.scale(divisions);
        self.fielded_force = Vec::new().into_boxed_slice();
        self.assert_invariants();
        self
    }

    pub fn with_exact_fielded_force(mut self, fielded_force: Box<[FieldedDivision]>) -> Self {
        let fielded_divisions = fielded_force
            .iter()
            .filter(|division| division.target_demand.has_equipment())
            .count();
        self.fielded_divisions = u16::try_from(fielded_divisions).unwrap_or(u16::MAX);
        self.fielded_demand = fielded_force
            .iter()
            .fold(EquipmentDemand::default(), |total, division| {
                total.plus(division.target_demand)
            });
        self.fielded_force = fielded_force;
        self.assert_invariants();
        self
    }

    pub fn assert_invariants(&self) {
        assert_eq!(self.state_defs.len(), self.states.len());
        assert_eq!(self.state_defs.len(), self.state_flags.len());
        assert!(!self.research_slots.is_empty());
        assert!(!self.tag.is_empty());
        assert!(!self.original_tag.is_empty());
        assert!(self.country.stability_bp <= 10_000);
        assert!(self.country.war_support_bp <= 10_000);
        assert!(self.stability_weekly_accumulator_bp.abs() < 7);

        assert_unique_strs(
            self.completed_focuses.iter().map(|focus| focus.id.as_ref()),
            "completed focus",
        );
        assert_unique_strs(
            self.active_ideas.iter().map(|idea| idea.id.as_ref()),
            "active idea",
        );
        assert_unique_copy(
            self.research_slots.iter().filter_map(|slot| slot.branch),
            "active research branch",
        );
        assert_unique_copy(
            self.research_slots
                .iter()
                .filter_map(|slot| slot.technology),
            "active research technology",
        );
        assert_unique_strs(
            self.country_flags.iter().map(|flag| flag.id.as_ref()),
            "country flag",
        );
        assert_unique_strs(
            self.country_leader_traits.iter().map(Box::as_ref),
            "country leader trait",
        );
        assert_unique_pairs(
            self.doctrine_cost_reductions
                .iter()
                .map(|reduction| (reduction.name.as_ref(), reduction.category.as_ref())),
            "doctrine cost reduction",
        );
        assert_unique_strs(self.country_rules.iter().map(Box::as_ref), "country rule");
        assert_unique_pairs(
            self.war_goals
                .iter()
                .map(|war_goal| (war_goal.target.as_ref(), war_goal.kind.as_ref())),
            "war goal",
        );
        assert_unique_copy(self.transferred_states.iter().copied(), "transferred state");
        assert!(
            self.technology_bonuses.iter().all(|bonus| bonus.uses > 0),
            "technology bonus must retain at least one use"
        );
        self.world_state.assert_invariants();

        for (index, (definition, state)) in
            self.state_defs.iter().zip(self.states.iter()).enumerate()
        {
            assert_eq!(usize::from(definition.id.0), index);
            assert!(state.infrastructure <= 10);
            assert!(state.land_fort_level <= 10);
            assert_unique_strs(
                self.state_flags[index].iter().map(Box::as_ref),
                "state flag",
            );
        }

        for line in &self.production_lines {
            assert!(line.unit_cost_centi > 0);
            assert!(line.efficiency_permille >= self.production_efficiency_floor_permille());
        }

        for slot in &self.research_slots {
            if self.generic_research_mode() {
                assert!(slot.technology.is_none());
            } else {
                assert_eq!(slot.branch.is_some(), slot.technology.is_some());
            }
        }

        for idea in &self.active_ideas {
            assert_ne!(idea.remaining_days, Some(0));
        }

        if !self.fielded_force.is_empty() {
            let exact_fielded_divisions = self
                .fielded_force
                .iter()
                .filter(|division| division.target_demand.has_equipment())
                .count();
            let exact_fielded_demand =
                self.fielded_force
                    .iter()
                    .fold(EquipmentDemand::default(), |total, division| {
                        assert_eq!(
                            division.equipped_demand.manpower,
                            division.target_demand.manpower
                        );
                        total.plus(division.target_demand)
                    });

            assert_eq!(
                self.fielded_divisions,
                u16::try_from(exact_fielded_divisions).unwrap_or(u16::MAX)
            );
            assert_eq!(self.fielded_demand, exact_fielded_demand);
        }
    }

    pub fn state_index(&self, state: StateId) -> usize {
        let index = usize::from(state.0);
        assert!(index < self.states.len());
        assert_eq!(self.state_defs[index].id, state);
        index
    }

    pub fn state(&self, state: StateId) -> &StateRuntime {
        let index = self.state_index(state);
        &self.states[index]
    }

    pub fn state_mut(&mut self, state: StateId) -> &mut StateRuntime {
        let index = self.state_index(state);
        &mut self.states[index]
    }

    pub fn state_def(&self, state: StateId) -> &StateDefinition {
        let index = self.state_index(state);
        &self.state_defs[index]
    }

    pub fn state_def_mut(&mut self, state: StateId) -> &mut StateDefinition {
        let index = self.state_index(state);
        &mut self.state_defs[index]
    }

    pub fn total_civilian_factories(&self) -> u16 {
        self.states
            .iter()
            .map(|state| u16::from(state.civilian_factories))
            .sum()
    }

    pub fn total_military_factories(&self) -> u16 {
        self.states
            .iter()
            .map(|state| u16::from(state.military_factories))
            .sum()
    }

    pub fn assigned_military_factories(&self) -> u16 {
        self.production_lines
            .iter()
            .map(|line| u16::from(line.factories))
            .sum()
    }

    pub fn unassigned_military_factories(&self) -> u16 {
        self.total_military_factories()
            .saturating_sub(self.assigned_military_factories())
    }

    pub fn queued_factory_projects(&self, state: StateId) -> u8 {
        self.construction_queue
            .iter()
            .filter(|project| project.state == state)
            .filter(|project| {
                matches!(
                    project.kind,
                    ConstructionKind::CivilianFactory | ConstructionKind::MilitaryFactory
                )
            })
            .count() as u8
    }

    pub fn consumer_goods_factories(&self, ideas: &[IdeaDefinition]) -> u16 {
        let mut ratio_bp = i32::from(self.country.laws.economy.consumer_goods_ratio_bp());
        ratio_bp += self.idea_modifiers(ideas).consumer_goods_bp;
        ratio_bp = ratio_bp.clamp(0, 10_000);

        let total_factories =
            u32::from(self.total_civilian_factories() + self.total_military_factories());
        let goods =
            (total_factories * u32::try_from(ratio_bp).unwrap_or_default()).div_ceil(10_000);

        u16::try_from(goods).unwrap_or(u16::MAX)
    }

    pub fn available_civilian_factories(&self, ideas: &[IdeaDefinition]) -> u16 {
        self.total_civilian_factories()
            .saturating_sub(self.consumer_goods_factories(ideas))
    }

    pub fn construction_speed_bp_for(
        &self,
        kind: FocusBuildingKind,
        ideas: &[IdeaDefinition],
    ) -> u16 {
        let mut bonus = self.idea_modifiers(ideas).construction_bonus_bp(kind)
            + i32::from(self.country.laws.trade.construction_speed_bp());
        bonus += match kind {
            FocusBuildingKind::CivilianFactory => {
                i32::from(self.country.laws.economy.civilian_factory_construction_bp())
            }
            FocusBuildingKind::MilitaryFactory => {
                i32::from(self.country.laws.economy.military_factory_construction_bp())
            }
            FocusBuildingKind::Infrastructure | FocusBuildingKind::LandFort => 0,
        };
        if self.generic_research_mode() {
            bonus += i32::from(self.completed_research.construction) * 200;
            bonus += i32::from(self.completed_research.industry) * 100;
        } else {
            bonus += self.technology_modifiers.construction_speed_bp;
        }

        if self.advisors.industry {
            bonus += 100;
        }

        u16::try_from(bonus.clamp(0, i32::from(u16::MAX))).unwrap_or(u16::MAX)
    }

    pub fn military_output_bp(&self, ideas: &[IdeaDefinition]) -> u16 {
        let mut bonus = self.idea_modifiers(ideas).factory_output_bp
            + i32::from(self.country.laws.trade.factory_output_bp());
        if self.generic_research_mode() {
            bonus += i32::from(self.completed_research.production) * 250;
        } else {
            bonus += self.technology_modifiers.factory_output_bp;
        }

        if self.advisors.military_industry {
            bonus += 100;
        }

        u16::try_from(bonus.clamp(0, i32::from(u16::MAX))).unwrap_or(u16::MAX)
    }

    pub fn political_power_daily_bonus_centi(&self, ideas: &[IdeaDefinition]) -> u16 {
        let mut bonus = self.idea_modifiers(ideas).political_power_daily_centi;

        if self.advisors.research {
            bonus += 25;
        }

        u16::try_from(bonus.clamp(0, i32::from(u16::MAX))).unwrap_or(u16::MAX)
    }

    pub fn idea_modifiers(&self, ideas: &[IdeaDefinition]) -> IdeaModifiers {
        self.active_ideas
            .iter()
            .filter_map(|active| ideas.iter().find(|idea| idea.id == active.id))
            .fold(IdeaModifiers::default(), |total, idea| {
                total.plus(idea.modifiers)
            })
    }

    pub fn current_stability_bp(&self, ideas: &[IdeaDefinition]) -> u16 {
        let value = i32::from(self.country.stability_bp) + self.idea_modifiers(ideas).stability_bp;
        u16::try_from(value.clamp(0, 10_000)).unwrap_or(10_000)
    }

    pub fn current_war_support_bp(&self, ideas: &[IdeaDefinition]) -> u16 {
        let value =
            i32::from(self.country.war_support_bp) + self.idea_modifiers(ideas).war_support_bp;
        u16::try_from(value.clamp(0, 10_000)).unwrap_or(10_000)
    }

    pub fn available_manpower(&self, ideas: &[IdeaDefinition]) -> u64 {
        let modifiers = self.idea_modifiers(ideas);
        let recruitable_bp = i32::from(self.country.laws.mobilization.manpower_permyriad())
            + modifiers.recruitable_population_bp;
        let recruitable_bp = u64::try_from(recruitable_bp.clamp(0, i32::MAX)).unwrap_or_default();
        let base = self.country.population.saturating_mul(recruitable_bp) / 10_000;
        let modifier_bp = 10_000 + modifiers.manpower_bp;
        let modifier_bp = u64::try_from(modifier_bp.clamp(0, i32::MAX)).unwrap_or_default();
        base.saturating_mul(modifier_bp) / 10_000
    }

    pub fn next_daily_stability_drift_bp(&mut self, ideas: &[IdeaDefinition]) -> i32 {
        self.stability_weekly_accumulator_bp += self.idea_modifiers(ideas).stability_weekly_bp;
        let daily_bp = self.stability_weekly_accumulator_bp / 7;
        self.stability_weekly_accumulator_bp -= daily_bp * 7;
        daily_bp
    }

    pub fn add_idea(&mut self, id: impl Into<Box<str>>, duration_days: Option<u16>) {
        let id = id.into();
        if let Some(existing) = self.active_ideas.iter_mut().find(|idea| idea.id == id) {
            existing.remaining_days = duration_days;
            return;
        }

        self.active_ideas.push(ActiveIdea {
            id,
            remaining_days: duration_days,
        });
    }

    pub fn remove_idea(&mut self, id: &str) {
        self.active_ideas.retain(|idea| idea.id.as_ref() != id);
    }

    pub fn add_doctrine_cost_reduction(&mut self, reduction: DoctrineCostReduction) {
        if self
            .doctrine_cost_reductions
            .iter()
            .any(|current| current.name == reduction.name && current.category == reduction.category)
        {
            return;
        }
        self.doctrine_cost_reductions.push(reduction);
    }

    pub fn has_idea(&self, id: &str) -> bool {
        self.active_ideas.iter().any(|idea| idea.id.as_ref() == id)
    }

    /// Decrement remaining_days for timed ideas and remove those that reach zero.
    /// Called after advance_day, so a 1-day idea survives through its creation day
    /// and is removed at the start of the next day.
    pub fn tick_active_ideas(&mut self) {
        for idea in &mut self.active_ideas {
            let Some(days) = idea.remaining_days else {
                continue;
            };
            idea.remaining_days = Some(days.saturating_sub(1));
        }
        self.active_ideas
            .retain(|idea| idea.remaining_days != Some(0));
    }

    /// Remove country flags whose absolute expiry date has been reached.
    /// A flag with `expires_on = D` is removed when `date >= D`, which matches
    /// the timed-idea semantics: both expire at the start of their deadline day.
    pub fn prune_expired_country_flags(&mut self) {
        self.country_flags.retain(|flag| match flag.expires_on {
            Some(expires_on) => self.country.date < expires_on,
            None => true,
        });
    }

    pub fn apply_timeline_events(&mut self, events: &[TimelineEvent]) {
        for event in events {
            if event.date() != self.country.date {
                continue;
            }
            self.world_state.apply_event(event);
        }
    }

    pub fn record_focus_completion(&mut self, id: impl Into<Box<str>>) {
        let id = id.into();
        if self.completed_focuses.iter().any(|focus| focus.id == id) {
            return;
        }

        self.completed_focuses.push(CompletedFocus {
            id,
            completed_on: self.country.date,
        });
    }

    pub fn apply_research_completion(&mut self, branch: ResearchBranch) {
        match branch {
            ResearchBranch::Industry => self.completed_research.industry += 1,
            ResearchBranch::Construction => self.completed_research.construction += 1,
            ResearchBranch::Electronics => self.completed_research.electronics += 1,
            ResearchBranch::Production => self.completed_research.production += 1,
        }
    }

    pub fn apply_technology_completion(&mut self, node: &TechnologyNode) {
        if node.id.index() >= self.completed_technologies.len()
            || self.completed_technologies[node.id.index()]
        {
            return;
        }

        self.completed_technologies[node.id.index()] = true;
        self.consume_technology_bonuses(node);
        self.technology_modifiers = self.technology_modifiers.plus(node.modifiers);
        self.apply_research_completion(node.branch);
        let efficiency_floor_permille = self.production_efficiency_floor_permille();
        for line in &mut self.production_lines {
            line.efficiency_permille = line.efficiency_permille.max(efficiency_floor_permille);
        }
        for unlock in node.equipment_unlocks.iter().copied() {
            self.equipment_profiles.set(unlock.kind, unlock.profile);
            for line in &mut self.production_lines {
                if line.equipment == unlock.kind {
                    line.unit_cost_centi = unlock.profile.unit_cost_centi;
                }
            }
        }
    }

    pub fn initialize_completed_technologies(
        &mut self,
        technology_tree: &TechnologyTree,
        completed_technologies: Box<[bool]>,
    ) {
        assert_eq!(technology_tree.len(), completed_technologies.len());
        self.completed_technologies = completed_technologies;
        self.technology_modifiers = TechnologyModifiers::default();
        self.completed_research = ResearchSummary::default();
        for (index, completed) in self.completed_technologies.clone().iter().enumerate() {
            if !*completed {
                continue;
            }
            self.completed_technologies[index] = false;
            self.apply_technology_completion(
                technology_tree.node(TechId(u16::try_from(index).unwrap_or(u16::MAX))),
            );
        }
        self.assert_invariants();
    }

    pub fn frontier_forts_complete(&self, requirements: &[FrontierFortRequirement]) -> bool {
        requirements.iter().all(|requirement| {
            self.state_defs
                .iter()
                .zip(self.states.iter())
                .filter(|(definition, _)| definition.frontier == Some(requirement.frontier))
                .all(|(_, runtime)| runtime.land_fort_level >= requirement.target_level)
        })
    }

    pub fn domestic_resources(&self, ideas: &[IdeaDefinition]) -> ResourceLedger {
        let base = self
            .state_defs
            .iter()
            .fold(ResourceLedger::default(), |total, state| {
                total.plus(state.resources)
            });
        let modifier_bp = i32::from(self.country.laws.trade.local_resource_retention_bp())
            + self.idea_modifiers(ideas).resource_factor_bp
            + self.technology_modifiers.local_resources_bp;
        let modifier_bp = u16::try_from(modifier_bp.clamp(0, i32::from(u16::MAX))).unwrap_or(0);
        base.scale_bp(modifier_bp)
    }

    pub fn daily_resource_demand(
        &self,
        equipment_profiles: ModeledEquipmentProfiles,
    ) -> ResourceLedger {
        self.production_lines
            .iter()
            .fold(ResourceLedger::default(), |total, line| {
                total.plus(line.daily_resource_demand(equipment_profiles))
            })
    }

    pub fn ready_divisions(&self, demand: EquipmentDemand, ideas: &[IdeaDefinition]) -> u16 {
        self.stockpile
            .ready_divisions(demand, self.available_manpower(ideas))
    }

    pub fn supported_divisions(&self, demand: EquipmentDemand, ideas: &[IdeaDefinition]) -> u16 {
        let free_manpower = self
            .available_manpower(ideas)
            .saturating_sub(u64::from(self.fielded_demand.manpower));
        if self.fielded_force.is_empty() {
            return self
                .fielded_divisions
                .saturating_add(self.stockpile.ready_divisions(demand, free_manpower));
        }

        let mut remaining_stockpile = self.stockpile;
        let mut ready_fielded = 0_u16;

        for division in self.fielded_force.iter() {
            if !division.target_demand.has_equipment() {
                continue;
            }
            let gap = division.reinforcement_gap();
            if !gap.has_equipment() {
                ready_fielded = ready_fielded.saturating_add(1);
                continue;
            }
            if remaining_stockpile.covers(gap) {
                remaining_stockpile = remaining_stockpile.saturating_sub_demand(gap);
                ready_fielded = ready_fielded.saturating_add(1);
            }
        }

        ready_fielded.saturating_add(remaining_stockpile.ready_divisions(demand, free_manpower))
    }

    pub fn queued_kind_projects(&self, state: StateId, kind: ConstructionKind) -> u8 {
        self.construction_queue
            .iter()
            .filter(|project| project.state == state && project.kind == kind)
            .count() as u8
    }

    pub fn has_completed_focus(&self, id: &str) -> bool {
        self.completed_focuses
            .iter()
            .any(|focus| focus.id.as_ref() == id)
    }

    pub fn completed_focus_by(&self, id: &str, deadline: GameDate) -> bool {
        self.completed_focuses
            .iter()
            .any(|focus| focus.id.as_ref() == id && focus.completed_on <= deadline)
    }

    pub fn has_country_flag(&self, flag: &str) -> bool {
        self.country_flags
            .iter()
            .any(|value| value.id.as_ref() == flag)
    }

    pub fn set_country_flag(&mut self, flag: impl Into<Box<str>>, expires_on: Option<GameDate>) {
        let flag = flag.into();
        self.country_flags.retain(|current| current.id != flag);
        self.country_flags.push(ActiveCountryFlag {
            id: flag,
            expires_on,
        });
    }

    pub fn add_country_leader_trait(&mut self, trait_id: impl Into<Box<str>>) {
        let trait_id = trait_id.into();
        if self
            .country_leader_traits
            .iter()
            .any(|current| current == &trait_id)
        {
            return;
        }
        self.country_leader_traits.push(trait_id);
    }

    pub fn has_state_flag_by_index(&self, index: usize, flag: &str) -> bool {
        self.state_flags[index]
            .iter()
            .any(|value| value.as_ref() == flag)
    }

    pub fn set_state_flag_by_index(&mut self, index: usize, flag: impl Into<Box<str>>) {
        let flag = flag.into();
        if self.state_flags[index]
            .iter()
            .any(|current| current == &flag)
        {
            return;
        }
        self.state_flags[index].push(flag);
    }

    pub fn research_speed_bp(&self, ideas: &[IdeaDefinition]) -> u16 {
        let bonus = self.idea_modifiers(ideas).research_speed_bp
            + i32::from(self.country.laws.trade.research_speed_bp())
            + self.technology_modifiers.research_speed_bp;
        u16::try_from(bonus.clamp(0, i32::from(u16::MAX))).unwrap_or(u16::MAX)
    }

    pub fn production_efficiency_floor_permille(&self) -> u16 {
        let floor = i32::from(BASE_PRODUCTION_EFFICIENCY_PERMILLE)
            + self
                .technology_modifiers
                .production_start_efficiency_permille;
        u16::try_from(floor.clamp(
            i32::from(BASE_PRODUCTION_EFFICIENCY_PERMILLE),
            i32::from(u16::MAX),
        ))
        .unwrap_or(u16::MAX)
    }

    pub fn production_efficiency_cap_permille(&self, base_cap_permille: u16) -> u16 {
        let floor = i32::from(self.production_efficiency_floor_permille());
        let cap = i32::from(base_cap_permille)
            + self.technology_modifiers.production_efficiency_cap_permille;
        u16::try_from(cap.clamp(floor, i32::from(u16::MAX))).unwrap_or(u16::MAX)
    }

    pub fn production_efficiency_gain_permille(&self, base_gain_permille: u16) -> u16 {
        let scaled = (u32::from(base_gain_permille)
            * u32::try_from(
                (10_000 + self.technology_modifiers.production_efficiency_gain_bp)
                    .clamp(0, i32::from(u16::MAX)),
            )
            .unwrap_or_default())
        .div_ceil(10_000);
        u16::try_from(scaled.max(1)).unwrap_or(u16::MAX)
    }

    pub fn generic_research_mode(&self) -> bool {
        self.completed_technologies.is_empty()
    }
}

fn assert_unique_strs<'a>(values: impl IntoIterator<Item = &'a str>, label: &str) {
    let mut seen: Vec<&str> = Vec::new();
    for value in values {
        assert!(
            seen.iter().all(|existing| existing != &value),
            "duplicate {label}: {value}",
        );
        seen.push(value);
    }
}

fn assert_unique_pairs<'a>(values: impl IntoIterator<Item = (&'a str, &'a str)>, label: &str) {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for value in values {
        assert!(
            seen.iter().all(|existing| existing != &value),
            "duplicate {label}: {} / {}",
            value.0,
            value.1,
        );
        seen.push(value);
    }
}

fn assert_unique_copy<T: Copy + Eq>(values: impl IntoIterator<Item = T>, label: &str) {
    let mut seen: Vec<T> = Vec::new();
    for value in values {
        assert!(
            seen.iter().all(|existing| existing != &value),
            "duplicate {label}",
        );
        seen.push(value);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrancePlanningState {
    pub country: CountryState,
    pub pivot_date: GameDate,
    pub military_factory_target_met: bool,
    pub frontier_forts_met: bool,
}

impl FrancePlanningState {
    pub fn phase(&self) -> StrategicPhase {
        if self.country.date < self.pivot_date {
            StrategicPhase::PrePivot
        } else {
            StrategicPhase::PostPivot
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::domain::{
        CountryLaws, DivisionTemplate, EquipmentDemand, EquipmentKind, FieldedDivision,
        FocusBuildingKind, GameDate, IdeaDefinition, IdeaModifiers, ModeledEquipmentProfiles,
        ResourceLedger, TechnologyModifiers, TimelineEvent, TradeLaw,
    };
    use crate::scenario::{France1936Scenario, Frontier};
    use crate::sim::actions::{ConstructionKind, StateId};

    use super::{
        BASE_PRODUCTION_EFFICIENCY_PERMILLE, CountryRuntime, CountryState, FrancePlanningState,
        POLITICAL_POWER_UNIT, ProductionLine, StateDefinition, StateRuntime, Stockpile,
        StrategicPhase,
    };

    fn test_runtime() -> CountryRuntime {
        CountryRuntime::new(
            CountryState::new(
                GameDate::new(1936, 1, 1),
                40_000_000,
                CountryLaws::default(),
            ),
            vec![
                StateDefinition {
                    id: StateId(0),
                    raw_state_id: 16,
                    name: "paris".into(),
                    building_slots: 12,
                    economic_weight: 10,
                    infrastructure_target: 8,
                    is_core_of_root: true,
                    frontier: None,
                    resources: ResourceLedger::default(),
                },
                StateDefinition {
                    id: StateId(1),
                    raw_state_id: 17,
                    name: "lorraine".into(),
                    building_slots: 8,
                    economic_weight: 7,
                    infrastructure_target: 7,
                    is_core_of_root: true,
                    frontier: Some(Frontier::Germany),
                    resources: ResourceLedger {
                        steel: 12,
                        tungsten: 4,
                        ..ResourceLedger::default()
                    },
                },
            ]
            .into_boxed_slice(),
            vec![
                StateRuntime {
                    civilian_factories: 10,
                    military_factories: 4,
                    infrastructure: 8,
                    land_fort_level: 0,
                },
                StateRuntime {
                    civilian_factories: 4,
                    military_factories: 2,
                    infrastructure: 6,
                    land_fort_level: 5,
                },
            ]
            .into_boxed_slice(),
            vec![ProductionLine::new(EquipmentKind::InfantryEquipment, 5)].into_boxed_slice(),
        )
    }

    fn equipment_demand(
        infantry_equipment: u16,
        support_equipment: u16,
        artillery: u16,
        anti_tank: u16,
        anti_air: u16,
        manpower: u16,
    ) -> EquipmentDemand {
        EquipmentDemand {
            infantry_equipment: u32::from(infantry_equipment.max(1)),
            support_equipment: u32::from(support_equipment),
            artillery: u32::from(artillery),
            anti_tank: u32::from(anti_tank),
            anti_air: u32::from(anti_air),
            manpower: u32::from(manpower.max(1)),
            ..EquipmentDemand::default()
        }
    }

    #[test]
    fn country_state_advances_daily_time_and_political_power() {
        let mut country = CountryState::new(
            GameDate::new(1936, 1, 1),
            40_000_000,
            CountryLaws::default(),
        );
        country.advance_day(0, 0);

        assert_eq!(country.date, GameDate::new(1936, 1, 2));
        assert_eq!(country.political_power_centi, 2 * POLITICAL_POWER_UNIT);
    }

    #[test]
    fn country_state_uses_mobilization_law_for_manpower() {
        let country = CountryState::new(
            GameDate::new(1936, 1, 1),
            40_000_000,
            CountryLaws::default(),
        );

        assert_eq!(country.available_manpower(), 1_000_000);
    }

    #[test]
    fn country_runtime_uses_dense_state_ids() {
        let runtime = test_runtime();

        assert_eq!(runtime.state(StateId(1)).military_factories, 2);
    }

    #[test]
    fn country_runtime_fixture_satisfies_invariants() {
        test_runtime().assert_invariants();
    }

    #[test]
    fn country_runtime_counts_consumer_goods_from_total_factories() {
        let runtime = test_runtime();
        let ideas = &[];

        assert_eq!(runtime.total_civilian_factories(), 14);
        assert_eq!(runtime.total_military_factories(), 6);
        assert_eq!(runtime.consumer_goods_factories(ideas), 7);
        assert_eq!(runtime.available_civilian_factories(ideas), 7);
    }

    #[test]
    fn consumer_goods_floor_clamps_at_zero_under_large_negative_modifiers() {
        let mut runtime = test_runtime();
        let ideas = [IdeaDefinition {
            id: "FRA_zero_consumer_goods".into(),
            modifiers: IdeaModifiers {
                consumer_goods_bp: -20_000,
                ..IdeaModifiers::default()
            },
        }];
        runtime.add_idea("FRA_zero_consumer_goods", None);

        assert_eq!(runtime.consumer_goods_factories(&ideas), 0);
        assert_eq!(
            runtime.available_civilian_factories(&ideas),
            runtime.total_civilian_factories(),
        );
    }

    #[test]
    fn country_runtime_applies_flat_and_scaled_manpower_modifiers() {
        let runtime = test_runtime();
        let ideas = [IdeaDefinition {
            id: "FRA_service_reform".into(),
            modifiers: IdeaModifiers {
                recruitable_population_bp: 300,
                manpower_bp: 2_500,
                ..IdeaModifiers::default()
            },
        }];
        let mut runtime = runtime;
        runtime.add_idea("FRA_service_reform", None);

        assert_eq!(runtime.available_manpower(&ideas), 2_750_000);
    }

    #[test]
    fn country_runtime_clamps_support_levels_from_ideas_into_valid_ranges() {
        let mut runtime = test_runtime();
        runtime.country.stability_bp = 9_800;
        runtime.country.war_support_bp = 200;
        let ideas = [IdeaDefinition {
            id: "FRA_support_clamps".into(),
            modifiers: IdeaModifiers {
                stability_bp: 700,
                war_support_bp: -2_000,
                ..IdeaModifiers::default()
            },
        }];
        runtime.add_idea("FRA_support_clamps", None);

        assert_eq!(runtime.current_stability_bp(&ideas), 10_000);
        assert_eq!(runtime.current_war_support_bp(&ideas), 0);
    }

    #[test]
    fn country_runtime_clamps_factory_output_floor_at_zero() {
        let mut runtime = test_runtime();
        let ideas = [IdeaDefinition {
            id: "FRA_output_floor".into(),
            modifiers: IdeaModifiers {
                factory_output_bp: -20_000,
                ..IdeaModifiers::default()
            },
        }];
        runtime.add_idea("FRA_output_floor", None);

        assert_eq!(runtime.military_output_bp(&ideas), 0);
    }

    #[test]
    fn country_runtime_clamps_research_speed_floor_at_zero() {
        let mut runtime = test_runtime();
        let ideas = [IdeaDefinition {
            id: "FRA_research_floor".into(),
            modifiers: IdeaModifiers {
                research_speed_bp: -20_000,
                ..IdeaModifiers::default()
            },
        }];
        runtime.add_idea("FRA_research_floor", None);

        assert_eq!(runtime.research_speed_bp(&ideas), 0);
    }

    #[test]
    fn country_runtime_accumulates_weekly_stability_without_losing_basis_points() {
        let mut runtime = test_runtime();
        let ideas = [IdeaDefinition {
            id: "FRA_home_front".into(),
            modifiers: IdeaModifiers {
                stability_weekly_bp: 25,
                ..IdeaModifiers::default()
            },
        }];
        runtime.add_idea("FRA_home_front", None);

        for _ in 0..14 {
            let drift_bp = runtime.next_daily_stability_drift_bp(&ideas);
            runtime.country.advance_day(0, drift_bp);
        }

        assert_eq!(runtime.country.stability_bp, 5_050);
        assert_eq!(runtime.stability_weekly_accumulator_bp, 0);
    }

    #[test]
    fn stockpile_converts_equipment_into_ready_divisions() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.per_division_demand();
        let stockpile = Stockpile {
            infantry_equipment: demand.infantry_equipment * 3,
            support_equipment: demand.support_equipment * 3,
            artillery: demand.artillery * 3,
            anti_tank: demand.anti_tank * 3,
            anti_air: demand.anti_air * 3,
            unmodeled_equipment: 0,
            ..Stockpile::default()
        };

        assert_eq!(stockpile.ready_divisions(demand, 500_000), 3);
    }

    #[test]
    fn france_planning_state_enters_post_pivot_on_pivot_day() {
        let country = CountryState::new(
            GameDate::new(1938, 6, 1),
            40_000_000,
            CountryLaws::default(),
        );
        let planning = FrancePlanningState {
            country,
            pivot_date: GameDate::new(1938, 6, 1),
            military_factory_target_met: false,
            frontier_forts_met: false,
        };

        assert_eq!(planning.phase(), StrategicPhase::PostPivot);
    }

    #[test]
    fn runtime_detects_when_frontier_fort_requirements_are_satisfied() {
        let scenario = France1936Scenario::standard();
        let runtime = scenario.bootstrap_runtime();

        assert!(!runtime.frontier_forts_complete(&scenario.frontier_forts));
    }

    #[test]
    fn runtime_aggregates_trade_law_adjusted_domestic_resources() {
        let runtime = test_runtime();
        let ideas = &[];

        assert_eq!(
            runtime.domestic_resources(ideas),
            ResourceLedger {
                steel: 6,
                tungsten: 2,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn runtime_closed_economy_exposes_full_domestic_resources() {
        let mut runtime = test_runtime();
        runtime.country.laws.trade = TradeLaw::ClosedEconomy;

        assert_eq!(
            runtime.domestic_resources(&[]),
            ResourceLedger {
                steel: 12,
                tungsten: 4,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn runtime_free_trade_applies_raw_trade_law_bonuses() {
        let mut runtime = test_runtime();
        runtime.country.laws.trade = TradeLaw::FreeTrade;
        runtime.country.laws.economy = crate::domain::EconomyLaw::WarEconomy;

        assert_eq!(
            runtime.domestic_resources(&[]),
            ResourceLedger {
                steel: 2,
                tungsten: 0,
                ..ResourceLedger::default()
            }
        );
        assert_eq!(
            runtime.construction_speed_bp_for(FocusBuildingKind::CivilianFactory, &[]),
            1_500
        );
        assert_eq!(runtime.military_output_bp(&[]), 1_500);
        assert_eq!(runtime.research_speed_bp(&[]), 1_000);
    }

    #[test]
    fn runtime_exact_technology_modifiers_affect_construction_research_and_output() {
        let mut runtime = test_runtime();
        runtime.country.laws.trade = TradeLaw::ClosedEconomy;
        runtime.completed_technologies = vec![false].into_boxed_slice();
        runtime.technology_modifiers = TechnologyModifiers {
            construction_speed_bp: 600,
            research_speed_bp: 350,
            factory_output_bp: 450,
            ..TechnologyModifiers::default()
        };

        assert_eq!(
            runtime.construction_speed_bp_for(FocusBuildingKind::Infrastructure, &[]),
            600
        );
        assert_eq!(runtime.military_output_bp(&[]), 450);
        assert_eq!(runtime.research_speed_bp(&[]), 350);
    }

    #[test]
    fn runtime_domestic_resources_apply_trade_law_and_local_resource_bonus() {
        let mut runtime = test_runtime();
        runtime.country.laws.trade = TradeLaw::LimitedExports;
        let ideas = [IdeaDefinition {
            id: "FRA_resource_policy".into(),
            modifiers: IdeaModifiers {
                resource_factor_bp: 1_500,
                ..IdeaModifiers::default()
            },
        }];
        runtime.add_idea("FRA_resource_policy", None);

        assert_eq!(
            runtime.domestic_resources(&ideas),
            ResourceLedger {
                steel: 10,
                tungsten: 3,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn runtime_aggregates_daily_resource_demand_from_production_lines() {
        let runtime = test_runtime();
        let profiles = ModeledEquipmentProfiles::default_1936();

        assert_eq!(
            runtime.daily_resource_demand(profiles),
            ResourceLedger {
                steel: 10,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn supported_divisions_include_fielded_force_and_reserve_stockpile() {
        let template = DivisionTemplate::canonical_france_line();
        let demand = template.per_division_demand();
        let runtime = test_runtime().with_fielded_force(24, demand);
        let mut stocked_runtime = runtime.clone();
        stocked_runtime.stockpile = Stockpile {
            infantry_equipment: demand.infantry_equipment * 2,
            support_equipment: demand.support_equipment * 2,
            artillery: demand.artillery * 2,
            anti_tank: demand.anti_tank * 2,
            anti_air: demand.anti_air * 2,
            unmodeled_equipment: 0,
            ..Stockpile::default()
        };

        assert_eq!(stocked_runtime.supported_divisions(demand, &[]), 26);
    }

    #[test]
    fn exact_fielded_force_requires_reinforcement_before_counting_understrength_divisions() {
        let demand = EquipmentDemand {
            infantry_equipment: 1_000,
            support_equipment: 0,
            artillery: 0,
            anti_tank: 0,
            anti_air: 0,
            manpower: 1_000,
            ..EquipmentDemand::default()
        };
        let runtime = test_runtime().with_exact_fielded_force(
            vec![
                FieldedDivision::new(demand, demand),
                FieldedDivision::new(demand, demand.scale_equipment_basis_points(5_000)),
            ]
            .into_boxed_slice(),
        );
        let mut reinforced = runtime.clone();
        reinforced.stockpile.infantry_equipment = 500;

        assert_eq!(runtime.supported_divisions(demand, &[]), 1);
        assert_eq!(reinforced.supported_divisions(demand, &[]), 2);
    }

    #[test]
    fn exact_fielded_force_ignores_unmodeled_only_divisions_in_readiness_count() {
        let demand = EquipmentDemand {
            infantry_equipment: 1_000,
            support_equipment: 0,
            artillery: 0,
            anti_tank: 0,
            anti_air: 0,
            manpower: 1_000,
            ..EquipmentDemand::default()
        };
        let armor_only = EquipmentDemand {
            manpower: 500,
            ..EquipmentDemand::default()
        };
        let runtime = test_runtime().with_exact_fielded_force(
            vec![
                FieldedDivision::new(demand, demand),
                FieldedDivision::new(armor_only, armor_only),
            ]
            .into_boxed_slice(),
        );

        assert_eq!(runtime.fielded_divisions, 1);
        assert_eq!(runtime.fielded_demand.manpower, 1_500);
        assert_eq!(runtime.supported_divisions(demand, &[]), 1);
    }

    #[test]
    fn queued_factory_project_count_ignores_non_slot_buildings() {
        let mut runtime = test_runtime();
        runtime.construction_queue.push(super::ConstructionProject {
            state: StateId(0),
            kind: ConstructionKind::CivilianFactory,
            total_cost_centi: 1,
            progress_centi: 0,
        });
        runtime.construction_queue.push(super::ConstructionProject {
            state: StateId(0),
            kind: ConstructionKind::Infrastructure,
            total_cost_centi: 1,
            progress_centi: 0,
        });

        assert_eq!(runtime.queued_factory_projects(StateId(0)), 1);
    }

    #[test]
    fn production_line_reassignment_respects_base_efficiency_floor() {
        let mut line = ProductionLine::new(EquipmentKind::InfantryEquipment, 5);
        line.efficiency_permille = 640;
        line.accumulated_ic_centi = 1_234;

        line.reassign(
            EquipmentKind::Artillery,
            4,
            EquipmentKind::Artillery.default_unit_cost_centi(),
            BASE_PRODUCTION_EFFICIENCY_PERMILLE,
        );

        assert_eq!(
            line.efficiency_permille,
            BASE_PRODUCTION_EFFICIENCY_PERMILLE,
        );
        assert_eq!(line.accumulated_ic_centi, 0);
        assert_eq!(line.factories, 4);
        assert_eq!(line.equipment, EquipmentKind::Artillery);
    }

    #[test]
    fn production_line_reports_daily_resource_demand_from_equipment_profile() {
        let line = ProductionLine::new(EquipmentKind::Artillery, 3);
        let profiles = ModeledEquipmentProfiles::default_1936();

        assert_eq!(
            line.daily_resource_demand(profiles),
            ResourceLedger {
                steel: 6,
                tungsten: 3,
                ..ResourceLedger::default()
            }
        );
    }

    #[test]
    fn add_idea_replaces_existing_entry_instead_of_stacking() {
        let mut runtime = test_runtime();

        runtime.add_idea("FRA_repeatable_spirit", Some(14));
        runtime.add_idea("FRA_repeatable_spirit", None);
        assert_eq!(runtime.active_ideas.len(), 1);
        assert_eq!(runtime.active_ideas[0].remaining_days, None);
        runtime.add_idea("FRA_repeatable_spirit", Some(5));

        assert_eq!(runtime.active_ideas.len(), 1);
        assert_eq!(runtime.active_ideas[0].remaining_days, Some(5));
    }

    #[test]
    fn timed_country_flags_expire_on_their_deadline() {
        let mut runtime = test_runtime();

        runtime.set_country_flag(
            "FRA_popular_front_cooldown",
            Some(GameDate::new(1937, 6, 10)),
        );
        runtime.country.date = GameDate::new(1937, 6, 9);
        runtime.prune_expired_country_flags();
        assert!(runtime.has_country_flag("FRA_popular_front_cooldown"));

        runtime.country.date = GameDate::new(1937, 6, 10);
        runtime.prune_expired_country_flags();
        assert!(!runtime.has_country_flag("FRA_popular_front_cooldown"));
    }

    #[test]
    fn timed_flag_and_timed_idea_expire_on_same_tick() {
        // Both mechanisms must agree: a 1-day timed idea and a flag with
        // expires_on = start + 1 day should both vanish on the same tick.
        let mut runtime = test_runtime();
        let start = GameDate::new(1937, 1, 1);
        runtime.country.date = start;

        runtime.add_idea("FRA_test_timed", Some(1));
        runtime.set_country_flag("FRA_test_flag", Some(start.add_days(1)));

        // Simulate one advance_day + tick cycle (matches engine loop order).
        runtime.country.advance_day(0, 0);
        runtime.tick_active_ideas();
        runtime.prune_expired_country_flags();

        // Both should be gone after the first day transition.
        assert!(!runtime.has_idea("FRA_test_timed"));
        assert!(!runtime.has_country_flag("FRA_test_flag"));
    }

    #[test]
    fn timeline_events_update_world_state_on_matching_date() {
        let mut runtime = test_runtime();
        let events = vec![
            TimelineEvent::DissolveCountry {
                date: GameDate::new(1938, 3, 12),
                tag: "AUS".into(),
            },
            TimelineEvent::StartWar {
                date: GameDate::new(1939, 9, 3),
                left: "FRA".into(),
                right: "GER".into(),
            },
        ];

        runtime.country.date = GameDate::new(1938, 3, 12);
        runtime.apply_timeline_events(&events);
        assert!(!runtime.world_state.country_exists("AUS"));
        assert!(!runtime.world_state.countries_at_war("FRA", "GER"));

        runtime.country.date = GameDate::new(1939, 9, 3);
        runtime.apply_timeline_events(&events);
        assert!(runtime.world_state.countries_at_war("FRA", "GER"));
    }

    proptest! {
        #[test]
        fn stockpile_ready_divisions_is_monotonic_with_more_stockpile_and_manpower(
            demand in (1u16..200, 0u16..50, 0u16..50, 0u16..50, 0u16..50, 1u16..20_000),
            stock in (0u16..500, 0u16..100, 0u16..100, 0u16..100, 0u16..100),
            deltas in (0u16..500, 0u16..100, 0u16..100, 0u16..100, 0u16..100),
            manpower_delta in 0u32..500_000,
            manpower in 1u32..1_000_000,
        ) {
            let demand = equipment_demand(
                demand.0, demand.1, demand.2, demand.3, demand.4, demand.5,
            );
            let base = Stockpile {
                infantry_equipment: u32::from(stock.0),
                support_equipment: u32::from(stock.1),
                artillery: u32::from(stock.2),
                anti_tank: u32::from(stock.3),
                anti_air: u32::from(stock.4),
                unmodeled_equipment: 0,
                ..Stockpile::default()
            };
            let improved = Stockpile {
                infantry_equipment: base.infantry_equipment + u32::from(deltas.0),
                support_equipment: base.support_equipment + u32::from(deltas.1),
                artillery: base.artillery + u32::from(deltas.2),
                anti_tank: base.anti_tank + u32::from(deltas.3),
                anti_air: base.anti_air + u32::from(deltas.4),
                unmodeled_equipment: 0,
                ..Stockpile::default()
            };

            let base_ready = base.ready_divisions(demand, u64::from(manpower));
            let improved_ready =
                improved.ready_divisions(demand, u64::from(manpower) + u64::from(manpower_delta));

            prop_assert!(improved_ready >= base_ready);
        }

        #[test]
        fn weekly_stability_drift_preserves_total_basis_points(
            weekly_bp in -500i32..500,
            days in 0usize..365,
        ) {
            let mut runtime = test_runtime();
            let ideas = [IdeaDefinition {
                id: "FRA_weekly_drift".into(),
                modifiers: IdeaModifiers {
                    stability_weekly_bp: weekly_bp,
                    ..IdeaModifiers::default()
                },
            }];
            runtime.add_idea("FRA_weekly_drift", None);
            let mut total_drift = 0_i32;

            for _ in 0..days {
                let drift_bp = runtime.next_daily_stability_drift_bp(&ideas);
                total_drift += drift_bp;
                runtime.country.advance_day(0, drift_bp);
                runtime.assert_invariants();
            }

            prop_assert_eq!(
                total_drift * 7 + runtime.stability_weekly_accumulator_bp,
                i32::try_from(days).unwrap_or(i32::MAX) * weekly_bp,
            );
            prop_assert!(runtime.stability_weekly_accumulator_bp.abs() < 7);
        }
    }
}
