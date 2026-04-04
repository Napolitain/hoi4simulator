use super::{
    actions::{AdvisorKind, ConstructionKind, FocusBranch, LawTarget},
    state::StrategicPhase,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructionDecisionContext {
    pub phase: StrategicPhase,
    pub military_factory_target_met: bool,
    pub minimum_force_target_met: bool,
    pub frontier_forts_met: bool,
    pub civilian_exception: bool,
    pub infrastructure_is_justified: bool,
}

impl ConstructionDecisionContext {
    pub fn pre_pivot(infrastructure_is_justified: bool) -> Self {
        Self {
            phase: StrategicPhase::PrePivot,
            military_factory_target_met: false,
            minimum_force_target_met: false,
            frontier_forts_met: false,
            civilian_exception: false,
            infrastructure_is_justified,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductionDecisionContext {
    pub changed_line_assignment: bool,
    pub demand_justified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleViolation {
    ConstructionKindNotAllowed(ConstructionKind),
    InfrastructureNeedsJustification,
    MustPrioritizeMilitaryFactories,
    MustPrioritizeFrontierForts,
    FocusBranchNotAllowed(FocusBranch),
    LawTargetNotAllowed(LawTarget),
    AdvisorNotAllowed(AdvisorKind),
    UnjustifiedProductionRetune,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FranceHeuristicRules;

impl FranceHeuristicRules {
    pub fn validate_construction(
        context: ConstructionDecisionContext,
        kind: ConstructionKind,
    ) -> Result<(), RuleViolation> {
        match context.phase {
            StrategicPhase::PrePivot => Self::validate_pre_pivot_construction(context, kind),
            StrategicPhase::PostPivot => Self::validate_post_pivot_construction(context, kind),
        }
    }

    pub fn validate_focus_branch(
        phase: StrategicPhase,
        branch: FocusBranch,
    ) -> Result<(), RuleViolation> {
        match phase {
            StrategicPhase::PrePivot => match branch {
                FocusBranch::Economy | FocusBranch::Industry => Ok(()),
                _ => Err(RuleViolation::FocusBranchNotAllowed(branch)),
            },
            StrategicPhase::PostPivot => match branch {
                FocusBranch::MilitaryIndustry => Ok(()),
                _ => Err(RuleViolation::FocusBranchNotAllowed(branch)),
            },
        }
    }

    pub fn validate_law_target(
        phase: StrategicPhase,
        target: LawTarget,
    ) -> Result<(), RuleViolation> {
        match phase {
            StrategicPhase::PrePivot => match target {
                LawTarget::Economy(_) | LawTarget::Trade(_) => Ok(()),
                LawTarget::Mobilization(_) => Err(RuleViolation::LawTargetNotAllowed(target)),
            },
            StrategicPhase::PostPivot => match target {
                LawTarget::Mobilization(_) => Ok(()),
                LawTarget::Economy(_) | LawTarget::Trade(_) => {
                    Err(RuleViolation::LawTargetNotAllowed(target))
                }
            },
        }
    }

    pub fn validate_advisor(
        phase: StrategicPhase,
        advisor: AdvisorKind,
    ) -> Result<(), RuleViolation> {
        match phase {
            StrategicPhase::PrePivot => match advisor {
                AdvisorKind::IndustryConcern | AdvisorKind::ResearchInstitute => Ok(()),
                AdvisorKind::MilitaryIndustrialist => {
                    Err(RuleViolation::AdvisorNotAllowed(advisor))
                }
            },
            StrategicPhase::PostPivot => match advisor {
                AdvisorKind::MilitaryIndustrialist => Ok(()),
                AdvisorKind::IndustryConcern | AdvisorKind::ResearchInstitute => {
                    Err(RuleViolation::AdvisorNotAllowed(advisor))
                }
            },
        }
    }

    pub fn validate_production_retune(
        context: ProductionDecisionContext,
    ) -> Result<(), RuleViolation> {
        if !context.changed_line_assignment || context.demand_justified {
            return Ok(());
        }

        Err(RuleViolation::UnjustifiedProductionRetune)
    }

    fn validate_pre_pivot_construction(
        context: ConstructionDecisionContext,
        kind: ConstructionKind,
    ) -> Result<(), RuleViolation> {
        match kind {
            ConstructionKind::CivilianFactory => Ok(()),
            ConstructionKind::Infrastructure if context.infrastructure_is_justified => Ok(()),
            ConstructionKind::Infrastructure => {
                Err(RuleViolation::InfrastructureNeedsJustification)
            }
            _ => Err(RuleViolation::ConstructionKindNotAllowed(kind)),
        }
    }

    fn validate_post_pivot_construction(
        context: ConstructionDecisionContext,
        kind: ConstructionKind,
    ) -> Result<(), RuleViolation> {
        if context.minimum_force_target_met && !context.frontier_forts_met {
            if kind == ConstructionKind::LandFort {
                return Ok(());
            }

            if context.civilian_exception && kind == ConstructionKind::CivilianFactory {
                return Ok(());
            }

            return Err(RuleViolation::MustPrioritizeFrontierForts);
        }

        if !context.military_factory_target_met {
            if kind == ConstructionKind::MilitaryFactory {
                return Ok(());
            }

            if context.civilian_exception && kind == ConstructionKind::CivilianFactory {
                return Ok(());
            }

            return Err(RuleViolation::MustPrioritizeMilitaryFactories);
        }

        if !context.frontier_forts_met {
            if kind == ConstructionKind::LandFort {
                return Ok(());
            }

            if context.civilian_exception && kind == ConstructionKind::CivilianFactory {
                return Ok(());
            }

            return Err(RuleViolation::MustPrioritizeFrontierForts);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{EconomyLaw, MobilizationLaw};
    use crate::sim::{
        actions::{AdvisorKind, ConstructionKind, FocusBranch, LawTarget},
        state::StrategicPhase,
    };

    use super::{
        ConstructionDecisionContext, FranceHeuristicRules, ProductionDecisionContext, RuleViolation,
    };

    #[test]
    fn pre_pivot_construction_allows_civilian_factories() {
        let context = ConstructionDecisionContext::pre_pivot(false);
        let result =
            FranceHeuristicRules::validate_construction(context, ConstructionKind::CivilianFactory);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn pre_pivot_construction_requires_justified_infrastructure() {
        let context = ConstructionDecisionContext::pre_pivot(false);
        let result =
            FranceHeuristicRules::validate_construction(context, ConstructionKind::Infrastructure);

        assert_eq!(result, Err(RuleViolation::InfrastructureNeedsJustification));
    }

    #[test]
    fn pre_pivot_construction_disallows_military_factories() {
        let context = ConstructionDecisionContext::pre_pivot(true);
        let result =
            FranceHeuristicRules::validate_construction(context, ConstructionKind::MilitaryFactory);

        assert_eq!(
            result,
            Err(RuleViolation::ConstructionKindNotAllowed(
                ConstructionKind::MilitaryFactory,
            ))
        );
    }

    #[test]
    fn post_pivot_requires_military_factories_before_forts() {
        let context = ConstructionDecisionContext {
            phase: StrategicPhase::PostPivot,
            military_factory_target_met: false,
            minimum_force_target_met: false,
            frontier_forts_met: false,
            civilian_exception: false,
            infrastructure_is_justified: false,
        };
        let result =
            FranceHeuristicRules::validate_construction(context, ConstructionKind::LandFort);

        assert_eq!(result, Err(RuleViolation::MustPrioritizeMilitaryFactories));
    }

    #[test]
    fn post_pivot_requires_forts_after_military_target_is_met() {
        let context = ConstructionDecisionContext {
            phase: StrategicPhase::PostPivot,
            military_factory_target_met: true,
            minimum_force_target_met: true,
            frontier_forts_met: false,
            civilian_exception: false,
            infrastructure_is_justified: false,
        };
        let result =
            FranceHeuristicRules::validate_construction(context, ConstructionKind::MilitaryFactory);

        assert_eq!(result, Err(RuleViolation::MustPrioritizeFrontierForts));
    }

    #[test]
    fn post_pivot_allows_forts_before_extra_military_once_minimum_force_is_met() {
        let context = ConstructionDecisionContext {
            phase: StrategicPhase::PostPivot,
            military_factory_target_met: false,
            minimum_force_target_met: true,
            frontier_forts_met: false,
            civilian_exception: false,
            infrastructure_is_justified: false,
        };
        let result =
            FranceHeuristicRules::validate_construction(context, ConstructionKind::LandFort);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn pre_pivot_focus_whitelist_keeps_only_economic_branches() {
        let allowed = FranceHeuristicRules::validate_focus_branch(
            StrategicPhase::PrePivot,
            FocusBranch::Economy,
        );
        let blocked = FranceHeuristicRules::validate_focus_branch(
            StrategicPhase::PrePivot,
            FocusBranch::MilitaryIndustry,
        );

        assert_eq!(allowed, Ok(()));
        assert_eq!(
            blocked,
            Err(RuleViolation::FocusBranchNotAllowed(
                FocusBranch::MilitaryIndustry,
            ))
        );
    }

    #[test]
    fn law_whitelist_shifts_from_economy_to_mobilization() {
        let pre_allowed = FranceHeuristicRules::validate_law_target(
            StrategicPhase::PrePivot,
            LawTarget::Economy(EconomyLaw::EarlyMobilization),
        );
        let pre_blocked = FranceHeuristicRules::validate_law_target(
            StrategicPhase::PrePivot,
            LawTarget::Mobilization(MobilizationLaw::LimitedConscription),
        );
        let post_allowed = FranceHeuristicRules::validate_law_target(
            StrategicPhase::PostPivot,
            LawTarget::Mobilization(MobilizationLaw::ExtensiveConscription),
        );

        assert_eq!(pre_allowed, Ok(()));
        assert!(pre_blocked.is_err());
        assert_eq!(post_allowed, Ok(()));
    }

    #[test]
    fn advisor_whitelist_shifts_to_military_after_pivot() {
        let pre_allowed = FranceHeuristicRules::validate_advisor(
            StrategicPhase::PrePivot,
            AdvisorKind::IndustryConcern,
        );
        let post_blocked = FranceHeuristicRules::validate_advisor(
            StrategicPhase::PostPivot,
            AdvisorKind::ResearchInstitute,
        );

        assert_eq!(pre_allowed, Ok(()));
        assert_eq!(
            post_blocked,
            Err(RuleViolation::AdvisorNotAllowed(
                AdvisorKind::ResearchInstitute,
            ))
        );
    }

    #[test]
    fn production_line_reassignment_requires_demand_justification() {
        let unjustified =
            FranceHeuristicRules::validate_production_retune(ProductionDecisionContext {
                changed_line_assignment: true,
                demand_justified: false,
            });
        let justified =
            FranceHeuristicRules::validate_production_retune(ProductionDecisionContext {
                changed_line_assignment: true,
                demand_justified: true,
            });

        assert_eq!(unjustified, Err(RuleViolation::UnjustifiedProductionRetune));
        assert_eq!(justified, Ok(()));
    }
}
