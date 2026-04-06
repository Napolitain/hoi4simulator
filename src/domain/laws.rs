use fory::ForyObject;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub enum EconomyLaw {
    CivilianEconomy,
    EarlyMobilization,
    PartialMobilization,
    WarEconomy,
    TotalMobilization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub enum TradeLaw {
    ExportFocus,
    LimitedExports,
    ClosedEconomy,
    FreeTrade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
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

impl TradeLaw {
    pub fn resources_to_market_bp(self) -> u16 {
        match self {
            Self::ExportFocus => 5_000,
            Self::LimitedExports => 2_500,
            Self::ClosedEconomy => 0,
            Self::FreeTrade => 8_000,
        }
    }

    pub fn local_resource_retention_bp(self) -> u16 {
        10_000 - self.resources_to_market_bp()
    }

    pub fn research_speed_bp(self) -> u16 {
        match self {
            Self::ExportFocus => 500,
            Self::LimitedExports => 100,
            Self::ClosedEconomy => 0,
            Self::FreeTrade => 1_000,
        }
    }

    pub fn construction_speed_bp(self) -> u16 {
        match self {
            Self::ExportFocus => 1_000,
            Self::LimitedExports => 500,
            Self::ClosedEconomy => 0,
            Self::FreeTrade => 1_500,
        }
    }

    pub fn factory_output_bp(self) -> u16 {
        match self {
            Self::ExportFocus => 1_000,
            Self::LimitedExports => 500,
            Self::ClosedEconomy => 0,
            Self::FreeTrade => 1_500,
        }
    }
}

impl EconomyLaw {
    pub fn consumer_goods_ratio_bp(self) -> u16 {
        match self {
            Self::CivilianEconomy => 3_500,
            Self::EarlyMobilization => 3_000,
            Self::PartialMobilization => 2_500,
            Self::WarEconomy => 2_000,
            Self::TotalMobilization => 1_500,
        }
    }

    pub fn civilian_factory_construction_bp(self) -> i16 {
        match self {
            Self::CivilianEconomy => -3_000,
            Self::EarlyMobilization => -1_000,
            Self::PartialMobilization | Self::WarEconomy | Self::TotalMobilization => 0,
        }
    }

    pub fn military_factory_construction_bp(self) -> i16 {
        match self {
            Self::CivilianEconomy => -3_000,
            Self::EarlyMobilization => -1_000,
            Self::PartialMobilization => 1_000,
            Self::WarEconomy => 2_000,
            Self::TotalMobilization => 3_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
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
    use proptest::prelude::*;

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

    #[test]
    fn trade_law_matches_market_and_bonus_values() {
        assert_eq!(TradeLaw::FreeTrade.resources_to_market_bp(), 8_000);
        assert_eq!(TradeLaw::ExportFocus.resources_to_market_bp(), 5_000);
        assert_eq!(TradeLaw::LimitedExports.resources_to_market_bp(), 2_500);
        assert_eq!(TradeLaw::ClosedEconomy.resources_to_market_bp(), 0);

        assert_eq!(TradeLaw::FreeTrade.local_resource_retention_bp(), 2_000);
        assert_eq!(TradeLaw::ExportFocus.local_resource_retention_bp(), 5_000);
        assert_eq!(
            TradeLaw::LimitedExports.local_resource_retention_bp(),
            7_500
        );
        assert_eq!(
            TradeLaw::ClosedEconomy.local_resource_retention_bp(),
            10_000
        );

        assert_eq!(TradeLaw::FreeTrade.research_speed_bp(), 1_000);
        assert_eq!(TradeLaw::ExportFocus.research_speed_bp(), 500);
        assert_eq!(TradeLaw::LimitedExports.research_speed_bp(), 100);
        assert_eq!(TradeLaw::ClosedEconomy.research_speed_bp(), 0);

        assert_eq!(TradeLaw::FreeTrade.construction_speed_bp(), 1_500);
        assert_eq!(TradeLaw::ExportFocus.construction_speed_bp(), 1_000);
        assert_eq!(TradeLaw::LimitedExports.construction_speed_bp(), 500);
        assert_eq!(TradeLaw::ClosedEconomy.construction_speed_bp(), 0);

        assert_eq!(TradeLaw::FreeTrade.factory_output_bp(), 1_500);
        assert_eq!(TradeLaw::ExportFocus.factory_output_bp(), 1_000);
        assert_eq!(TradeLaw::LimitedExports.factory_output_bp(), 500);
        assert_eq!(TradeLaw::ClosedEconomy.factory_output_bp(), 0);
    }

    #[test]
    fn economy_law_matches_raw_consumer_goods_and_build_speed_values() {
        assert_eq!(EconomyLaw::CivilianEconomy.consumer_goods_ratio_bp(), 3_500);
        assert_eq!(
            EconomyLaw::EarlyMobilization.consumer_goods_ratio_bp(),
            3_000
        );
        assert_eq!(
            EconomyLaw::PartialMobilization.consumer_goods_ratio_bp(),
            2_500
        );
        assert_eq!(EconomyLaw::WarEconomy.consumer_goods_ratio_bp(), 2_000);
        assert_eq!(
            EconomyLaw::TotalMobilization.consumer_goods_ratio_bp(),
            1_500
        );

        assert_eq!(
            EconomyLaw::CivilianEconomy.civilian_factory_construction_bp(),
            -3_000
        );
        assert_eq!(
            EconomyLaw::EarlyMobilization.civilian_factory_construction_bp(),
            -1_000
        );
        assert_eq!(
            EconomyLaw::PartialMobilization.civilian_factory_construction_bp(),
            0
        );

        assert_eq!(
            EconomyLaw::CivilianEconomy.military_factory_construction_bp(),
            -3_000
        );
        assert_eq!(
            EconomyLaw::EarlyMobilization.military_factory_construction_bp(),
            -1_000
        );
        assert_eq!(
            EconomyLaw::PartialMobilization.military_factory_construction_bp(),
            1_000
        );
        assert_eq!(
            EconomyLaw::WarEconomy.military_factory_construction_bp(),
            2_000
        );
        assert_eq!(
            EconomyLaw::TotalMobilization.military_factory_construction_bp(),
            3_000
        );
    }

    fn law_rank(law: EconomyLaw) -> u8 {
        match law {
            EconomyLaw::CivilianEconomy => 0,
            EconomyLaw::EarlyMobilization => 1,
            EconomyLaw::PartialMobilization => 2,
            EconomyLaw::WarEconomy => 3,
            EconomyLaw::TotalMobilization => 4,
        }
    }

    fn law_from_rank(rank: u8) -> EconomyLaw {
        match rank {
            0 => EconomyLaw::CivilianEconomy,
            1 => EconomyLaw::EarlyMobilization,
            2 => EconomyLaw::PartialMobilization,
            3 => EconomyLaw::WarEconomy,
            _ => EconomyLaw::TotalMobilization,
        }
    }

    proptest! {
        #[test]
        fn stronger_economy_laws_reduce_consumer_goods_and_raise_military_build_speed(
            left_rank in 0_u8..5,
            right_rank in 0_u8..5,
        ) {
            let left = law_from_rank(left_rank);
            let right = law_from_rank(right_rank);
            prop_assume!(law_rank(left) <= law_rank(right));

            prop_assert!(right.consumer_goods_ratio_bp() <= left.consumer_goods_ratio_bp());
            prop_assert!(right.military_factory_construction_bp() >= left.military_factory_construction_bp());
        }
    }
}
