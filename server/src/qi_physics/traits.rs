use crate::cultivation::components::ColorKind;

use super::env::{ContainerKind, MediumKind};

pub trait StyleAttack {
    fn style_color(&self) -> ColorKind;
    fn injected_qi(&self) -> f64;

    fn purity(&self) -> f64 {
        1.0
    }

    fn rejection_rate(&self) -> f64 {
        0.30
    }

    fn medium(&self) -> MediumKind {
        MediumKind::bare(self.style_color())
    }
}

pub trait StyleDefense {
    fn defense_color(&self) -> ColorKind;
    fn resistance(&self) -> f64;

    fn drain_affinity(&self) -> f64 {
        0.0
    }
}

pub trait Container {
    fn container_kind(&self) -> ContainerKind;
    fn sealed_qi(&self) -> f64;
    fn capacity(&self) -> f64;
}

#[derive(Debug, Clone, Copy)]
pub struct SimpleStyleAttack {
    pub color: ColorKind,
    pub qi: f64,
    pub purity: f64,
    pub rejection_rate: f64,
    pub medium: MediumKind,
}

impl SimpleStyleAttack {
    pub fn new(color: ColorKind, qi: f64) -> Self {
        Self {
            color,
            qi,
            purity: 1.0,
            rejection_rate: 0.30,
            medium: MediumKind::bare(color),
        }
    }
}

impl StyleAttack for SimpleStyleAttack {
    fn style_color(&self) -> ColorKind {
        self.color
    }

    fn injected_qi(&self) -> f64 {
        self.qi
    }

    fn purity(&self) -> f64 {
        self.purity
    }

    fn rejection_rate(&self) -> f64 {
        self.rejection_rate
    }

    fn medium(&self) -> MediumKind {
        self.medium
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SimpleStyleDefense {
    pub color: ColorKind,
    pub resistance: f64,
    pub drain_affinity: f64,
}

impl SimpleStyleDefense {
    pub fn new(color: ColorKind, resistance: f64) -> Self {
        Self {
            color,
            resistance,
            drain_affinity: 0.0,
        }
    }
}

impl StyleDefense for SimpleStyleDefense {
    fn defense_color(&self) -> ColorKind {
        self.color
    }

    fn resistance(&self) -> f64 {
        self.resistance
    }

    fn drain_affinity(&self) -> f64 {
        self.drain_affinity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_attack_defaults_to_full_purity() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        assert_eq!(atk.purity(), 1.0);
        assert_eq!(atk.rejection_rate(), 0.30);
        assert_eq!(atk.injected_qi(), 10.0);
    }

    #[test]
    fn simple_defense_defaults_to_no_drain() {
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.3);
        assert_eq!(def.drain_affinity(), 0.0);
        assert_eq!(def.resistance(), 0.3);
    }
}
