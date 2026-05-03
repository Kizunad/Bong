//! plan-terrain-jiuzong-ruin-v1 §7 P3 — 七宗守墓人与残卷分发表。

use valence::prelude::{bevy_ecs, App, Component};

use crate::worldgen::zong_formation::ZongmenOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleId {
    Baomai,
    Zhenfa,
    Dugu,
    Anqi,
    Zhenmai,
    MultiStyle,
    Tuike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeFragment {
    Style(StyleId),
}

#[derive(Debug, Clone, PartialEq, Component)]
pub struct ZongKeeper {
    pub origin: ZongmenOrigin,
    pub keeper_id: String,
    pub home_zone: String,
    pub flow_style: StyleId,
    pub patrol_radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ZongKeeperAggressionTrigger {
    FormationActivated,
    CoreContainerLooted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZongCanjuanLootEntry {
    pub origin: ZongmenOrigin,
    pub fragment: RecipeFragment,
    pub low_tier_drop_rate: f64,
    pub high_tier_drop_rate_active_affinity: f64,
    pub loot_table_path: &'static str,
}

pub fn register(_app: &mut App) {
    tracing::info!("[bong][npc][zong_keeper] registered seven zong keeper contract profiles");
}

pub fn zong_keeper_profile(origin: ZongmenOrigin) -> ZongKeeper {
    ZongKeeper {
        origin,
        keeper_id: format!("zong_keeper:{}", origin.zone_id()),
        home_zone: origin.zone_id().to_string(),
        flow_style: style_for_origin(origin),
        patrol_radius: 96.0,
    }
}

pub fn zong_keeper_profiles() -> Vec<ZongKeeper> {
    ZongmenOrigin::ALL
        .into_iter()
        .map(zong_keeper_profile)
        .collect()
}

pub fn style_for_origin(origin: ZongmenOrigin) -> StyleId {
    match origin {
        ZongmenOrigin::Bloodstream => StyleId::Baomai,
        ZongmenOrigin::Beiling => StyleId::Zhenfa,
        ZongmenOrigin::Nanyuan => StyleId::Dugu,
        ZongmenOrigin::Chixia => StyleId::Anqi,
        ZongmenOrigin::Xuanshui => StyleId::Zhenmai,
        ZongmenOrigin::Taichu => StyleId::MultiStyle,
        ZongmenOrigin::Youan => StyleId::Tuike,
    }
}

pub fn should_zong_keeper_turn_hostile(
    keeper: &ZongKeeper,
    zone_id: &str,
    origin: ZongmenOrigin,
    trigger: ZongKeeperAggressionTrigger,
) -> bool {
    matches!(
        trigger,
        ZongKeeperAggressionTrigger::FormationActivated
            | ZongKeeperAggressionTrigger::CoreContainerLooted
    ) && keeper.origin == origin
        && keeper.home_zone == zone_id
}

pub fn canjuan_loot_entry(origin: ZongmenOrigin) -> ZongCanjuanLootEntry {
    ZongCanjuanLootEntry {
        origin,
        fragment: RecipeFragment::Style(style_for_origin(origin)),
        low_tier_drop_rate: 0.02,
        high_tier_drop_rate_active_affinity: 0.003,
        loot_table_path: loot_table_path(origin),
    }
}

fn loot_table_path(origin: ZongmenOrigin) -> &'static str {
    match origin {
        ZongmenOrigin::Bloodstream => "server/assets/loot/zong_canjuan_bloodstream.json",
        ZongmenOrigin::Beiling => "server/assets/loot/zong_canjuan_beiling.json",
        ZongmenOrigin::Nanyuan => "server/assets/loot/zong_canjuan_nanyuan.json",
        ZongmenOrigin::Chixia => "server/assets/loot/zong_canjuan_chixia.json",
        ZongmenOrigin::Xuanshui => "server/assets/loot/zong_canjuan_xuanshui.json",
        ZongmenOrigin::Taichu => "server/assets/loot/zong_canjuan_taichu.json",
        ZongmenOrigin::Youan => "server/assets/loot/zong_canjuan_youan.json",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeper_profiles_cover_all_seven_origins() {
        let profiles = zong_keeper_profiles();

        assert_eq!(profiles.len(), 7);
        assert_eq!(profiles[0].flow_style, StyleId::Baomai);
        assert_eq!(profiles[6].flow_style, StyleId::Tuike);
        assert!(profiles
            .iter()
            .all(|profile| profile.keeper_id.starts_with("zong_keeper:jiuzong_")));
    }

    #[test]
    fn keeper_only_turns_hostile_for_own_origin_and_zone() {
        let keeper = zong_keeper_profile(ZongmenOrigin::Bloodstream);

        assert!(should_zong_keeper_turn_hostile(
            &keeper,
            "jiuzong_bloodstream_ruin",
            ZongmenOrigin::Bloodstream,
            ZongKeeperAggressionTrigger::FormationActivated,
        ));
        assert!(!should_zong_keeper_turn_hostile(
            &keeper,
            "jiuzong_beiling_ruin",
            ZongmenOrigin::Beiling,
            ZongKeeperAggressionTrigger::FormationActivated,
        ));
    }

    #[test]
    fn canjuan_loot_entry_binds_origin_to_style_fragment() {
        let entry = canjuan_loot_entry(ZongmenOrigin::Youan);

        assert_eq!(entry.fragment, RecipeFragment::Style(StyleId::Tuike));
        assert_eq!(entry.low_tier_drop_rate, 0.02);
        assert_eq!(entry.high_tier_drop_rate_active_affinity, 0.003);
        assert!(entry.loot_table_path.ends_with("zong_canjuan_youan.json"));
    }
}
