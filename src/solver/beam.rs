use crate::domain::GameDate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannerWeights {
    pub civilian_growth: u16,
    pub military_factories: u16,
    pub military_output: u16,
}

impl PlannerWeights {
    pub fn new(civilian_growth: u16, military_factories: u16, military_output: u16) -> Self {
        assert!(civilian_growth + military_factories + military_output > 0);

        Self {
            civilian_growth,
            military_factories,
            military_output,
        }
    }
}

impl Default for PlannerWeights {
    fn default() -> Self {
        Self {
            civilian_growth: 8,
            military_factories: 5,
            military_output: 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamSearchConfig {
    pub beam_width: usize,
    pub replan_days: u16,
}

impl BeamSearchConfig {
    pub fn new(beam_width: usize, replan_days: u16) -> Self {
        assert!(beam_width > 0);
        assert!(replan_days > 0);

        Self {
            beam_width,
            replan_days,
        }
    }
}

impl Default for BeamSearchConfig {
    fn default() -> Self {
        Self {
            beam_width: 64,
            replan_days: 35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RollingWindow {
    pub start: GameDate,
    pub end: GameDate,
}

impl RollingWindow {
    pub fn from_start(start: GameDate, replan_days: u16) -> Self {
        assert!(replan_days > 0);

        Self {
            start,
            end: start.add_days(replan_days),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchNode {
    pub window: RollingWindow,
    pub score: i64,
    pub applied_actions: usize,
}

impl SearchNode {
    pub fn new(window: RollingWindow) -> Self {
        Self {
            window,
            score: 0,
            applied_actions: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::GameDate;

    use super::{BeamSearchConfig, PlannerWeights, RollingWindow, SearchNode};

    #[test]
    fn beam_search_defaults_match_the_approved_planning_cadence() {
        let config = BeamSearchConfig::default();

        assert_eq!(config.beam_width, 64);
        assert_eq!(config.replan_days, 35);
    }

    #[test]
    fn planner_weights_require_positive_total_signal() {
        let result = std::panic::catch_unwind(|| PlannerWeights::new(0, 0, 0));

        assert!(result.is_err());
    }

    #[test]
    fn rolling_window_expands_from_the_start_date() {
        let window = RollingWindow::from_start(GameDate::new(1936, 1, 1), 35);

        assert_eq!(window.start, GameDate::new(1936, 1, 1));
        assert_eq!(window.end, GameDate::new(1936, 2, 5));
    }

    #[test]
    fn search_node_starts_empty_and_scoreless() {
        let window = RollingWindow::from_start(GameDate::new(1936, 1, 1), 35);
        let node = SearchNode::new(window);

        assert_eq!(node.score, 0);
        assert_eq!(node.applied_actions, 0);
    }
}
