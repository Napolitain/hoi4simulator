use std::collections::BTreeSet;

use super::GameDate;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WarState {
    pub first: Box<str>,
    pub second: Box<str>,
}

impl WarState {
    pub fn new(first: impl Into<Box<str>>, second: impl Into<Box<str>>) -> Self {
        let first = first.into();
        let second = second.into();
        assert_ne!(first, second);

        if first <= second {
            Self { first, second }
        } else {
            Self {
                first: second,
                second: first,
            }
        }
    }

    pub fn matches(&self, left: &str, right: &str) -> bool {
        let (left, right) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.first.as_ref() == left && self.second.as_ref() == right
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WorldState {
    pub dissolved_countries: Vec<Box<str>>,
    pub active_wars: Vec<WarState>,
}

impl WorldState {
    pub fn country_exists(&self, tag: &str) -> bool {
        !self
            .dissolved_countries
            .iter()
            .any(|country| country.as_ref() == tag)
    }

    pub fn countries_at_war(&self, left: &str, right: &str) -> bool {
        self.active_wars.iter().any(|war| war.matches(left, right))
    }

    pub fn dissolve_country(&mut self, tag: impl Into<Box<str>>) {
        let tag = tag.into();
        if self
            .dissolved_countries
            .iter()
            .any(|country| country == &tag)
        {
            return;
        }
        self.dissolved_countries.push(tag);
    }

    pub fn start_war(&mut self, left: impl Into<Box<str>>, right: impl Into<Box<str>>) {
        let war = WarState::new(left, right);
        if self.active_wars.iter().any(|current| current == &war) {
            return;
        }
        self.active_wars.push(war);
    }

    pub fn apply_event(&mut self, event: &TimelineEvent) {
        match event {
            TimelineEvent::StartWar { left, right, .. } => {
                self.start_war(left.clone(), right.clone());
            }
            TimelineEvent::DissolveCountry { tag, .. } => {
                self.dissolve_country(tag.clone());
            }
        }
    }

    pub fn assert_invariants(&self) {
        let dissolved = self
            .dissolved_countries
            .iter()
            .map(Box::as_ref)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            dissolved.len(),
            self.dissolved_countries.len(),
            "duplicate dissolved country tag"
        );

        let wars = self.active_wars.iter().collect::<BTreeSet<_>>();
        assert_eq!(wars.len(), self.active_wars.len(), "duplicate war pair");
        assert!(
            self.active_wars
                .iter()
                .all(|war| war.first.as_ref() != war.second.as_ref()),
            "war pair cannot target the same country"
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimelineCondition {
    DateAtLeast(GameDate),
    DateBefore(GameDate),
    CountryExists(Box<str>),
    HasWarWith(Box<str>),
}

impl TimelineCondition {
    pub fn evaluate(
        &self,
        current_date: GameDate,
        world_state: &WorldState,
        root_tag: &str,
    ) -> bool {
        match self {
            Self::DateAtLeast(threshold) => current_date >= *threshold,
            Self::DateBefore(threshold) => current_date < *threshold,
            Self::CountryExists(tag) => world_state.country_exists(tag),
            Self::HasWarWith(tag) => world_state.countries_at_war(root_tag, tag),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimelineEvent {
    StartWar {
        date: GameDate,
        left: Box<str>,
        right: Box<str>,
    },
    DissolveCountry {
        date: GameDate,
        tag: Box<str>,
    },
}

impl TimelineEvent {
    pub fn date(&self) -> GameDate {
        match self {
            Self::StartWar { date, .. } | Self::DissolveCountry { date, .. } => *date,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TimelineCondition, TimelineEvent, WarState, WorldState};
    use crate::domain::GameDate;

    #[test]
    fn war_state_matches_symmetrically() {
        let war = WarState::new("GER", "POL");

        assert!(war.matches("GER", "POL"));
        assert!(war.matches("POL", "GER"));
    }

    #[test]
    fn world_state_tracks_dissolved_countries_and_wars() {
        let mut state = WorldState::default();
        state.dissolve_country("AUS");
        state.start_war("FRA", "GER");

        assert!(!state.country_exists("AUS"));
        assert!(state.country_exists("CZE"));
        assert!(state.countries_at_war("GER", "FRA"));
    }

    #[test]
    fn timeline_event_applies_to_world_state() {
        let mut state = WorldState::default();
        state.apply_event(&TimelineEvent::DissolveCountry {
            date: GameDate::new(1938, 3, 12),
            tag: "AUS".into(),
        });
        state.apply_event(&TimelineEvent::StartWar {
            date: GameDate::new(1939, 9, 3),
            left: "FRA".into(),
            right: "GER".into(),
        });

        assert!(!state.country_exists("AUS"));
        assert!(state.countries_at_war("FRA", "GER"));
    }

    #[test]
    fn timeline_condition_evaluates_against_date_and_world_state() {
        let mut state = WorldState::default();
        state.dissolve_country("AUS");
        state.start_war("FRA", "GER");

        assert!(
            TimelineCondition::DateAtLeast(GameDate::new(1939, 9, 1)).evaluate(
                GameDate::new(1939, 9, 1),
                &state,
                "FRA",
            )
        );
        assert!(
            TimelineCondition::DateBefore(GameDate::new(1940, 6, 22)).evaluate(
                GameDate::new(1940, 5, 10),
                &state,
                "FRA",
            )
        );
        assert!(!TimelineCondition::CountryExists("AUS".into()).evaluate(
            GameDate::new(1939, 1, 1),
            &state,
            "FRA",
        ));
        assert!(TimelineCondition::HasWarWith("GER".into()).evaluate(
            GameDate::new(1939, 9, 10),
            &state,
            "FRA",
        ));
    }
}
