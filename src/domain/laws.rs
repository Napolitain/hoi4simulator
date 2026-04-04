#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EconomyLaw {
    CivilianEconomy,
    EarlyMobilization,
    PartialMobilization,
    WarEconomy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeLaw {
    ExportFocus,
    LimitedExports,
    ClosedEconomy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobilizationLaw {
    VolunteerOnly,
    LimitedConscription,
    ExtensiveConscription,
}

impl MobilizationLaw {
    pub fn manpower_permyriad(self) -> u16 {
        match self {
            Self::VolunteerOnly => 150,
            Self::LimitedConscription => 250,
            Self::ExtensiveConscription => 500,
        }
    }

    pub fn available_manpower(self, population: u64) -> u64 {
        assert!(population > 0);

        population * u64::from(self.manpower_permyriad()) / 10_000
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountryLaws {
    pub economy: EconomyLaw,
    pub trade: TradeLaw,
    pub mobilization: MobilizationLaw,
}

impl Default for CountryLaws {
    fn default() -> Self {
        Self {
            economy: EconomyLaw::CivilianEconomy,
            trade: TradeLaw::ExportFocus,
            mobilization: MobilizationLaw::LimitedConscription,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CountryLaws, EconomyLaw, MobilizationLaw, TradeLaw};

    #[test]
    fn mobilization_law_scales_available_manpower() {
        let limited = MobilizationLaw::LimitedConscription.available_manpower(40_000_000);
        let extensive = MobilizationLaw::ExtensiveConscription.available_manpower(40_000_000);

        assert_eq!(limited, 1_000_000);
        assert_eq!(extensive, 2_000_000);
    }

    #[test]
    fn default_country_laws_match_the_opening_macro_game() {
        let laws = CountryLaws::default();

        assert_eq!(laws.economy, EconomyLaw::CivilianEconomy);
        assert_eq!(laws.trade, TradeLaw::ExportFocus);
        assert_eq!(laws.mobilization, MobilizationLaw::LimitedConscription);
    }
}
