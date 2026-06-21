use std::collections::HashMap;

use valence::prelude::{Entity, Resource};

use crate::cultivation::components::MeridianId;

#[derive(Debug, Clone, PartialEq)]
pub enum CastResult {
    Started {
        cooldown_ticks: u64,
        anim_duration_ticks: u32,
    },
    Rejected {
        reason: CastRejectReason,
    },
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastRejectReason {
    RealmTooLow,
    /// 招式依赖经脉永久 SEVERED 或当前 integrity 不可用。`Option<MeridianId>` 标
    /// 出首条不满足的依赖经脉；旧调用点未携带具体 id 时为 `None`（plan-meridian-
    /// severed-v1 上线后逐步收口）。
    MeridianSevered(Option<MeridianId>),
    QiInsufficient,
    OnCooldown,
    InvalidTarget,
    InRecovery,
    /// 招式需手持对应武器（如剑技需持剑）但当前手里没有合规武器。
    /// 仅用于把"缺武器"反馈给玩家——是否真正放出由各 resolver 既有判定决定，
    /// 本变体不改施法逻辑，只让通用警示 HUD 能区分"缺武器"与"目标无效"。
    NoWeapon,
}

impl CastRejectReason {
    /// 兼容旧调用点 —— 不带 meridian_id 的 `MeridianSevered`。
    pub const MERIDIAN_SEVERED: CastRejectReason = CastRejectReason::MeridianSevered(None);

    /// 把施法被拒原因映射到推给 client 的 `CastOutcomeV1`（通用技能警示 HUD 用）。
    ///
    /// 纯反馈：cast 已被各 resolver 既有逻辑拒绝，本映射只决定"告诉玩家哪条原因"。
    /// `MeridianSevered` 复用既有 `MeridianGated`（经脉门控本就推这个 outcome）。
    /// 全 match、无 catch-all：新增 `CastRejectReason` 变体不加映射会触发 rustc
    /// non-exhaustive 编译错误，逼使警示文案保持对所有原因生效（通用性不变量）。
    pub fn to_cast_outcome(self) -> crate::schema::combat_hud::CastOutcomeV1 {
        use crate::schema::combat_hud::CastOutcomeV1;
        match self {
            CastRejectReason::RealmTooLow => CastOutcomeV1::RejectRealmTooLow,
            CastRejectReason::MeridianSevered(_) => CastOutcomeV1::MeridianGated,
            CastRejectReason::QiInsufficient => CastOutcomeV1::RejectQiInsufficient,
            CastRejectReason::OnCooldown => CastOutcomeV1::RejectOnCooldown,
            CastRejectReason::InvalidTarget => CastOutcomeV1::RejectInvalidTarget,
            CastRejectReason::InRecovery => CastOutcomeV1::RejectInRecovery,
            CastRejectReason::NoWeapon => CastOutcomeV1::RejectNoWeapon,
        }
    }
}

pub type SkillFn = fn(
    &mut valence::prelude::bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult;

#[derive(Default)]
pub struct SkillRegistry {
    entries: HashMap<&'static str, SkillFn>,
}

impl Resource for SkillRegistry {}

impl SkillRegistry {
    pub fn register(&mut self, skill_id: &'static str, skill_fn: SkillFn) {
        self.entries.insert(skill_id, skill_fn);
    }

    pub fn lookup(&self, skill_id: &str) -> Option<SkillFn> {
        self.entries.get(skill_id).copied()
    }

    /// 遍历所有已注册 skill_id 与 resolver fn-pointer 对。用于审计测试。
    pub fn iter(&self) -> impl Iterator<Item = (&&'static str, &SkillFn)> {
        self.entries.iter()
    }
}

pub fn init_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::default();
    crate::combat::carrier::register_skills(&mut registry);
    crate::combat::anqi_v2::register_skills(&mut registry);
    crate::cultivation::burst_meridian::register_skills(&mut registry);
    crate::combat::jiemai::register_skills(&mut registry);
    crate::combat::zhenmai_v2::register_skills(&mut registry);
    crate::combat::woliu::register_skills(&mut registry);
    crate::combat::yidao::register_skills(&mut registry);
    crate::combat::woliu_v2::register_skills(&mut registry);
    crate::combat::dugu_v2::register_skills(&mut registry);
    crate::combat::baomai_v3::register_skills(&mut registry);
    crate::combat::tuike_v2::register_skills(&mut registry);
    crate::combat::sword_basics::register_skills(&mut registry);
    crate::cultivation::dugu::register_skills(&mut registry);
    crate::cultivation::full_power_strike::register_skills(&mut registry);
    crate::dandao::register_skills(&mut registry);
    crate::sword_path::skill_register::register_skills(&mut registry);
    crate::npc::npc_skill::register_npc_skills(&mut registry);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::meridian::severed::SkillMeridianDependencies;
    use crate::schema::combat_hud::CastOutcomeV1;

    /// 通用技能警示：每个 `CastRejectReason` 变体都映射到一个非中性 `CastOutcomeV1`。
    /// 锁住"覆盖所有拒绝原因"不变量——新增 reject 变体若忘了映射，rustc 会先在
    /// `to_cast_outcome` 的 non-exhaustive match 处编译红，这里再正向锁每条映射目标。
    #[test]
    fn every_reject_reason_maps_to_distinct_warning_outcome() {
        // 列举全部变体（含 MeridianSevered 的 Some/None 两态）。
        let cases: &[(CastRejectReason, CastOutcomeV1)] = &[
            (
                CastRejectReason::RealmTooLow,
                CastOutcomeV1::RejectRealmTooLow,
            ),
            (
                CastRejectReason::QiInsufficient,
                CastOutcomeV1::RejectQiInsufficient,
            ),
            (
                CastRejectReason::OnCooldown,
                CastOutcomeV1::RejectOnCooldown,
            ),
            (
                CastRejectReason::InvalidTarget,
                CastOutcomeV1::RejectInvalidTarget,
            ),
            (
                CastRejectReason::InRecovery,
                CastOutcomeV1::RejectInRecovery,
            ),
            (CastRejectReason::NoWeapon, CastOutcomeV1::RejectNoWeapon),
            // MeridianSevered 两态都复用既有 MeridianGated（经脉门控本就推这个）。
            (
                CastRejectReason::MeridianSevered(None),
                CastOutcomeV1::MeridianGated,
            ),
            (
                CastRejectReason::MeridianSevered(Some(MeridianId::Lung)),
                CastOutcomeV1::MeridianGated,
            ),
        ];
        for (reason, expected) in cases {
            let got = reason.to_cast_outcome();
            assert_eq!(
                got, *expected,
                "CastRejectReason::{reason:?} 应映射到 {expected:?}（通用警示文案据此区分），实际 {got:?}"
            );
            // 拒绝原因绝不能落到 None/Completed 这类"无警示"outcome，否则玩家看不到为什么被拒。
            assert_ne!(
                got,
                CastOutcomeV1::None,
                "CastRejectReason::{reason:?} 映射成了 None —— 会导致警示 HUD 静默，违反通用性"
            );
            assert_ne!(got, CastOutcomeV1::Completed, "拒绝不应映射成 Completed");
        }
    }

    /// NoWeapon 是新增的"缺武器"专用拒绝原因，必须映射到独立的 RejectNoWeapon，
    /// 而非旧的 InvalidTarget——否则玩家分不清"目标无效"和"手里没武器"。
    #[test]
    fn no_weapon_maps_to_dedicated_outcome_not_invalid_target() {
        assert_eq!(
            CastRejectReason::NoWeapon.to_cast_outcome(),
            CastOutcomeV1::RejectNoWeapon
        );
        assert_ne!(
            CastRejectReason::NoWeapon.to_cast_outcome(),
            CastOutcomeV1::RejectInvalidTarget,
            "缺武器必须有独立 outcome，文案为「缺少武器」而非「目标无效」"
        );
    }

    /// 构造与生产路径完全一致的 SkillMeridianDependencies（不走 Bevy App）。
    fn build_production_deps() -> SkillMeridianDependencies {
        let mut deps = SkillMeridianDependencies::default();
        crate::combat::zhenmai_v2::declare_meridian_dependencies(&mut deps);
        crate::combat::anqi_v2::declare_meridian_dependencies(&mut deps);
        crate::combat::dugu_v2::declare_meridian_dependencies(&mut deps);
        crate::combat::tuike_v2::declare_meridian_dependencies(&mut deps);
        crate::combat::sword_basics::declare_meridian_dependencies(&mut deps);
        crate::combat::shield_block::declare_meridian_dependencies(&mut deps);
        crate::sword_path::skill_register::declare_meridian_dependencies(&mut deps);
        crate::movement::dash_proficiency::declare_dash_meridian_dependencies(&mut deps);
        crate::npc::npc_skill::declare_npc_skill_meridian_deps(&mut deps);
        crate::combat::woliu::declare_meridian_dependencies(&mut deps);
        crate::cultivation::burst_meridian::declare_meridian_dependencies(&mut deps);
        crate::combat::yidao::declare_meridian_dependencies(&mut deps);
        crate::dandao::declare_meridian_dependencies(&mut deps);
        // dugu 两招无经脉前置，显式声明空 deps 以满足审计完整性不变量。
        crate::cultivation::dugu::declare_meridian_dependencies(&mut deps);
        // woliu_v2 は Bevy Startup system 経由 — 無 ECS バリアントで代替。
        crate::combat::woliu_v2::skills::declare_woliu_v2_deps_direct(&mut deps);
        // baomai_v3 は App Resource 経由 — production declare fn を直接呼ぶ。
        crate::combat::baomai_v3::skills::declare_meridian_dependencies(&mut deps);
        deps
    }

    /// 審計不変量（宣言完整性）：SkillRegistry に登録された招式のうち
    /// TECHNIQUE_DEFINITIONS に存在するもの（プレイヤー技能バー使用可能かつ resolver が存在する）は
    /// すべて SkillMeridianDependencies に declare が存在しなければならない。
    ///
    /// **重要な担保範囲の制限**：
    /// - 本不変量は「宣言が存在すること」を保証するのみであり、resolver が実行時に
    ///   check_meridian_dependencies を呼び出すことは保証しない。
    ///   経脈ゲートが運行時に強制されるかどうかは P0 cast-entry 統一拦截の範疇。
    /// - 本不変量の対象は SkillRegistry ∩ TECHNIQUE_DEFINITIONS の積集合のみ。
    ///   TECHNIQUE_DEFINITIONS に存在するが SkillRegistry に未登録（skeleton）の招式は
    ///   本不変量の外であり防御できない。
    ///
    ///   burst_meridian の 3 招（tie_shan_kao / xue_beng_bu / ni_mai_hu_ti）は
    ///   かつて skeleton だったが現在は resolver 登録 + declare 済み
    ///   （`cultivation::burst_meridian::register_skills` / `declare_meridian_dependencies`）。
    ///   いずれかの register から declare を外すと本不変量が即赤になる。
    ///
    /// 故意に declare を削除すると本テストが赤くなり、将来の register-without-declare 漏れを防ぐ。
    #[test]
    fn every_player_castable_skill_has_meridian_dependency_declaration() {
        use crate::cultivation::known_techniques::TECHNIQUE_DEFINITIONS;
        use std::collections::HashSet;

        let deps = build_production_deps();
        let registry = init_registry();

        // 対象は SkillRegistry ∩ TECHNIQUE_DEFINITIONS の積集合。
        // burst_meridian 3 招は登録 + declare 済みなのでここで網羅される
        // （declare を外すと missing に乗って赤になる）。
        let player_castable: HashSet<&str> =
            TECHNIQUE_DEFINITIONS.iter().map(|def| def.id).collect();

        let mut missing: Vec<&str> = Vec::new();
        for (skill_id, _fn_ptr) in registry.iter() {
            if player_castable.contains(*skill_id) && !deps.is_declared(skill_id) {
                missing.push(skill_id);
            }
        }
        missing.sort_unstable();
        assert!(
            missing.is_empty(),
            "SkillRegistry に登録された player-castable 招式が SkillMeridianDependencies \
             に未宣言です。declare_meridian_dependencies に追加してください。\
             注意：本不変量は宣言の存在のみを保証し、resolver が実行時に \
             check_meridian_dependencies を呼び出すかどうかは検証しません \
             （経脈ゲートの運行時強制は P0 cast-entry 統一拦截の範疇）。\
             未宣言の招式: {:?}",
            missing
        );
    }

    /// 証明テスト：woliu.vortex の declare を削除すると上のテストが赤くなる構造を確認する。
    /// woliu.vortex は TECHNIQUE_DEFINITIONS にあり、deps に宣言が存在する。
    #[test]
    fn woliu_vortex_is_declared_in_production_deps() {
        let deps = build_production_deps();
        assert!(
            deps.is_declared("woliu.vortex"),
            "woliu.vortex must be declared in SkillMeridianDependencies; \
             it is player-castable and gates on Lung meridian"
        );
        assert_eq!(
            deps.lookup("woliu.vortex"),
            &[crate::cultivation::components::MeridianId::Lung],
            "woliu.vortex should declare exactly [Lung] as its meridian dependency"
        );
    }

    /// 証明テスト：burst_meridian.beng_quan の declare が正しい経脈集合を持つ。
    #[test]
    fn burst_meridian_beng_quan_is_declared_in_production_deps() {
        use crate::cultivation::components::MeridianId;
        let deps = build_production_deps();
        assert!(
            deps.is_declared("burst_meridian.beng_quan"),
            "burst_meridian.beng_quan must be declared; it is player-castable"
        );
        let declared = deps.lookup("burst_meridian.beng_quan");
        assert!(
            declared.contains(&MeridianId::LargeIntestine),
            "beng_quan must declare LargeIntestine"
        );
        assert!(
            declared.contains(&MeridianId::SmallIntestine),
            "beng_quan must declare SmallIntestine"
        );
        assert!(
            declared.contains(&MeridianId::TripleEnergizer),
            "beng_quan must declare TripleEnergizer"
        );
    }

    /// 証明テスト：全登録スキルのうち declare が存在するものが実際に宣言されている。
    /// 将来 declare を 1 つ削除すると audit テストが赤くなる構造を担保する。
    #[test]
    fn declared_count_matches_expected_minimum() {
        let deps = build_production_deps();
        // woliu + burst_meridian + yidao(5) + dandao(3) + dugu(2) が今回追加された 12 件。
        // 既存宣言は zhenmai_v2(5) + anqi_v2(6) + dugu_v2(5) + tuike_v2(3) + sword_basics(4)
        //   + shield_block(1) + sword_path(5) + dash(1) + npc(3) + baomai_v3(6) + woliu_v2(15)
        //   = 既存 54 件。合計 ≥ 66。
        let count: usize = deps.declared_skills().count();
        assert!(
            count >= 66,
            "expected at least 66 declared skills (54 existing + 12 new gaps), got {}",
            count
        );
    }
}
