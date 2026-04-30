#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiritualSenseRing {
    Inner,
    Middle,
    Outer,
}

pub fn throttle_interval_ticks(ring: SpiritualSenseRing) -> u64 {
    match ring {
        SpiritualSenseRing::Inner => 5,
        SpiritualSenseRing::Middle => 20,
        SpiritualSenseRing::Outer => 50,
    }
}

pub fn should_scan(now_tick: u64, last_scan_tick: Option<u64>, ring: SpiritualSenseRing) -> bool {
    let Some(last) = last_scan_tick else {
        return true;
    };
    now_tick.saturating_sub(last) >= throttle_interval_ticks(ring)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_intervals() {
        assert_eq!(throttle_interval_ticks(SpiritualSenseRing::Inner), 5);
        assert_eq!(throttle_interval_ticks(SpiritualSenseRing::Middle), 20);
        assert_eq!(throttle_interval_ticks(SpiritualSenseRing::Outer), 50);
        assert!(should_scan(5, Some(0), SpiritualSenseRing::Inner));
        assert!(!should_scan(4, Some(0), SpiritualSenseRing::Inner));
    }
}
