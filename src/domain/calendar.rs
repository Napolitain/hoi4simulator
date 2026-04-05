use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl GameDate {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        assert!(month >= 1);
        assert!(month <= 12);

        let max_day = Self::days_in_month(year, month);
        assert!(day >= 1);
        assert!(day <= max_day);

        Self { year, month, day }
    }

    pub fn add_days(self, days: u16) -> Self {
        let mut date = self;
        for _ in 0..days {
            date = date.next_day();
        }
        date
    }

    pub fn days_until(self, other: Self) -> i32 {
        other.ordinal_days() - self.ordinal_days()
    }

    pub fn next_day(self) -> Self {
        let max_day = Self::days_in_month(self.year, self.month);

        if self.day < max_day {
            return Self::new(self.year, self.month, self.day + 1);
        }

        if self.month < 12 {
            return Self::new(self.year, self.month + 1, 1);
        }

        Self::new(self.year + 1, 1, 1)
    }

    pub fn previous_day(self) -> Self {
        assert!(self.year > 0 || self.month > 1 || self.day > 1);

        if self.day > 1 {
            return Self::new(self.year, self.month, self.day - 1);
        }

        if self.month > 1 {
            let month = self.month - 1;
            return Self::new(self.year, month, Self::days_in_month(self.year, month));
        }

        Self::new(self.year - 1, 12, 31)
    }

    pub fn is_leap_year(year: u16) -> bool {
        (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
    }

    pub fn days_in_month(year: u16, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if Self::is_leap_year(year) => 29,
            2 => 28,
            _ => panic!("invalid month"),
        }
    }

    fn ordinal_days(self) -> i32 {
        let mut days = 0_i32;

        for year in 0..self.year {
            if Self::is_leap_year(year) {
                days += 366;
            } else {
                days += 365;
            }
        }

        for month in 1..self.month {
            days += i32::from(Self::days_in_month(self.year, month));
        }

        days + i32::from(self.day) - 1
    }
}

impl fmt::Display for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PivotWindow {
    pub start: GameDate,
    pub end: GameDate,
}

impl PivotWindow {
    pub fn new(start: GameDate, end: GameDate) -> Self {
        assert!(start <= end);
        Self { start, end }
    }

    pub fn contains(self, date: GameDate) -> bool {
        date >= self.start && date <= self.end
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{GameDate, PivotWindow};

    #[test]
    fn game_date_rejects_invalid_months() {
        let result = std::panic::catch_unwind(|| GameDate::new(1936, 13, 1));
        assert!(result.is_err());
    }

    #[test]
    fn game_date_rejects_invalid_days() {
        let result = std::panic::catch_unwind(|| GameDate::new(1936, 2, 30));
        assert!(result.is_err());
    }

    #[test]
    fn game_date_advances_across_month_boundaries() {
        let start = GameDate::new(1936, 1, 31);
        let next = start.next_day();

        assert_eq!(next, GameDate::new(1936, 2, 1));
    }

    #[test]
    fn game_date_adds_days_with_leap_year_support() {
        let start = GameDate::new(1936, 2, 27);
        let end = start.add_days(3);

        assert_eq!(end, GameDate::new(1936, 3, 1));
    }

    #[test]
    fn game_date_moves_back_across_month_boundaries() {
        let date = GameDate::new(1936, 3, 1);

        assert_eq!(date.previous_day(), GameDate::new(1936, 2, 29));
    }

    #[test]
    fn pivot_window_contains_both_bounds() {
        let window = PivotWindow::new(GameDate::new(1938, 6, 1), GameDate::new(1939, 1, 1));

        assert!(window.contains(GameDate::new(1938, 6, 1)));
        assert!(window.contains(GameDate::new(1939, 1, 1)));
        assert!(!window.contains(GameDate::new(1938, 5, 31)));
    }

    proptest! {
        #[test]
        fn next_day_is_strictly_monotone(
            year in 1930u16..1945,
            month in 1u8..13,
            day_seed in 0u16..31,
        ) {
            let max_day = GameDate::days_in_month(year, month);
            let day = u8::try_from(day_seed % u16::from(max_day) + 1).unwrap_or(max_day);
            let start = GameDate::new(year, month, day);

            prop_assert!(start.next_day() > start);
        }

        #[test]
        fn add_days_round_trips_with_days_until(
            year in 1930u16..1945,
            month in 1u8..13,
            day_seed in 0u16..31,
            days in 0u16..366,
        ) {
            let max_day = GameDate::days_in_month(year, month);
            let day = u8::try_from(day_seed % u16::from(max_day) + 1).unwrap_or(max_day);
            let start = GameDate::new(year, month, day);
            let end = start.add_days(days);

            prop_assert_eq!(start.days_until(end), i32::from(days));
            prop_assert_eq!(end.days_until(start), -i32::from(days));
        }

        #[test]
        fn add_days_is_associative(
            year in 1930u16..1940,
            month in 1u8..13,
            day_seed in 0u16..31,
            a in 0u16..183,
            b in 0u16..183,
        ) {
            let max_day = GameDate::days_in_month(year, month);
            let day = u8::try_from(day_seed % u16::from(max_day) + 1).unwrap_or(max_day);
            let start = GameDate::new(year, month, day);

            let via_two_steps = start.add_days(a).add_days(b);
            let via_one_step = start.add_days(a + b);

            prop_assert_eq!(via_two_steps, via_one_step);
        }
    }
}
