//! plan-skill-anim-fidelity-v1 P5 —— 招式粒子双端接线矩阵的**唯一真相源**。
//!
//! P5 把 11 招从「借别家粒子」改成各自专属 `SpawnParticle` event_id。这类改动的
//! 经典失败模式是两端字符串各自漂移：server 改了 emit 的 id、client 忘了同步
//! `VfxBootstrap` 注册 → `BongVfxParticleBridge` 查表落空记 bridgeMiss，招式**静默
//! 无特效**（不报错、不崩溃，只是玩家什么都看不到）。
//!
//! 因此矩阵在这里以结构化常量表落库，并单向导出
//! `client/src/test/resources/bong/skill_vfx_wiring_manifest.json` 供双端消费：
//! - server（`skill_vfx_wiring_test.rs`）：逐招驱动真实 resolver，断言发出的
//!   `SpawnParticle` event_id / color 与本表一致，且**旧借用 id 不再出现**；
//! - client（`SkillVfxWiringManifestTest`）：经 classloader 读同一份清单，断言
//!   `VfxBootstrap.registerDefaults()` 后每个 id 都能查到、且落到本表声明的
//!   `VfxPlayer` 类上。
//!
//! 清单**不可手改**——唯一重生成入口：
//! `cd server && BONG_REGEN_VFX_MANIFEST=1 cargo test skill_vfx_wiring`。

use crate::combat::zhenmai_v2;
use crate::cultivation::burst_meridian;
use crate::npc::npc_skill;

/// 一招的粒子接线：招式 id → 发射的 event_id → client 播放器类名。
///
/// `legacy_event_id` 记录 P5 之前借用的那个 id，供负向回归断言使用——去复用一旦
/// 被回退（有人把常量改回借用），server 侧对应测试立刻撞红。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SkillVfxWiring {
    /// 招式 id（`SkillRegistry` 注册名）。
    pub skill_id: &'static str,
    /// server 发射的 `SpawnParticle` event_id。
    pub event_id: &'static str,
    /// `#RRGGBB` 粒子颜色。
    pub color: &'static str,
    /// client 侧承接该 event 的 `VfxPlayer` 实现类名（simple name）。
    pub player_class: &'static str,
    /// P5 之前借用的 event_id（负向断言用，不参与运行时）。
    pub legacy_event_id: &'static str,
}

/// P5 去复用后的 11 条接线（真脉 5 + 爆脉 3 + NPC 3）。
///
/// 顺序即 plan §P5.2 矩阵表的行序，方便逐行对读。
pub const P5_SKILL_VFX_WIRING: &[SkillVfxWiring] = &[
    // ── 真脉 5 招 → ZhenmaiPulsePlayer（金脉 #D4AF6A 色系，明度阶梯）──────────
    SkillVfxWiring {
        skill_id: zhenmai_v2::PARRY_SKILL_ID,
        event_id: zhenmai_v2::PARRY_PARTICLE_ID,
        color: zhenmai_v2::PARRY_PARTICLE_COLOR,
        player_class: "ZhenmaiPulsePlayer",
        legacy_event_id: "bong:jiemai_burst_blood",
    },
    SkillVfxWiring {
        skill_id: zhenmai_v2::NEUTRALIZE_SKILL_ID,
        event_id: zhenmai_v2::NEUTRALIZE_PARTICLE_ID,
        color: zhenmai_v2::NEUTRALIZE_PARTICLE_COLOR,
        player_class: "ZhenmaiPulsePlayer",
        legacy_event_id: "bong:jiemai_neutralize_dust",
    },
    SkillVfxWiring {
        skill_id: zhenmai_v2::MULTIPOINT_SKILL_ID,
        event_id: zhenmai_v2::MULTIPOINT_PARTICLE_ID,
        color: zhenmai_v2::MULTIPOINT_PARTICLE_COLOR,
        player_class: "ZhenmaiPulsePlayer",
        legacy_event_id: "bong:jiemai_burst_blood",
    },
    SkillVfxWiring {
        skill_id: zhenmai_v2::HARDEN_SKILL_ID,
        event_id: zhenmai_v2::HARDEN_PARTICLE_ID,
        color: zhenmai_v2::HARDEN_PARTICLE_COLOR,
        player_class: "ZhenmaiPulsePlayer",
        legacy_event_id: "bong:jiemai_neutralize_dust",
    },
    SkillVfxWiring {
        skill_id: zhenmai_v2::SEVER_CHAIN_SKILL_ID,
        event_id: zhenmai_v2::SEVER_SNAP_PARTICLE_ID,
        color: zhenmai_v2::SEVER_SNAP_PARTICLE_COLOR,
        player_class: "ZhenmaiPulsePlayer",
        legacy_event_id: "bong:jiemai_sever_flash",
    },
    // ── 爆脉 3 招 → BurstMeridianFamilyPlayer（共用 #C58B3F，纯形态分化）──────
    SkillVfxWiring {
        skill_id: burst_meridian::TIE_SHAN_KAO_SKILL_ID,
        event_id: burst_meridian::TIE_SHAN_KAO_PARTICLE_ID,
        color: burst_meridian::BURST_MERIDIAN_FAMILY_COLOR,
        player_class: "BurstMeridianFamilyPlayer",
        legacy_event_id: "bong:burst_meridian_beng_quan",
    },
    SkillVfxWiring {
        skill_id: burst_meridian::XUE_BENG_BU_SKILL_ID,
        event_id: burst_meridian::XUE_BENG_BU_PARTICLE_ID,
        color: burst_meridian::BURST_MERIDIAN_FAMILY_COLOR,
        player_class: "BurstMeridianFamilyPlayer",
        legacy_event_id: "bong:burst_meridian_beng_quan",
    },
    SkillVfxWiring {
        skill_id: burst_meridian::NI_MAI_HU_TI_SKILL_ID,
        event_id: burst_meridian::NI_MAI_HU_TI_PARTICLE_ID,
        color: burst_meridian::BURST_MERIDIAN_FAMILY_COLOR,
        player_class: "BurstMeridianFamilyPlayer",
        legacy_event_id: "bong:burst_meridian_beng_quan",
    },
    // ── NPC 3 招 → NpcSkillAuraPlayer（形态从简，绿/黄/蓝高分离三元组）────────
    SkillVfxWiring {
        skill_id: "npc.heal_basic",
        event_id: npc_skill::HEAL_PARTICLE_ID,
        color: npc_skill::HEAL_PARTICLE_COLOR,
        player_class: "NpcSkillAuraPlayer",
        legacy_event_id: "bong:yidao_meridian_repair",
    },
    SkillVfxWiring {
        skill_id: "npc.buff_speed",
        event_id: npc_skill::BUFF_SPEED_PARTICLE_ID,
        color: npc_skill::BUFF_SPEED_PARTICLE_COLOR,
        player_class: "NpcSkillAuraPlayer",
        legacy_event_id: "bong:jiemai_neutralize_dust",
    },
    SkillVfxWiring {
        skill_id: "npc.buff_defense",
        event_id: npc_skill::BUFF_DEFENSE_PARTICLE_ID,
        color: npc_skill::BUFF_DEFENSE_PARTICLE_COLOR,
        player_class: "NpcSkillAuraPlayer",
        legacy_event_id: "bong:burst_meridian_beng_quan",
    },
];

/// 按 `skill_id` 取接线行。找不到返回 `None`（测试用）。
pub fn wiring_for(skill_id: &str) -> Option<&'static SkillVfxWiring> {
    P5_SKILL_VFX_WIRING
        .iter()
        .find(|wiring| wiring.skill_id == skill_id)
}
