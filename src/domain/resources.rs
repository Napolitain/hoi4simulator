use fory::ForyObject;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ForyObject)]
pub enum ResourceKind {
    Steel,
    Aluminium,
    Tungsten,
    Chromium,
    Oil,
    Rubber,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ForyObject)]
pub struct ResourceLedger {
    pub steel: u32,
    pub aluminium: u32,
    pub tungsten: u32,
    pub chromium: u32,
    pub oil: u32,
    pub rubber: u32,
}

impl ResourceLedger {
    pub fn get(self, kind: ResourceKind) -> u32 {
        match kind {
            ResourceKind::Steel => self.steel,
            ResourceKind::Aluminium => self.aluminium,
            ResourceKind::Tungsten => self.tungsten,
            ResourceKind::Chromium => self.chromium,
            ResourceKind::Oil => self.oil,
            ResourceKind::Rubber => self.rubber,
        }
    }

    pub fn add_kind(&mut self, kind: ResourceKind, amount: u32) {
        match kind {
            ResourceKind::Steel => self.steel = self.steel.saturating_add(amount),
            ResourceKind::Aluminium => self.aluminium = self.aluminium.saturating_add(amount),
            ResourceKind::Tungsten => self.tungsten = self.tungsten.saturating_add(amount),
            ResourceKind::Chromium => self.chromium = self.chromium.saturating_add(amount),
            ResourceKind::Oil => self.oil = self.oil.saturating_add(amount),
            ResourceKind::Rubber => self.rubber = self.rubber.saturating_add(amount),
        }
    }

    pub fn add_named(&mut self, name: &str, amount: u32) -> bool {
        let Some(kind) = ResourceKind::from_clausewitz_name(name) else {
            return false;
        };
        self.add_kind(kind, amount);
        true
    }

    pub fn plus(self, other: Self) -> Self {
        Self {
            steel: self.steel.saturating_add(other.steel),
            aluminium: self.aluminium.saturating_add(other.aluminium),
            tungsten: self.tungsten.saturating_add(other.tungsten),
            chromium: self.chromium.saturating_add(other.chromium),
            oil: self.oil.saturating_add(other.oil),
            rubber: self.rubber.saturating_add(other.rubber),
        }
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            steel: self.steel.saturating_sub(other.steel),
            aluminium: self.aluminium.saturating_sub(other.aluminium),
            tungsten: self.tungsten.saturating_sub(other.tungsten),
            chromium: self.chromium.saturating_sub(other.chromium),
            oil: self.oil.saturating_sub(other.oil),
            rubber: self.rubber.saturating_sub(other.rubber),
        }
    }

    pub fn cap_at(self, available: Self) -> Self {
        Self {
            steel: self.steel.min(available.steel),
            aluminium: self.aluminium.min(available.aluminium),
            tungsten: self.tungsten.min(available.tungsten),
            chromium: self.chromium.min(available.chromium),
            oil: self.oil.min(available.oil),
            rubber: self.rubber.min(available.rubber),
        }
    }

    pub fn scale(self, multiplier: u16) -> Self {
        let multiplier = u32::from(multiplier);

        Self {
            steel: self.steel.saturating_mul(multiplier),
            aluminium: self.aluminium.saturating_mul(multiplier),
            tungsten: self.tungsten.saturating_mul(multiplier),
            chromium: self.chromium.saturating_mul(multiplier),
            oil: self.oil.saturating_mul(multiplier),
            rubber: self.rubber.saturating_mul(multiplier),
        }
    }

    pub fn scale_bp(self, basis_points: u16) -> Self {
        let basis_points = u64::from(basis_points);
        let scale = |value: u32| {
            u32::try_from((u64::from(value) * basis_points) / 10_000).unwrap_or(u32::MAX)
        };

        Self {
            steel: scale(self.steel),
            aluminium: scale(self.aluminium),
            tungsten: scale(self.tungsten),
            chromium: scale(self.chromium),
            oil: scale(self.oil),
            rubber: scale(self.rubber),
        }
    }

    pub fn total(self) -> u32 {
        self.steel
            .saturating_add(self.aluminium)
            .saturating_add(self.tungsten)
            .saturating_add(self.chromium)
            .saturating_add(self.oil)
            .saturating_add(self.rubber)
    }

    pub fn any_positive(self) -> bool {
        self.total() > 0
    }

    pub fn utilization_bp(self, available: Self) -> u16 {
        let capped_total = u64::from(self.cap_at(available).total());
        let available_total = u64::from(available.total());
        if available_total == 0 {
            return 0;
        }

        u16::try_from((capped_total * 10_000 / available_total).min(10_000)).unwrap_or(10_000)
    }
}

impl ResourceKind {
    pub fn from_clausewitz_name(name: &str) -> Option<Self> {
        match name {
            "steel" => Some(Self::Steel),
            "aluminium" => Some(Self::Aluminium),
            "tungsten" => Some(Self::Tungsten),
            "chromium" => Some(Self::Chromium),
            "oil" => Some(Self::Oil),
            "rubber" => Some(Self::Rubber),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourceKind, ResourceLedger};

    #[test]
    fn resource_ledger_parses_clausewitz_resource_names() {
        let mut resources = ResourceLedger::default();

        assert!(resources.add_named("steel", 12));
        assert!(resources.add_named("tungsten", 3));
        assert!(!resources.add_named("uranium", 4));
        assert_eq!(resources.steel, 12);
        assert_eq!(resources.tungsten, 3);
    }

    #[test]
    fn resource_ledger_scales_and_caps_usage() {
        let demand = ResourceLedger {
            steel: 3,
            aluminium: 1,
            tungsten: 2,
            chromium: 0,
            oil: 0,
            rubber: 0,
        };
        let available = ResourceLedger {
            steel: 8,
            aluminium: 4,
            tungsten: 3,
            chromium: 0,
            oil: 0,
            rubber: 0,
        };

        assert_eq!(
            demand.scale(3),
            ResourceLedger {
                steel: 9,
                aluminium: 3,
                tungsten: 6,
                chromium: 0,
                oil: 0,
                rubber: 0,
            }
        );
        assert_eq!(demand.scale(3).cap_at(available).tungsten, 3);
        assert_eq!(demand.scale(3).utilization_bp(available), 9_333);
        assert_eq!(demand.scale_bp(15_000).steel, 4);
        assert_eq!(demand.get(ResourceKind::Steel), 3);
    }
}
