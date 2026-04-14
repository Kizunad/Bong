//! 修仙系统 — plan-cultivation-v1 完整切片（server 侧 P1–P5）。
//!
//! 子模块：
//!   * components       — 状态定义（Cultivation / MeridianSystem / QiColor / Karma / Contamination）
//!   * topology         — 20 经邻接表 Resource
//!   * tick             — QiRegenTick + ZoneQiDrainTick（零和合并实现）
//!   * meridian_open    — MeridianOpenTick（含 MeridianTarget Component）
//!   * breakthrough     — 5 阶升境事务
//!   * tribulation      — 化虚渡劫状态机（Spirit→Void）
//!   * forging          — rate / capacity 独立锻造
//!   * composure        — 心境缓慢回升
//!   * qi_zero_decay    — 爆脉降境 + LIFO 经脉封闭
//!   * color            — QiColorEvolutionTick
//!   * contamination    — 异种真元排异（10:15）
//!   * overload         — 超量流量 → 裂痕
//!   * heal             — 裂痕愈合
//!   * negative_zone    — 负灵域反吸
//!   * death_hooks      — 死亡触发 & 重生惩罚 & 终结清理
//!   * life_record      — 修炼生平卷
//!   * karma            — 业力极慢衰减
//!   * insight / insight_fallback / insight_apply — 顿悟系统
//!
//! 跨仓库 TODO：
//!   * 客户端 inspect UI + 目标选择对话框（plan §7）
//!   * agent LLM runtime（InsightRequest → InsightOffer 桥）
//!   * 战斗 plan：消费 CultivationDeathTrigger / TribulationFailed / throughput 写入

pub mod breakthrough;
pub mod color;
pub mod components;
pub mod composure;
pub mod contamination;
pub mod death_hooks;
pub mod forging;
pub mod heal;
pub mod insight;
pub mod insight_apply;
pub mod insight_fallback;
pub mod insight_flow;
pub mod karma;
pub mod life_record;
pub mod meridian_open;
pub mod negative_zone;
pub mod overload;
pub mod qi_zero_decay;
pub mod tick;
pub mod topology;
pub mod tribulation;

use valence::prelude::{
    Added, App, Client, Commands, Entity, IntoSystemConfigs, Query, Update, Username, Without,
};

use self::breakthrough::{breakthrough_system, BreakthroughOutcome, BreakthroughRequest};
use self::color::{qi_color_evolution_tick, PracticeLog};
use self::components::{Contamination, Cultivation, Karma, MeridianSystem, QiColor};
use self::composure::composure_tick;
use self::contamination::contamination_tick;
use self::death_hooks::{
    on_player_revived, on_player_terminated, CultivationDeathTrigger, PlayerRevived,
    PlayerTerminated,
};
use self::forging::{forging_system, ForgeOutcome, ForgeRequest};
use self::heal::meridian_heal_tick;
use self::insight::{
    InsightChosen, InsightOffer, InsightQuota, InsightRequest, InsightTriggerRegistry,
};
use self::insight_apply::{InsightModifiers, UnlockedPerceptions};
use self::insight_flow::{
    apply_insight_chosen, insight_trigger_on_breakthrough, insight_trigger_on_forge,
    process_insight_request,
};
use self::karma::karma_decay_tick;
use self::life_record::LifeRecord;
use self::meridian_open::meridian_open_tick;
use self::negative_zone::negative_zone_siphon_tick;
use self::overload::overload_detection_tick;
use self::qi_zero_decay::{qi_zero_decay_tick, RealmRegressed};
use self::tick::{qi_regen_and_zone_drain_tick, CultivationClock};
use self::topology::MeridianTopology;
use self::tribulation::{
    start_tribulation_system, tribulation_failure_system, tribulation_wave_system,
    InitiateXuhuaTribulation, TribulationAnnounce, TribulationFailed, TribulationWaveCleared,
};
use crate::player::state::canonical_player_id;

pub fn register(app: &mut App) {
    tracing::info!("[bong][cultivation] registering cultivation systems (plan P1–P5)");
    app.insert_resource(MeridianTopology::standard());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(InsightTriggerRegistry::with_defaults());

    // 事件（plan §3/§4/§5 全家桶）
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<ForgeRequest>();
    app.add_event::<ForgeOutcome>();
    app.add_event::<RealmRegressed>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<InsightRequest>();
    app.add_event::<InsightOffer>();
    app.add_event::<InsightChosen>();

    // Bevy IntoSystemConfigs 最多 20 个元素；拆两组。
    app.add_systems(
        Update,
        (
            attach_cultivation_to_joined_clients,
            // 核心 tick：回气/扣 zone → 打通 → 事务
            qi_regen_and_zone_drain_tick,
            meridian_open_tick.after(qi_regen_and_zone_drain_tick),
            breakthrough_system.after(meridian_open_tick),
            forging_system.after(breakthrough_system),
            // 稳态演化
            qi_color_evolution_tick,
            composure_tick,
            qi_zero_decay_tick.after(qi_regen_and_zone_drain_tick),
            // plan §2.1 损伤/净化链
            overload_detection_tick.after(meridian_open_tick),
            contamination_tick.after(qi_regen_and_zone_drain_tick),
            meridian_heal_tick.after(overload_detection_tick),
            negative_zone_siphon_tick.after(qi_regen_and_zone_drain_tick),
            // plan §3.2 渡劫
            start_tribulation_system,
            tribulation_wave_system,
            tribulation_failure_system,
            // plan §4 死亡/重生钩子
            on_player_revived,
            on_player_terminated,
            // plan §11-5 业力
            karma_decay_tick,
        ),
    );
    app.add_systems(
        Update,
        (
            // plan §5.4 / §5.5 顿悟流水线
            insight_trigger_on_breakthrough.after(breakthrough_system),
            insight_trigger_on_forge.after(forging_system),
            process_insight_request
                .after(insight_trigger_on_breakthrough)
                .after(insight_trigger_on_forge),
            apply_insight_chosen.after(process_insight_request),
        ),
    );
}

type CultivationAttachFilter = (Added<Client>, Without<Cultivation>);

fn attach_cultivation_to_joined_clients(
    mut commands: Commands,
    joined_clients: Query<(Entity, &Username), CultivationAttachFilter>,
) {
    for (entity, username) in &joined_clients {
        commands.entity(entity).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            QiColor::default(),
            Karma::default(),
            PracticeLog::default(),
            Contamination::default(),
            LifeRecord::new(canonical_player_id(username.0.as_str())),
            InsightQuota::default(),
            UnlockedPerceptions::default(),
            InsightModifiers::new(),
        ));
        tracing::info!("[bong][cultivation] attached full cultivation bundle to {entity:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::player::state::canonical_player_id;
    use valence::prelude::App;
    use valence::testing::create_mock_client;

    #[test]
    fn joined_clients_receive_canonical_player_character_id() {
        let mut app = App::new();
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Alice");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("joined client should receive a LifeRecord");

        assert_eq!(life_record.character_id, canonical_player_id("Alice"));
    }
}
