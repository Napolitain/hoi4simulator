use crate::domain::{EconomyLaw, EquipmentKind, GameDate, MobilizationLaw, TradeLaw};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StateId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionKind {
    CivilianFactory,
    MilitaryFactory,
    Infrastructure,
    LandFort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructionAction {
    pub date: GameDate,
    pub state: StateId,
    pub kind: ConstructionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusBranch {
    Economy,
    Industry,
    MilitaryIndustry,
    Politics,
    Diplomacy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusAction {
    pub date: GameDate,
    pub branch: FocusBranch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LawTarget {
    Economy(EconomyLaw),
    Trade(TradeLaw),
    Mobilization(MobilizationLaw),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LawCategory {
    Economy,
    Trade,
    Mobilization,
}

impl LawCategory {
    pub const COUNT: usize = 3;

    pub const fn index(self) -> usize {
        match self {
            Self::Economy => 0,
            Self::Trade => 1,
            Self::Mobilization => 2,
        }
    }
}

impl LawTarget {
    pub const fn category(self) -> LawCategory {
        match self {
            Self::Economy(_) => LawCategory::Economy,
            Self::Trade(_) => LawCategory::Trade,
            Self::Mobilization(_) => LawCategory::Mobilization,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LawAction {
    pub date: GameDate,
    pub target: LawTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvisorKind {
    IndustryConcern,
    ResearchInstitute,
    MilitaryIndustrialist,
}

impl AdvisorKind {
    pub const COUNT: usize = 3;

    pub const fn index(self) -> usize {
        match self {
            Self::IndustryConcern => 0,
            Self::ResearchInstitute => 1,
            Self::MilitaryIndustrialist => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvisorAction {
    pub date: GameDate,
    pub kind: AdvisorKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResearchBranch {
    Industry,
    Construction,
    Electronics,
    Production,
}

impl ResearchBranch {
    pub const COUNT: usize = 4;

    pub const fn index(self) -> usize {
        match self {
            Self::Industry => 0,
            Self::Construction => 1,
            Self::Electronics => 2,
            Self::Production => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResearchAction {
    pub date: GameDate,
    pub slot: u8,
    pub branch: ResearchBranch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductionAction {
    pub date: GameDate,
    pub slot: u8,
    pub equipment: EquipmentKind,
    pub factories: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Construction(ConstructionAction),
    Production(ProductionAction),
    Focus(FocusAction),
    Law(LawAction),
    Advisor(AdvisorAction),
    Research(ResearchAction),
}

impl Action {
    pub fn date(self) -> GameDate {
        match self {
            Self::Construction(action) => action.date,
            Self::Production(action) => action.date,
            Self::Focus(action) => action.date,
            Self::Law(action) => action.date,
            Self::Advisor(action) => action.date,
            Self::Research(action) => action.date,
        }
    }
}
