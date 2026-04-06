use super::{EquipmentKind, GameDate, GovernmentIdeology, TechnologyBonus, TimelineCondition};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusBuildingKind {
    CivilianFactory,
    MilitaryFactory,
    Infrastructure,
    LandFort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusStateScope {
    AnyState,
    EveryOwnedState,
    RandomControlledState,
    RandomNeighborState,
    RandomOwnedState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateCondition {
    Always,
    All(Vec<StateCondition>),
    Any(Vec<StateCondition>),
    Not(Box<StateCondition>),
    RawStateId(u32),
    IsControlledByRoot,
    IsCoreOfRoot,
    IsOwnedByRoot,
    OwnerIsRootOrSubject,
    HasStateFlag(Box<str>),
    InfrastructureLessThan(u8),
    FreeSharedBuildingSlotsGreaterThan(u8),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusCondition {
    Always,
    All(Vec<FocusCondition>),
    Any(Vec<FocusCondition>),
    Not(Box<FocusCondition>),
    HasCompletedFocus(Box<str>),
    HasCountryFlag(Box<str>),
    HasDlc(Box<str>),
    HasGameRule { rule: Box<str>, option: Box<str> },
    HasGovernment(GovernmentIdeology),
    HasIdea(Box<str>),
    IsInFaction(bool),
    IsPuppet(bool),
    IsSubject(bool),
    OriginalTag(Box<str>),
    Timeline(Box<TimelineCondition>),
    HasWarSupportAtLeast(u16),
    NumOfFactoriesAtLeast(u16),
    NumOfMilitaryFactoriesAtLeast(u16),
    AmountResearchSlotsGreaterThan(u8),
    AmountResearchSlotsLessThan(u8),
    AnyControlledState(Box<StateCondition>),
    AnyOwnedState(Box<StateCondition>),
    AnyState(Box<StateCondition>),
    Unsupported(Box<str>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdeaModifiers {
    pub consumer_goods_bp: i32,
    pub stability_bp: i32,
    pub stability_weekly_bp: i32,
    pub war_support_bp: i32,
    pub political_power_daily_centi: i32,
    pub factory_output_bp: i32,
    pub research_speed_bp: i32,
    pub recruitable_population_bp: i32,
    pub manpower_bp: i32,
    pub resource_factor_bp: i32,
    pub civilian_factory_construction_bp: i32,
    pub military_factory_construction_bp: i32,
    pub infrastructure_construction_bp: i32,
    pub land_fort_construction_bp: i32,
}

impl IdeaModifiers {
    pub fn plus(self, other: Self) -> Self {
        Self {
            consumer_goods_bp: self.consumer_goods_bp + other.consumer_goods_bp,
            stability_bp: self.stability_bp + other.stability_bp,
            stability_weekly_bp: self.stability_weekly_bp + other.stability_weekly_bp,
            war_support_bp: self.war_support_bp + other.war_support_bp,
            political_power_daily_centi: self.political_power_daily_centi
                + other.political_power_daily_centi,
            factory_output_bp: self.factory_output_bp + other.factory_output_bp,
            research_speed_bp: self.research_speed_bp + other.research_speed_bp,
            recruitable_population_bp: self.recruitable_population_bp
                + other.recruitable_population_bp,
            manpower_bp: self.manpower_bp + other.manpower_bp,
            resource_factor_bp: self.resource_factor_bp + other.resource_factor_bp,
            civilian_factory_construction_bp: self.civilian_factory_construction_bp
                + other.civilian_factory_construction_bp,
            military_factory_construction_bp: self.military_factory_construction_bp
                + other.military_factory_construction_bp,
            infrastructure_construction_bp: self.infrastructure_construction_bp
                + other.infrastructure_construction_bp,
            land_fort_construction_bp: self.land_fort_construction_bp
                + other.land_fort_construction_bp,
        }
    }

    pub fn construction_bonus_bp(self, kind: FocusBuildingKind) -> i32 {
        match kind {
            FocusBuildingKind::CivilianFactory => self.civilian_factory_construction_bp,
            FocusBuildingKind::MilitaryFactory => self.military_factory_construction_bp,
            FocusBuildingKind::Infrastructure => self.infrastructure_construction_bp,
            FocusBuildingKind::LandFort => self.land_fort_construction_bp,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaDefinition {
    pub id: Box<str>,
    pub modifiers: IdeaModifiers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoctrineCostReduction {
    pub name: Box<str>,
    pub category: Box<str>,
    pub cost_reduction_bp: u16,
    pub uses: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateOperation {
    AddBuildingConstruction {
        kind: FocusBuildingKind,
        level: u8,
        instant: bool,
    },
    AddExtraSharedBuildingSlots(u8),
    NestedScope(StateScopedEffects),
    SetStateFlag(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateScopedEffects {
    pub scope: FocusStateScope,
    pub limit: StateCondition,
    pub operations: Vec<StateOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusEffect {
    AddIdea(Box<str>),
    AddArmyExperience(u16),
    AddCountryLeaderTrait(Box<str>),
    AddDoctrineCostReduction(DoctrineCostReduction),
    AddManpower(u64),
    AddPoliticalPower(u32),
    AddResearchSlot(u8),
    AddStability(u16),
    AddTimedIdea {
        id: Box<str>,
        days: u16,
    },
    AddWarSupport(u16),
    AddEquipmentToStockpile {
        equipment: EquipmentKind,
        amount: u32,
    },
    AddTechnologyBonus(TechnologyBonus),
    CreateFaction(Box<str>),
    CreateWarGoal {
        target: Box<str>,
        kind: Box<str>,
    },
    JoinFaction(Box<str>),
    RemoveIdea(Box<str>),
    SetCountryRule {
        rule: Box<str>,
        enabled: bool,
    },
    SetCountryFlag {
        flag: Box<str>,
        days: Option<u16>,
    },
    SetPolitics {
        government: GovernmentIdeology,
        elections_allowed: Option<bool>,
        last_election: Option<GameDate>,
    },
    StateScoped(StateScopedEffects),
    SwapIdea {
        remove: Box<str>,
        add: Box<str>,
    },
    TransferState(u32),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NationalFocus {
    pub id: Box<str>,
    pub days: u16,
    pub prerequisites: Vec<Box<str>>,
    pub mutually_exclusive: Vec<Box<str>>,
    pub available: FocusCondition,
    pub bypass: FocusCondition,
    pub search_filters: Vec<Box<str>>,
    pub effects: Vec<FocusEffect>,
}

impl NationalFocus {
    pub fn has_filter(&self, value: &str) -> bool {
        self.search_filters
            .iter()
            .any(|filter| filter.as_ref() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HardFocusGoal {
    pub id: Box<str>,
    pub deadline: GameDate,
}

#[cfg(test)]
mod tests {
    use super::{FocusBuildingKind, IdeaModifiers, NationalFocus};

    #[test]
    fn idea_modifiers_add_componentwise() {
        let left = IdeaModifiers {
            consumer_goods_bp: -1_500,
            factory_output_bp: 500,
            ..IdeaModifiers::default()
        };
        let right = IdeaModifiers {
            consumer_goods_bp: 200,
            research_speed_bp: 1_000,
            ..IdeaModifiers::default()
        };

        let combined = left.plus(right);

        assert_eq!(combined.consumer_goods_bp, -1_300);
        assert_eq!(combined.factory_output_bp, 500);
        assert_eq!(combined.research_speed_bp, 1_000);
    }

    #[test]
    fn national_focus_reports_exact_filter_membership() {
        let focus = NationalFocus {
            id: "FRA_example".into(),
            days: 70,
            prerequisites: Vec::new(),
            mutually_exclusive: Vec::new(),
            available: super::FocusCondition::Always,
            bypass: super::FocusCondition::Always,
            search_filters: vec![
                "FOCUS_FILTER_INDUSTRY".into(),
                "FOCUS_FILTER_RESEARCH".into(),
            ],
            effects: Vec::new(),
        };

        assert!(focus.has_filter("FOCUS_FILTER_INDUSTRY"));
        assert!(!focus.has_filter("FOCUS_FILTER_POLITICAL"));
        assert_eq!(
            IdeaModifiers::default().construction_bonus_bp(FocusBuildingKind::Infrastructure),
            0
        );
    }
}
