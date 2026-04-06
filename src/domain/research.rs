use super::{EquipmentKind, EquipmentProfile};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TechId(pub u16);

impl TechId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TechnologyModifiers {
    pub construction_speed_bp: i32,
    pub local_resources_bp: i32,
    pub research_speed_bp: i32,
    pub factory_output_bp: i32,
    pub production_efficiency_cap_permille: i32,
    pub production_efficiency_gain_bp: i32,
    pub production_start_efficiency_permille: i32,
}

impl TechnologyModifiers {
    pub fn plus(self, other: Self) -> Self {
        Self {
            construction_speed_bp: self.construction_speed_bp + other.construction_speed_bp,
            local_resources_bp: self.local_resources_bp + other.local_resources_bp,
            research_speed_bp: self.research_speed_bp + other.research_speed_bp,
            factory_output_bp: self.factory_output_bp + other.factory_output_bp,
            production_efficiency_cap_permille: self.production_efficiency_cap_permille
                + other.production_efficiency_cap_permille,
            production_efficiency_gain_bp: self.production_efficiency_gain_bp
                + other.production_efficiency_gain_bp,
            production_start_efficiency_permille: self.production_start_efficiency_permille
                + other.production_start_efficiency_permille,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologyBonus {
    pub name: Box<str>,
    pub categories: Box<[Box<str>]>,
    pub bonus_bp: u16,
    pub uses: u8,
}

impl TechnologyBonus {
    pub fn matches(&self, node: &TechnologyNode) -> bool {
        self.categories
            .iter()
            .any(|category| technology_bonus_category_matches(category, node))
    }
}

fn technology_bonus_category_matches(category: &str, node: &TechnologyNode) -> bool {
    node.categories
        .iter()
        .any(|current| current.as_ref() == category)
        || matches!(
            (category, node.branch),
            ("industry", ResearchBranch::Industry) | ("electronics", ResearchBranch::Electronics)
        )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquipmentUnlock {
    pub kind: EquipmentKind,
    pub profile: EquipmentProfile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologyNode {
    pub id: TechId,
    pub token: Box<str>,
    pub branch: ResearchBranch,
    pub categories: Box<[Box<str>]>,
    pub start_year: u16,
    pub base_days: u16,
    pub prerequisites: Box<[TechId]>,
    pub exclusive_with: Box<[TechId]>,
    pub modifiers: TechnologyModifiers,
    pub equipment_unlocks: Box<[EquipmentUnlock]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TechnologyTree {
    nodes: Box<[TechnologyNode]>,
}

impl TechnologyTree {
    pub fn new(nodes: Vec<TechnologyNode>) -> Self {
        for (index, node) in nodes.iter().enumerate() {
            assert_eq!(node.id.index(), index);
        }
        Self {
            nodes: nodes.into_boxed_slice(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn nodes(&self) -> &[TechnologyNode] {
        &self.nodes
    }

    pub fn node(&self, id: TechId) -> &TechnologyNode {
        &self.nodes[id.index()]
    }

    pub fn find_by_token(&self, token: &str) -> Option<TechId> {
        self.nodes
            .iter()
            .find(|node| node.token.as_ref() == token)
            .map(|node| node.id)
    }

    pub fn next_available(
        &self,
        branch: ResearchBranch,
        completed: &[bool],
        active: impl IntoIterator<Item = TechId>,
    ) -> Option<TechId> {
        if completed.len() != self.nodes.len() {
            return None;
        }

        let mut reserved = vec![false; self.nodes.len()];
        for tech_id in active {
            if tech_id.index() < reserved.len() {
                reserved[tech_id.index()] = true;
            }
        }

        self.nodes
            .iter()
            .filter(|node| node.branch == branch)
            .filter(|node| !completed[node.id.index()])
            .filter(|node| !reserved[node.id.index()])
            .filter(|node| {
                node.prerequisites
                    .iter()
                    .all(|prerequisite| completed[prerequisite.index()])
            })
            .filter(|node| {
                node.exclusive_with
                    .iter()
                    .all(|exclusive| !completed[exclusive.index()] && !reserved[exclusive.index()])
            })
            .min_by_key(|node| (node.start_year, node.id.0))
            .map(|node| node.id)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ResearchBranch, TechId, TechnologyBonus, TechnologyModifiers, TechnologyNode,
        TechnologyTree,
    };

    fn test_tree() -> TechnologyTree {
        TechnologyTree::new(vec![
            TechnologyNode {
                id: TechId(0),
                token: "construction1".into(),
                branch: ResearchBranch::Construction,
                categories: vec!["construction_tech".into()].into_boxed_slice(),
                start_year: 1936,
                base_days: 100,
                prerequisites: Vec::new().into_boxed_slice(),
                exclusive_with: Vec::new().into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: Vec::new().into_boxed_slice(),
            },
            TechnologyNode {
                id: TechId(1),
                token: "construction2".into(),
                branch: ResearchBranch::Construction,
                categories: vec!["construction_tech".into()].into_boxed_slice(),
                start_year: 1937,
                base_days: 100,
                prerequisites: vec![TechId(0)].into_boxed_slice(),
                exclusive_with: Vec::new().into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: Vec::new().into_boxed_slice(),
            },
            TechnologyNode {
                id: TechId(2),
                token: "basic_machine_tools".into(),
                branch: ResearchBranch::Industry,
                categories: vec!["industry".into()].into_boxed_slice(),
                start_year: 1936,
                base_days: 100,
                prerequisites: Vec::new().into_boxed_slice(),
                exclusive_with: Vec::new().into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: Vec::new().into_boxed_slice(),
            },
        ])
    }

    #[test]
    fn technology_tree_finds_next_available_by_branch_and_prerequisite() {
        let tree = test_tree();
        let completed = vec![false, false, false];

        assert_eq!(
            tree.next_available(ResearchBranch::Construction, &completed, []),
            Some(TechId(0))
        );
        assert_eq!(
            tree.next_available(ResearchBranch::Industry, &completed, []),
            Some(TechId(2))
        );
    }

    #[test]
    fn technology_tree_skips_completed_and_active_nodes() {
        let tree = test_tree();
        let completed = vec![true, false, false];

        assert_eq!(
            tree.next_available(ResearchBranch::Construction, &completed, []),
            Some(TechId(1))
        );
        assert_eq!(
            tree.next_available(ResearchBranch::Industry, &completed, [TechId(2)]),
            None
        );
    }

    #[test]
    fn technology_tree_respects_mutual_exclusivity() {
        let tree = TechnologyTree::new(vec![
            TechnologyNode {
                id: TechId(0),
                token: "concentrated_industry".into(),
                branch: ResearchBranch::Industry,
                categories: vec!["industry".into()].into_boxed_slice(),
                start_year: 1936,
                base_days: 100,
                prerequisites: Vec::new().into_boxed_slice(),
                exclusive_with: vec![TechId(1)].into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: Vec::new().into_boxed_slice(),
            },
            TechnologyNode {
                id: TechId(1),
                token: "dispersed_industry".into(),
                branch: ResearchBranch::Industry,
                categories: vec!["industry".into()].into_boxed_slice(),
                start_year: 1936,
                base_days: 100,
                prerequisites: Vec::new().into_boxed_slice(),
                exclusive_with: vec![TechId(0)].into_boxed_slice(),
                modifiers: TechnologyModifiers::default(),
                equipment_unlocks: Vec::new().into_boxed_slice(),
            },
        ]);

        assert_eq!(
            tree.next_available(ResearchBranch::Industry, &[true, false], []),
            None
        );
    }

    #[test]
    fn technology_bonus_matches_exact_categories_and_industry_branch() {
        let artillery = TechnologyNode {
            id: TechId(0),
            token: "improved_artillery_upgrade".into(),
            branch: ResearchBranch::Production,
            categories: vec!["artillery".into()].into_boxed_slice(),
            start_year: 1936,
            base_days: 100,
            prerequisites: Vec::new().into_boxed_slice(),
            exclusive_with: Vec::new().into_boxed_slice(),
            modifiers: TechnologyModifiers::default(),
            equipment_unlocks: Vec::new().into_boxed_slice(),
        };
        let industry = TechnologyNode {
            id: TechId(1),
            token: "basic_machine_tools".into(),
            branch: ResearchBranch::Industry,
            categories: Vec::new().into_boxed_slice(),
            start_year: 1936,
            base_days: 100,
            prerequisites: Vec::new().into_boxed_slice(),
            exclusive_with: Vec::new().into_boxed_slice(),
            modifiers: TechnologyModifiers::default(),
            equipment_unlocks: Vec::new().into_boxed_slice(),
        };
        let artillery_bonus = TechnologyBonus {
            name: "FRA_artillery_focus".into(),
            categories: vec!["artillery".into()].into_boxed_slice(),
            bonus_bp: 10_000,
            uses: 1,
        };
        let industry_bonus = TechnologyBonus {
            name: "FRA_laissez_faire".into(),
            categories: vec!["industry".into()].into_boxed_slice(),
            bonus_bp: 15_000,
            uses: 3,
        };

        assert!(artillery_bonus.matches(&artillery));
        assert!(!artillery_bonus.matches(&industry));
        assert!(industry_bonus.matches(&industry));
        assert!(!industry_bonus.matches(&artillery));
    }
}
