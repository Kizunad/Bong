use std::collections::BTreeMap;

use valence::prelude::{bevy_ecs, Event, Resource};

use crate::cultivation::components::Cultivation;
use crate::inventory::{ItemInstance, PlayerInventory};
use crate::world::zone::ZoneRegistry;

use super::constants::{DEFAULT_SPIRIT_QI_TOTAL, QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use super::{finite_non_negative, QiPhysicsError};

const SPIRIT_QI_TOTAL_ENV: &str = "BONG_SPIRIT_QI_TOTAL";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldQiBudget {
    pub initial_total: f64,
    pub current_total: f64,
    pub era_decay_accum: f64,
}

impl Resource for WorldQiBudget {}

impl Default for WorldQiBudget {
    fn default() -> Self {
        Self::from_total(DEFAULT_SPIRIT_QI_TOTAL)
    }
}

impl WorldQiBudget {
    pub fn from_total(total: f64) -> Self {
        let total = if total.is_finite() && total > 0.0 {
            total
        } else {
            DEFAULT_SPIRIT_QI_TOTAL
        };
        Self {
            initial_total: total,
            current_total: total,
            era_decay_accum: 0.0,
        }
    }

    pub fn from_env() -> Self {
        std::env::var(SPIRIT_QI_TOTAL_ENV)
            .ok()
            .and_then(|raw| raw.parse::<f64>().ok())
            .map(Self::from_total)
            .unwrap_or_default()
    }

    pub fn apply_era_decay(&mut self, ratio: f64) -> Result<f64, QiPhysicsError> {
        let ratio = finite_non_negative(ratio, "era_decay_ratio")?.clamp(0.0, 1.0);
        let decay = self.current_total * ratio;
        self.current_total = (self.current_total - decay).max(0.0);
        self.era_decay_accum += decay;
        Ok(decay)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QiAccountKind {
    Player,
    Npc,
    Zone,
    Container,
    Rift,
    Tiandao,
    Overflow,
}

impl QiAccountKind {
    /// plan-offscreen-war-v1 P0：`bong:qi/ledger` per-account 字段 key 的**稳定** wire 串。
    ///
    /// 不能用 `{:?}`（Debug）——那会把外部 Redis schema 绑死到 Rust 变体名，重命名
    /// 变体会静默改掉 wire 契约。这里显式锁定 lowercase 串，改名变体编译期 exhaustive
    /// 检查会逼着同步更新此处（即 wire 变更必须是有意识的）。
    fn as_wire_str(self) -> &'static str {
        match self {
            QiAccountKind::Player => "player",
            QiAccountKind::Npc => "npc",
            QiAccountKind::Zone => "zone",
            QiAccountKind::Container => "container",
            QiAccountKind::Rift => "rift",
            QiAccountKind::Tiandao => "tiandao",
            QiAccountKind::Overflow => "overflow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QiAccountId {
    pub kind: QiAccountKind,
    pub id: String,
}

impl QiAccountId {
    pub fn new(kind: QiAccountKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }

    pub fn player(id: impl Into<String>) -> Self {
        Self::new(QiAccountKind::Player, id)
    }

    pub fn npc(id: impl Into<String>) -> Self {
        Self::new(QiAccountKind::Npc, id)
    }

    pub fn zone(id: impl Into<String>) -> Self {
        Self::new(QiAccountKind::Zone, id)
    }

    pub fn container(id: impl Into<String>) -> Self {
        Self::new(QiAccountKind::Container, id)
    }

    pub fn rift(id: impl Into<String>) -> Self {
        Self::new(QiAccountKind::Rift, id)
    }

    pub fn tiandao() -> Self {
        Self::new(QiAccountKind::Tiandao, "tiandao")
    }

    pub fn overflow(id: impl Into<String>) -> Self {
        Self::new(QiAccountKind::Overflow, id)
    }
}

impl std::fmt::Display for QiAccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 用稳定 wire 串而非 Debug，锁住 `account:<kind>:<id>` 的外部契约。
        write!(f, "{}:{}", self.kind.as_wire_str(), self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QiTransferReason {
    CultivationRegen,
    Excretion,
    ReleaseToZone,
    Collision,
    Channeling,
    /// bughunt r5 — 经脉打通进度消耗 `qi_current` 后，真元逸散回所在 zone ledger。
    ///
    /// 守恒约束：`cultivation.qi_current -= cost`；同 tick 内必须把 cost 记入
    /// `WorldQiAccount` 的 zone 账户，并追加
    /// `QiTransfer(from=player/npc:<id>, to=zone:<name>, reason=MeridianOpen)` 审计轨迹。
    /// 玩家/NPC 实际真元仍活在 ECS 组件中，不镜像到 player/npc ledger balance。
    MeridianOpen,
    /// bughunt r6 — 境界突破仪式消耗 `qi_current` 后，真元逸散回所在 zone ledger。
    ///
    /// 守恒约束：
    ///   - 玩家/在线 NPC：`cultivation.qi_current -= cost`；同 tick 内把 cost 记入
    ///     zone 账户，并追加 `QiTransfer(from=player/npc:<id>, to=zone:<name>,
    ///     reason=Breakthrough)` audit。活体真元仍在 ECS，不镜像到 ledger 余额。
    ///   - dormant NPC：真元已镜像在 `WorldQiAccount` 的 npc 账户，必须走真实
    ///     `WorldQiAccount::transfer(npc -> zone)`，否则快照与账本余额会分叉。
    Breakthrough,
    /// bughunt r7 — 经脉锻造 tier 消耗 `qi_current` 后，真元逸散回所在 zone ledger。
    ///
    /// 守恒约束：`cultivation.qi_current -= cost`；同 tick 内把 cost 记入 zone 账户，并追加
    /// `QiTransfer(from=player/npc:<id>, to=zone:<name>, reason=MeridianForge)` audit。
    /// 活体真元仍在 ECS，不镜像到 player/npc ledger balance。
    MeridianForge,
    RiftCollapse,
    /// bughunt QS-01 — 裂口气压负压（rift-mouth neg_pressure）每 tick 从附近**玩家/非 NPC actor**
    /// （tick_neg_pressure 查询带 `Without<NpcMarker>`，不抽 NPC）qi_current 抽走真元，守恒转入 rift
    /// ledger 账户。
    ///
    /// 守恒约束：
    ///   - `cultivation.qi_current -= actual_drain`（ECS，已在 tick_neg_pressure 扣减）；
    ///   - 同 tick 内把 `actual_drain` 记入 `QiAccountId::rift(zone_label)` 账户；
    ///   - `push_transfer_audit(QiTransfer(from=player:<entity>, to=rift:<label>,
    ///     reason=NegPressureDrain))` 留审计轨迹；
    ///   - 活体真元仍在 ECS，不镜像到 player/npc ledger balance（audit-only 模式）；
    ///   - `summarize_world_qi` 口径：player_qi 减少，ledger_qi（rift 账户）增加，总量不变。
    NegPressureDrain,
    EraDecay,
    /// plan-zone-qi-economy-v1 — 手搓 qi_cost 一次性投入待分配池；
    /// 后续 zone 回流由 heartbeat 以 ZoneInflow 单独审计，区别于 ReleaseToZone（招式释放）。
    Crafting,
    /// plan-void-actions-v1 — 化虚世界级 action 的真元投入，必须保留 ledger 轨迹。
    VoidAction,
    /// plan-yidao-v1 — 医者把自身真元转入患者治疗路径，守恒轨迹必须可追溯。
    Healing,
    /// plan-halfstep-buff-v1 P1 — 半步化虚 buff 容量扩张（qi_max ×1.10）的 audit-only 标记。
    ///
    /// 半步 buff 是**容量扩张**，不是真元搬运（worldview §三:78 化虚稀缺 + qi_physics 守恒律）。
    /// 此变种用于在 ledger 留下"天道授予 N 真元容量"的可审计轨迹，amount = bonus capacity；
    /// 实际 qi_current 不变、SPIRIT_QI_TOTAL 不变。emit 为 event，不调 `WorldQiAccount::transfer`
    /// （后者会变动 balance）。
    HalfStepBuff,
    /// plan-dandao-runtime-wiring-v1 P4 — 暴龙王真元吸取光环。
    ///
    /// worldview §十六:1572「负压畸变体」正典依据：负压畸变体通过真元吸取光环持续
    /// 从附近修士真元库抽取 +50%。吸来的真元守恒转入 zone 账户（坍缩渊），
    /// **不得凭空消失**。amount = 玩家实际被吸走量（经光环计算，已扣除 50% 加成）。
    BossDrain,
    /// bughunt r8 — 骨煞冲撞命中时从目标 qi_current 抽走的真元逸散回目标所在 zone。
    ///
    /// 守恒约束：`target.cultivation.qi_current -= actual_drain`；同 tick 内把
    /// `actual_drain` 记入 zone 账户，并追加
    /// `QiTransfer(from=player/npc:<target>, to=zone:<name>, reason=SkullFiendDrain)` audit。
    /// 活体真元仍在 ECS，不镜像到 player/npc ledger balance。
    SkullFiendDrain,
    /// plan-fauna-stitched-beast-v1 P0 — 异变缝合兽融合时 N 只野兽 qi_current 合并到 HybridBeast。
    ///
    /// worldview §七「几只野兽相互吞噬」正典依据：低灵气饥饿状态下，低阶野兽融合为缝合兽，
    /// 真元加和后按 FUSION_RETAIN_RATIO 保留于 HybridBeast，余下 20% 走 ReleaseToZone 逸散。
    ///
    /// 守恒约束：
    ///   - 每只组件兽：emit QiTransfer(from=npc:<beast_id>, to=npc:<hybrid_id>, reason=FusionMerge)
    ///   - 逸散 20%：emit QiTransfer(from=npc:<hybrid_id>, to=zone:<zone_name>, reason=ReleaseToZone)
    ///   - **sum(beast_qi) == hybrid_qi + released_to_zone**，无凭空消失
    FusionMerge,
    /// plan-daozhan-v1 P0 — 道伥伏击时从玩家 qi_current 吸取真元，守恒转入道伥储量。
    ///
    /// 守恒约束：player.qi_current -= amount；daozhan.daozhan_qi += amount；
    /// QiTransfer(from=player:<id>, to=npc:daozhan:<id>, reason=DaoZhangDrain)。
    /// 凝结/坍缩渊死亡 spawn 的道伥初始 qi 走死亡转移，不走此路径。
    DaoZhangDrain,
    /// bughunt r12 — 鼠咬从玩家 qi_current 吸取真元，守恒转入 RatBlackboard.drained_qi。
    ///
    /// 守恒约束：player.qi_current -= amount；rat.drained_qi += amount；
    /// QiTransfer(from=player:<id>, to=npc:rat:<id>, reason=RatBiteDrain)。
    /// 鼠死亡时 release_drained_qi_on_death_system 按衰减比例归还 zone。
    RatBiteDrain,
    /// plan-daozhan-v1 P0 — 天道凝结道伥时从高浓度 zone.spirit_qi 凝出初始真元。
    ///
    /// 守恒约束：zone.spirit_qi -= delta；daozhan.qi_init = condensed_amount；
    /// QiTransfer(from=zone:<name>, to=npc:daozhan:<id>, reason=TiandaoCondense)。
    /// 绝不凭空创生：zone 必须先减，道伥才获得真元。
    TiandaoCondense,
    /// plan-dying-elder-v1 P0 — 玩家向垂死大能交付回元丹，丹携带的 qi 转入大能 qi_current。
    ///
    /// 守恒约束：丹从玩家背包消耗（inventory 真删）；丹携带的 qi_gain 值走本 reason 转入大能；
    /// QiTransfer(from=item:hui_yuan_pill:<instance_id>, to=npc:dying_elder:<id>, reason=TradeDan)。
    /// 丹的 qi 来自炼丹时 zone 灵气凝结（已在 alchemy plan 记账），此处只搬运成品 qi，不凭空创生。
    TradeDan,
    /// plan-dying-elder-v1 P0 — 垂死大能翻脸夺舍时，从玩家 qi_current 吸取真元转入大能。
    ///
    /// 守恒约束：
    ///   - `player.qi_current` 清零（实际转移量 = 玩家当前 qi_current）走本 reason；
    ///   - `player.qi_max -= soul_seize_drain`（永久容量 debuff，**不是** qi 搬运——守恒只作用于
    ///     qi_current 转移；qi_max 减少是单独的容量变化，不重复计入 QiTransfer）；
    ///   - QiTransfer(from=player:<uuid>, to=npc:dying_elder:<id>, reason=SoulSeize)；
    ///   - 凭空吸取红线：玩家 qi 减少量必须等于大能 qi 增加量，qi_max debuff 不影响此不变式。
    ///
    /// worldview 正典依据（§七「无人可信，算计至上」）：夺舍是末法最惨结局，永久烙印强化危机感。
    SoulSeize,
    /// plan-tiandao-hunt-v1 P4 — Watch 级天道微调区域灵气。
    ///
    /// 守恒约束：zone.spirit_qi -= delta；对应 delta 先镜像到 zone ledger 源账户，
    /// 再通过 QiTransfer(from=zone:<name>, to=tiandao:tiandao, reason=TiandaoWatchDrain)
    /// 转入天道账户。summarize_world_qi 口径下 zone_qi 降低、ledger_qi 增加，总量不变。
    TiandaoWatchDrain,
    /// plan-era-state-v1 P0 — 变化时代潮汐涌动（正向 qi 搬运）。
    ///
    /// 守恒约束：搬运必须是账户间 QiTransfer（from=zone:<src>, to=zone:<dst>），
    /// 两端 balance 一增一减，initial_total 恒定。不凭空增减。
    /// 负向衰减走 [`QiTransferReason::EraDecay`] + [`crate::qi_physics::tiandao::era_decay_step`]。
    EraShift,
    /// plan-qi-handling-attrition-v1 P0 — 搬运灵物天道税（worldview §八.2）。
    ///
    /// inventory 操作对 spirit_quality>0 物品施加磨损，逸散量守恒归还玩家所在 zone。
    /// 守恒约束：item.spirit_quality 减少量(绝对) == zone 接收量，不凭空消失。
    /// op_kind 区分操作类型（拾起/移动/搜刮/炼器/炼丹），供审计轨迹按类型区分。
    AttritionTax {
        op_kind: AttritionOpKind,
    },
    /// plan-qixiu-depth-v1 P2 — 法器铭纹每日养护消耗玩家真元。
    ///
    /// 守恒约束：`cultivation.qi_current -= cost`；逸散回玩家所在 zone ledger；
    /// `QiTransfer(from=player:<entity_bits>, to=zone:<name>, reason=ArtifactMaintenance)`。
    /// zone 不可解析时 fallback overflow，真元绝不凭空消失。
    ArtifactMaintenance,
    /// plan-qixiu-depth-v1 P2 — 法器品阶跃升时玩家真元消耗（30%）。
    ///
    /// 守恒约束：`cultivation.qi_current -= cost（= qi_current × 0.3）`；
    /// 逸散回玩家所在 zone ledger；
    /// `QiTransfer(from=player:<entity_bits>, to=zone:<name>, reason=ArtifactEvolution)`。
    /// zone 不可解析时 fallback overflow，真元绝不凭空消失。
    ArtifactEvolution,
    /// plan-qi-conservation-leaks-v1 P4 — 毒蛊脏真元过渡态散回施法者所在 zone。
    ///
    /// worldview §六.2 正典依据：脏真元注入目标体内后，99% 经异体排斥最终散回受害者所在 zone
    /// （`DUGU_DIRTY_QI_ZONE_RETURN_RATIO`），守恒必须落账。
    ///
    /// 守恒约束：
    ///   - zone.spirit_qi += returned_zone_qi；
    ///   - `push_transfer_audit(QiTransfer(from=player:<caster>, to=zone:<name>, reason=DuguReturnToZone))`；
    ///   - ECS `Cultivation.qi_current` 已在 apply_eclipse / apply_reverse 中扣减，
    ///     **不得再动 player ledger 账户**（player qi 活在 ECS，不在 WorldQiAccount balance）；
    ///   - 此路径是 audit-only + zone balance 更新，**禁止**调 `WorldQiAccount::transfer`
    ///     （后者会检查 player ledger 余额并拒绝）。
    DuguReturnToZone,
    /// bughunt r8 — Reverse（倒蚀）清零受害者 qi_current 时，被消灭的真元守恒归还受害者所在 zone。
    ///
    /// 与 `DuguReturnToZone`（脏真元残留散逸）**正交**：DuguReturnToZone 是 taint 残留按
    /// intensity × ratio 计算，此路径是受害者自身真元库清零量（可为大值）。
    ///
    /// 守恒约束：
    ///   - victim.qi_current 清零前先读取实际量（max(0, qi_current)），累加为 victim_qi_total；
    ///   - victim_qi_total 走 qi_release_to_zone 归还受害者脚下 zone；
    ///   - `push_transfer_audit(QiTransfer(from=npc/player:<victim>, to=zone:<name>, reason=DuguReverseVictimQi))`；
    ///   - 此路径是 audit-only + zone balance 更新，**禁止**调 `WorldQiAccount::transfer`。
    DuguReverseVictimQi,
    /// plan-zone-qi-economy-v1 P1 §8.1 决议 #5 — heartbeat 平衡回流：独立待分配池
    /// （`pending_inflow_account`）按 zone 的 `qi_equilibrium` / `qi_inflow_per_min` 配置
    /// 滴灌回 `zone.spirit_qi`。
    ///
    /// 守恒约束：
    ///   - `zone.spirit_qi` 增加量 == 待分配池账户减少量（换算系数 `QI_ZONE_UNIT_CAPACITY`）；
    ///   - 只补到 `qi_equilibrium` 即停（`zone_equilibrium_inflow` 已钳位），绝不过冲；
    ///   - 待分配池余额不足时缩量，绝不透支（**禁止**凭空创生）；
    ///   - `active_events` 含 `EVENT_REALM_COLLAPSE` 或 `zone.spirit_qi < 0.0`（负灵域）的
    ///     zone 本 reason 不生效（调用方 `continue`，不产生 `QiTransfer`）；
    ///   - 这是**真实 `WorldQiAccount::transfer`**（非 audit-only）：`from` 是待分配池、
    ///     `to` 是 zone 的 ledger 镜像账户，调用前需按 `apply_dormant_regen_with_multiplier`
    ///     范本先用 `set_balance` 把 zone 镜像同步到 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`
    ///     真实值，转账后再把结果写回 `zone.spirit_qi`。
    ZoneInflow,
    /// plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮（伪灵脉）生命周期归还：运行期
    /// 衰减与最终 dissipate 都把动态 zone 减少的真实余额归还独立待分配池。
    ///
    /// 修复旧版本缺陷：`settle_pseudo_vein_qi` 曾经只收回 30%、70% 永久留在 zone（凭空创生，
    /// 因为注入侧的 `from` 是不存在真实余额的 `QiAccountId::tiandao()`）。P3 改为
    /// `inject_zone_for_pseudo_vein` 从 `pending_inflow_account` 真实借出（`ReleaseToZone`），
    /// heartbeat 动态 zone 的生命周期衰减与 dissipate 使用本 reason 把减少量逐 tick 转回
    /// `pending_inflow_account`；依附既有 zone 的 runtime 则在 dissipate 时把**能还多少还多少**
    /// （`min(injected_qi, zone 当前绝对余额)`，不是固定比例）。借款期间被玩家/NPC 正常吸收的
    /// 部分已经通过既有 `regen_from_zone` 路径守恒记账，剩余未被吸收的部分才需要"还款"。
    ///
    /// 守恒约束：
    ///   - `zone.spirit_qi` 减少量 == `pending_inflow_account` 增加量（换算系数
    ///     `QI_ZONE_UNIT_CAPACITY`），二者必须精确相等，不凭空增减；
    ///   - 这是真实 `WorldQiAccount::transfer`（非 audit-only），调用前需按
    ///     `apply_dormant_regen_with_multiplier` 范本同步 zone ledger 镜像。
    PseudoVeinSettle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QiTransferDisposition {
    AuditOnly,
    BalanceMutating,
}

impl QiTransferReason {
    pub(crate) const fn disposition(self) -> QiTransferDisposition {
        match self {
            Self::HalfStepBuff
            | Self::DuguReturnToZone
            | Self::DuguReverseVictimQi
            | Self::NegPressureDrain => QiTransferDisposition::AuditOnly,
            Self::CultivationRegen
            | Self::Excretion
            | Self::ReleaseToZone
            | Self::Collision
            | Self::Channeling
            | Self::MeridianOpen
            | Self::Breakthrough
            | Self::MeridianForge
            | Self::RiftCollapse
            | Self::EraDecay
            | Self::Crafting
            | Self::VoidAction
            | Self::Healing
            | Self::BossDrain
            | Self::SkullFiendDrain
            | Self::FusionMerge
            | Self::DaoZhangDrain
            | Self::RatBiteDrain
            | Self::TiandaoCondense
            | Self::TradeDan
            | Self::SoulSeize
            | Self::TiandaoWatchDrain
            | Self::EraShift
            | Self::AttritionTax { .. }
            | Self::ArtifactMaintenance
            | Self::ArtifactEvolution
            | Self::ZoneInflow
            | Self::PseudoVeinSettle => QiTransferDisposition::BalanceMutating,
        }
    }
}

#[cfg(test)]
pub(crate) const ALL_CONCRETE_QI_TRANSFER_REASONS: [QiTransferReason; 36] = [
    QiTransferReason::CultivationRegen,
    QiTransferReason::Excretion,
    QiTransferReason::ReleaseToZone,
    QiTransferReason::Collision,
    QiTransferReason::Channeling,
    QiTransferReason::MeridianOpen,
    QiTransferReason::Breakthrough,
    QiTransferReason::MeridianForge,
    QiTransferReason::RiftCollapse,
    QiTransferReason::NegPressureDrain,
    QiTransferReason::EraDecay,
    QiTransferReason::Crafting,
    QiTransferReason::VoidAction,
    QiTransferReason::Healing,
    QiTransferReason::HalfStepBuff,
    QiTransferReason::BossDrain,
    QiTransferReason::SkullFiendDrain,
    QiTransferReason::FusionMerge,
    QiTransferReason::DaoZhangDrain,
    QiTransferReason::RatBiteDrain,
    QiTransferReason::TiandaoCondense,
    QiTransferReason::TradeDan,
    QiTransferReason::SoulSeize,
    QiTransferReason::TiandaoWatchDrain,
    QiTransferReason::EraShift,
    QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::Pickup,
    },
    QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::SlotMove,
    },
    QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::ContainerSearch,
    },
    QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::ForgeLoad,
    },
    QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::AlchemyLoad,
    },
    QiTransferReason::ArtifactMaintenance,
    QiTransferReason::ArtifactEvolution,
    QiTransferReason::DuguReturnToZone,
    QiTransferReason::DuguReverseVictimQi,
    QiTransferReason::ZoneInflow,
    QiTransferReason::PseudoVeinSettle,
];

/// plan-qi-handling-attrition-v1 P0 — 搬运磨损操作类型，对应不同基础磨损率。
///
/// 定义在 ledger.rs 内（与 `QiTransferReason` 同级），避免 attrition.rs ↔ ledger.rs 循环依赖。
/// attrition.rs 反向 use 此 enum。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttritionOpKind {
    /// 从地面拾起物品（base rate × 1.0 = 0.03）
    Pickup,
    /// 背包内槽位移动（base rate × 0.667 ≈ 0.02）
    SlotMove,
    /// TSY 容器搜刮入包（base rate × 1.667 ≈ 0.05）
    ContainerSearch,
    /// 炼器炉加料（base rate × 1.333 ≈ 0.04）
    ForgeLoad,
    /// 炼丹炉加料（base rate × 1.333 ≈ 0.04）
    AlchemyLoad,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct QiTransfer {
    pub from: QiAccountId,
    pub to: QiAccountId,
    pub amount: f64,
    pub reason: QiTransferReason,
}

impl QiTransfer {
    pub fn new(
        from: QiAccountId,
        to: QiAccountId,
        amount: f64,
        reason: QiTransferReason,
    ) -> Result<Self, QiPhysicsError> {
        let amount = finite_non_negative(amount, "transfer.amount")?;
        Ok(Self {
            from,
            to,
            amount,
            reason,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct WorldQiAccount {
    balances: BTreeMap<QiAccountId, f64>,
    transfers: Vec<QiTransfer>,
}

impl Resource for WorldQiAccount {}

fn audit_only_reason_label(reason: QiTransferReason) -> Option<&'static str> {
    if reason.disposition() != QiTransferDisposition::AuditOnly {
        return None;
    }
    Some(match reason {
        QiTransferReason::HalfStepBuff => "HalfStepBuff",
        QiTransferReason::DuguReturnToZone => "DuguReturnToZone",
        QiTransferReason::DuguReverseVictimQi => "DuguReverseVictimQi",
        QiTransferReason::NegPressureDrain => "NegPressureDrain",
        _ => unreachable!("all audit-only QiTransferReason variants need a stable error label"),
    })
}

pub fn reject_audit_only_qi_reason(reason: QiTransferReason) -> Result<(), QiPhysicsError> {
    if let Some(reason) = audit_only_reason_label(reason) {
        Err(QiPhysicsError::AuditOnlyReason { reason })
    } else {
        Ok(())
    }
}

fn checked_destination_credit(before: f64, amount: f64) -> Result<f64, QiPhysicsError> {
    let after = before + amount;
    if !after.is_finite() || (amount > 0.0 && after == before) {
        return Err(QiPhysicsError::InvalidAmount {
            field: "destination_balance",
            value: after,
        });
    }
    Ok(after)
}

fn checked_source_debit(before: f64, amount: f64) -> Result<f64, QiPhysicsError> {
    let after = before - amount;
    if !after.is_finite() || (amount > 0.0 && after == before) {
        return Err(QiPhysicsError::InvalidAmount {
            field: "source_balance",
            value: after,
        });
    }
    Ok(after)
}

/// `WorldQiAccount` 的轻量原子事务。
///
/// 事务只保存本次触及账户的 overlay 和本次新增的审计项；既有 `transfers` 历史不参与
/// 暂存，避免高频路径按完整审计向量的长度做深拷贝。事务闭合成功后一次性提交，闭合
/// 失败则丢弃 overlay，调用方看不到部分余额或审计记录。
pub(crate) struct WorldQiAccountTransaction<'a> {
    base_balances: &'a BTreeMap<QiAccountId, f64>,
    changes: BTreeMap<QiAccountId, Option<f64>>,
    transfers: Vec<QiTransfer>,
}

impl WorldQiAccountTransaction<'_> {
    fn balance(&self, account: &QiAccountId) -> f64 {
        match self.changes.get(account) {
            Some(Some(balance)) => *balance,
            Some(None) => 0.0,
            None => self.base_balances.get(account).copied().unwrap_or(0.0),
        }
    }

    fn has_account(&self, account: &QiAccountId) -> bool {
        match self.changes.get(account) {
            Some(balance) => balance.is_some(),
            None => self.base_balances.contains_key(account),
        }
    }

    pub(crate) fn set_balance(
        &mut self,
        account: QiAccountId,
        amount: f64,
    ) -> Result<(), QiPhysicsError> {
        let amount = finite_non_negative(amount, "balance")?;
        self.changes.insert(account, Some(amount));
        Ok(())
    }

    fn remove_balance(&mut self, account: &QiAccountId) {
        self.changes.insert(account.clone(), None);
    }

    fn transfer(&mut self, transfer: QiTransfer) -> Result<(), QiPhysicsError> {
        reject_audit_only_qi_reason(transfer.reason)?;

        let amount = finite_non_negative(transfer.amount, "transfer.amount")?;
        if transfer.from == transfer.to {
            return Err(QiPhysicsError::SameAccountTransfer {
                account: transfer.from.to_string(),
            });
        }
        let available = self.balance(&transfer.from);
        if amount > available {
            return Err(QiPhysicsError::InsufficientQi {
                account: transfer.from.to_string(),
                available,
                requested: amount,
            });
        }

        let to_balance = self.balance(&transfer.to);
        let to_after = checked_destination_credit(to_balance, amount)?;
        let from_after = checked_source_debit(available, amount)?;

        self.set_balance(transfer.from.clone(), from_after)?;
        self.set_balance(transfer.to.clone(), to_after)?;
        self.transfers.push(transfer);
        Ok(())
    }

    pub(crate) fn transfer_external_qi_to_ledger(
        &mut self,
        from: QiAccountId,
        to: QiAccountId,
        amount: f64,
        reason: QiTransferReason,
    ) -> Result<Option<QiTransfer>, QiPhysicsError> {
        let amount = finite_non_negative(amount, "transfer.amount")?;
        reject_audit_only_qi_reason(reason)?;
        if amount == 0.0 {
            return Ok(None);
        }
        let transfer = QiTransfer::new(from.clone(), to, amount, reason)?;
        if transfer.from == transfer.to {
            return Err(QiPhysicsError::SameAccountTransfer {
                account: from.to_string(),
            });
        }

        // 外部 source 只在事务 overlay 里临时镜像；事务成功提交时恢复原状，原先不存在
        // 的 source 则以删除标记结束，不会污染长期账本。
        checked_destination_credit(self.balance(&transfer.to), amount)?;
        let source_existed = self.has_account(&from);
        let source_before = self.balance(&from);
        let source_shadow = finite_non_negative(source_before + amount, "source_shadow_balance")?;
        if source_shadow == source_before {
            return Err(QiPhysicsError::UnrepresentableChange {
                field: "source_shadow_balance",
                before: source_before,
                amount,
            });
        }
        self.set_balance(from.clone(), source_shadow)?;

        let result = self.transfer(transfer.clone());
        if source_existed {
            self.set_balance(from, source_before)?;
        } else {
            self.remove_balance(&from);
        }

        result.map(|()| Some(transfer))
    }
}

impl WorldQiAccount {
    /// 在不复制既有审计历史的前提下执行失败原子的账本操作。
    pub(crate) fn with_transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut WorldQiAccountTransaction<'_>) -> Result<T, QiPhysicsError>,
    ) -> Result<T, QiPhysicsError> {
        let mut transaction = WorldQiAccountTransaction {
            base_balances: &self.balances,
            changes: BTreeMap::new(),
            transfers: Vec::new(),
        };
        let value = operation(&mut transaction)?;

        let changes = std::mem::take(&mut transaction.changes);
        let transfers = std::mem::take(&mut transaction.transfers);
        drop(transaction);

        for (account, balance) in changes {
            match balance {
                Some(balance) => {
                    self.balances.insert(account, balance);
                }
                None => {
                    self.balances.remove(&account);
                }
            }
        }
        self.transfers.extend(transfers);
        Ok(value)
    }

    pub fn set_balance(&mut self, account: QiAccountId, amount: f64) -> Result<(), QiPhysicsError> {
        let amount = finite_non_negative(amount, "balance")?;
        self.balances.insert(account, amount);
        Ok(())
    }

    pub fn remove_balance(&mut self, account: &QiAccountId) -> Option<f64> {
        self.balances.remove(account)
    }

    pub fn has_account(&self, account: &QiAccountId) -> bool {
        self.balances.contains_key(account)
    }

    pub fn balance(&self, account: &QiAccountId) -> f64 {
        self.balances.get(account).copied().unwrap_or(0.0)
    }

    pub fn transfer(&mut self, transfer: QiTransfer) -> Result<(), QiPhysicsError> {
        // plan-halfstep-buff-v1 P1：HalfStepBuff 是 audit-only 标记（容量扩张，非真元搬运），
        // 误调 transfer 会变动 balance，违反 doc-comment 语义 + worldview §二 守恒律。
        // 拒绝在入口，强制 caller 走 EventWriter<QiTransfer> 单纯 emit 路径。
        //
        // plan-qi-conservation-leaks-v1 P4 / bughunt r8 — DuguReturnToZone /
        // DuguReverseVictimQi 的 doc-comment 同样标注"audit-only，禁止调 transfer"
        // （余额已经在 ECS Cultivation 组件或 zone balance 里正确更新，调用方必须走
        // push_transfer_audit 单纯留痕）；NegPressureDrain（bughunt QS-01）注释里也写了
        // "活体真元仍在 ECS，不镜像到 player/npc ledger balance"。三者此前只在文档里
        // 约定，没有编译期/运行期防护——照搬 HalfStepBuff 先例把它们一并拒在入口。
        reject_audit_only_qi_reason(transfer.reason)?;

        let amount = finite_non_negative(transfer.amount, "transfer.amount")?;
        if transfer.from == transfer.to {
            return Err(QiPhysicsError::SameAccountTransfer {
                account: transfer.from.to_string(),
            });
        }
        let available = self.balance(&transfer.from);
        if amount > available {
            return Err(QiPhysicsError::InsufficientQi {
                account: transfer.from.to_string(),
                available,
                requested: amount,
            });
        }

        let to_balance = self.balance(&transfer.to);
        let to_after = checked_destination_credit(to_balance, amount)?;
        let from_after = checked_source_debit(available, amount)?;

        self.balances.insert(transfer.from.clone(), from_after);
        self.balances.insert(transfer.to.clone(), to_after);
        self.transfers.push(transfer);
        Ok(())
    }

    pub fn total(&self) -> f64 {
        self.balances.values().sum()
    }

    pub fn transfers(&self) -> &[QiTransfer] {
        &self.transfers
    }

    /// audit-only 记录：仅将 `transfer` 追加到审计轨迹，不修改任何账户余额。
    ///
    /// 用于「玩家真元存储在 Cultivation.qi_current（ECS 组件），不在此 ledger balances」
    /// 的跨账本转账场景（如 BossDrain）——余额已在外部正确更新，此处仅留轨迹。
    pub fn push_transfer_audit(&mut self, transfer: QiTransfer) {
        self.transfers.push(transfer);
    }

    /// plan-offscreen-war-v1 P0：守恒 telemetry 用——按 `QiAccountId` 升序（BTreeMap
    /// 天然有序）迭代每个账户的余额，供 `bong:qi/ledger` 把 per-zone / per-npc
    /// 账本暴露给外部脚本做精确守恒断言。只读，不改账本。
    pub fn iter_balances(&self) -> impl Iterator<Item = (&QiAccountId, f64)> {
        self.balances
            .iter()
            .map(|(account, balance)| (account, *balance))
    }
}

/// 将存放在 ECS / item 等外部物理权威中的真元，真实转入 [`WorldQiAccount`] 目标账户。
/// 外部源通常没有长期 ledger 镜像余额；本 helper 会：
/// 1. 保存 source 原有余额与账户存在状态；
/// 2. 临时把本次 `amount` 加到 source 影子余额；
/// 3. 调用 [`WorldQiAccount::transfer`] 完成目标余额增加与审计追加；
/// 4. 无论成功或失败，都把 source 恢复到调用前的精确状态。
///
/// Therefore the caller only commits the external field debit after this function succeeds; on
/// failure, external state and the ledger source can remain unchanged for retry. The amount and
/// reason are still validated for `amount == 0`, but a valid zero transfer is an explicit no-op
/// that creates no account and appends no audit.
pub fn transfer_external_qi_to_ledger(
    account: &mut WorldQiAccount,
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) -> Result<Option<QiTransfer>, QiPhysicsError> {
    account.with_transaction(|transaction| {
        transaction.transfer_external_qi_to_ledger(from, to, amount, reason)
    })
}

/// Atomically debit a stable ledger owner and credit the external signed Zone owner.
///
/// `requested` is a demand, not an already-accepted amount: this function computes Zone room
/// against `zone_ceiling` before any debit and transfers only the accepted amount. That keeps the
/// ceiling invariant inside the same failure-atomic boundary as the stable balance and audit, so
/// callers never need a post-commit clamp that could destroy qi.
pub fn transfer_ledger_qi_to_zone(
    account: &mut WorldQiAccount,
    from: QiAccountId,
    zone_name: &str,
    zone_spirit_qi: &mut f64,
    requested: f64,
    zone_ceiling: f64,
    reason: QiTransferReason,
) -> Result<Option<QiTransfer>, QiPhysicsError> {
    let requested = finite_non_negative(requested, "ledger_to_zone.requested")?;
    let zone_ceiling = finite_non_negative(zone_ceiling, "ledger_to_zone.zone_ceiling")?;
    reject_audit_only_qi_reason(reason)?;
    if !zone_spirit_qi.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "zone.spirit_qi",
            value: *zone_spirit_qi,
        });
    }

    let room_absolute =
        ((zone_ceiling - *zone_spirit_qi).max(0.0) * QI_ZONE_UNIT_CAPACITY).min(f64::MAX);
    let accepted = requested.min(room_absolute);
    if accepted == 0.0 {
        return Ok(None);
    }
    let available = account.balance(&from);
    if accepted > available {
        return Err(QiPhysicsError::InsufficientQi {
            account: from.to_string(),
            available,
            requested: accepted,
        });
    }
    let zone_after = if accepted == room_absolute {
        zone_ceiling
    } else {
        *zone_spirit_qi + accepted / QI_ZONE_UNIT_CAPACITY
    };
    if !zone_after.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "zone.spirit_qi_after",
            value: zone_after,
        });
    }
    let to = QiAccountId::zone(zone_name);
    if from == to {
        return Err(QiPhysicsError::SameAccountTransfer {
            account: from.to_string(),
        });
    }
    let transfer = QiTransfer::new(from.clone(), to, accepted, reason)?;

    let source_after = if accepted == available {
        0.0
    } else {
        available - accepted
    };
    if source_after == available {
        return Err(QiPhysicsError::UnrepresentableChange {
            field: "source_balance",
            before: available,
            amount: accepted,
        });
    }
    if zone_after == *zone_spirit_qi {
        return Err(QiPhysicsError::UnrepresentableChange {
            field: "zone.spirit_qi",
            before: *zone_spirit_qi,
            amount: accepted / QI_ZONE_UNIT_CAPACITY,
        });
    }

    account.set_balance(from, source_after)?;
    *zone_spirit_qi = zone_after;
    account.push_transfer_audit(transfer.clone());
    Ok(Some(transfer))
}

/// Atomically debit the external signed Zone owner and credit a stable ledger owner.
///
/// The Zone must hold the complete requested amount in its positive balance. Negative pressure is
/// never flattened or used as a source. The stable credit commits first; only a successful credit
/// is followed by the preflighted Zone field update, so destination overflow leaves both owners and
/// the audit unchanged.
pub fn transfer_zone_qi_to_ledger(
    account: &mut WorldQiAccount,
    zone_name: &str,
    zone_spirit_qi: &mut f64,
    to: QiAccountId,
    requested: f64,
    reason: QiTransferReason,
) -> Result<Option<QiTransfer>, QiPhysicsError> {
    let requested = finite_non_negative(requested, "zone_to_ledger.requested")?;
    reject_audit_only_qi_reason(reason)?;
    if !zone_spirit_qi.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "zone.spirit_qi",
            value: *zone_spirit_qi,
        });
    }

    let available = finite_non_negative(
        (*zone_spirit_qi).max(0.0) * QI_ZONE_UNIT_CAPACITY,
        "zone_to_ledger.available",
    )?;
    if requested > available {
        return Err(QiPhysicsError::InsufficientQi {
            account: QiAccountId::zone(zone_name).to_string(),
            available,
            requested,
        });
    }
    if requested == 0.0 {
        return Ok(None);
    }

    let zone_after = if requested == available {
        0.0
    } else {
        *zone_spirit_qi - requested / QI_ZONE_UNIT_CAPACITY
    };
    if !zone_after.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "zone.spirit_qi_after",
            value: zone_after,
        });
    }
    if zone_after == *zone_spirit_qi {
        return Err(QiPhysicsError::UnrepresentableChange {
            field: "zone.spirit_qi",
            before: *zone_spirit_qi,
            amount: requested / QI_ZONE_UNIT_CAPACITY,
        });
    }
    let from = QiAccountId::zone(zone_name);
    let transfer = transfer_external_qi_to_ledger(account, from, to, requested, reason)?;
    *zone_spirit_qi = zone_after;
    Ok(transfer)
}

/// plan-zone-qi-economy-v1 P0 §8.1 决议 #1 — 独立"待分配池"账户 id。
///
/// 开脉 / 突破消耗真元回充的目标**不是** `zone:<name>` 账户，也**不是**
/// `WorldQiBudget.current_total`：
///   - 不选 `zone:<name>`：signed 区域灵压由 `Zone.spirit_qi` 唯一持有；长期 ledger mirror
///     会被 `summarize_world_qi` 重复计入，并允许字段与 shadow 漂移。
///   - 不选 `WorldQiBudget.current_total`：那是 `compute_void_quota_limit`
///     （`cultivation::tribulation`）的化虚名额闸门基准，注入会让名额随修炼活跃度
///     膨胀、可被玩家刷高，破坏 void-quota 稀缺性（用户 2026-07-03 拍板红线）。
///
/// 待分配池是全服单例（不按 zone 拆分），P1 heartbeat 回流 system 会按各 zone 的
/// `qi_equilibrium` 配置从这一个账户滴灌进 `zone.spirit_qi`。
pub const PENDING_INFLOW_ACCOUNT_ID: &str = "pending_inflow";
/// R5 P0 — 所有无法定位 zone、zone 已满或 signed zone 上界不允许接收的活体真元，
/// 真实转入此稳定聚合池。固定 id 可由 `qi_runtime_accounts` 完整枚举和跨重启恢复；
/// 禁止退回 `overflow:<entity>` 动态 event-only id，否则事件发出后余额仍会蒸发。
pub const QI_FLOW_OVERFLOW_ACCOUNT_ID: &str = "qi_flow_overflow";
/// 垂死大能给丹超过 150% cap 后的稳定聚合池。不得含 entity id，否则重启后无法枚举恢复。
pub const DYING_ELDER_DAN_EXCESS_ACCOUNT_ID: &str = "dying_elder_dan_excess";
/// 垂死大能死亡时 zone 无法接收部分的稳定聚合池。
pub const DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID: &str = "dying_elder_release";
/// 坍缩渊与负压 drain 的稳定真元池。该余额无 ECS 字段承载，必须跨重启恢复。
pub const RIFT_DRAIN_ACCOUNT_ID: &str = "rift_drain";

/// 没有 ECS/zone 字段承载、必须经 `qi_runtime_accounts` 持久化的完整白名单。
pub const PERSISTENT_RUNTIME_QI_ACCOUNT_IDS: [&str; 5] = [
    PENDING_INFLOW_ACCOUNT_ID,
    QI_FLOW_OVERFLOW_ACCOUNT_ID,
    DYING_ELDER_DAN_EXCESS_ACCOUNT_ID,
    DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID,
    RIFT_DRAIN_ACCOUNT_ID,
];

/// 独立待分配池账户（`QiAccountKind::Overflow` + 固定 id，见 [`PENDING_INFLOW_ACCOUNT_ID`]）。
pub fn pending_inflow_account() -> QiAccountId {
    QiAccountId::overflow(PENDING_INFLOW_ACCOUNT_ID)
}

pub fn qi_flow_overflow_account() -> QiAccountId {
    QiAccountId::overflow(QI_FLOW_OVERFLOW_ACCOUNT_ID)
}

pub fn dying_elder_dan_excess_account() -> QiAccountId {
    QiAccountId::overflow(DYING_ELDER_DAN_EXCESS_ACCOUNT_ID)
}

pub fn dying_elder_release_overflow_account() -> QiAccountId {
    QiAccountId::overflow(DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID)
}

pub fn rift_drain_account() -> QiAccountId {
    QiAccountId::rift(RIFT_DRAIN_ACCOUNT_ID)
}

pub fn persistent_runtime_qi_accounts() -> [QiAccountId; 5] {
    [
        pending_inflow_account(),
        qi_flow_overflow_account(),
        dying_elder_dan_excess_account(),
        dying_elder_release_overflow_account(),
        rift_drain_account(),
    ]
}

/// plan-zone-qi-economy-v1 P0 §8.1 决议 #1 — 消耗（开脉 / 突破）真元回充独立待分配池。
///
/// 记账范本照抄 `npc::dormant::apply_dormant_regen_with_multiplier`（双账本严格同步：
/// `set_balance` + `transfer` + 真实字段变更），**不照抄**旧 `credit_meridian_open_cost`
/// "只手写 `set_balance` 叠加、绕开 `transfer()` insufficient 检查与审计"的写法
/// （那正是记账蒸发 bug 本身）。
///
/// 玩家 / NPC 侧真实真元活在 ECS `Cultivation.qi_current`（调用方已在此之前完成扣减），
/// 此 ledger 上的 `from` 账户对 `MeridianOpen` / `Breakthrough` 这类 reason 而言不长期
/// 持有余额——这里把它的 ledger 影子余额临时"引燃"成本次转移额，使
/// [`WorldQiAccount::transfer`] 的原子记账（insufficient 检查 + from/to 同步扣加 +
/// 审计追加）可以照常生效，而不是绕开它手写 `set_balance`。转移后 `from` 侧恢复到
/// 调用前的精确状态；原先不存在的临时账户会被移除，不留残留、不跨 tick 累积。
///
/// `amount == 0.0` 是显式 no-op（不创建待分配池账户、不追加审计）；`amount < 0.0` /
/// 非有限值经由 [`QiTransfer::new`] 的 `finite_non_negative` 校验拒绝
/// （错误 `field` 固定为 `"transfer.amount"`，供上游按 field 精确匹配）。
///
/// `zone_name` 仅用于失败诊断（待分配池是全服单例，与具体 zone 无关）；供
/// `practice_session_tick`（§8.1 决议 #6，独立守恒待办）未来复用同一签名。
pub fn credit_pending_inflow(
    account: &mut WorldQiAccount,
    zone_name: &str,
    from: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) -> Result<(), QiPhysicsError> {
    let to = pending_inflow_account();
    if let Err(error) = transfer_external_qi_to_ledger(account, from, to, amount, reason) {
        tracing::warn!(
            "[bong][qi_physics] credit_pending_inflow failed zone={} amount={} reason={:?} error={:?}",
            zone_name,
            amount,
            reason,
            error
        );
        return Err(error);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldQiSnapshot {
    pub player_qi: f64,
    pub zone_qi: f64,
    pub container_qi: f64,
    pub ledger_qi: f64,
    pub era_decay_accum: f64,
    pub budget_initial_total: f64,
    pub budget_current_total: f64,
}

impl WorldQiSnapshot {
    pub fn total_observed(self) -> f64 {
        self.player_qi + self.zone_qi + self.container_qi + self.ledger_qi
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QiPhysicsIpcSnapshot {
    pub observed_total: f64,
    pub budget_current_total: f64,
    pub era_decay_accum: f64,
}

pub fn snapshot_for_ipc(snapshot: &WorldQiSnapshot) -> QiPhysicsIpcSnapshot {
    QiPhysicsIpcSnapshot {
        observed_total: snapshot.total_observed(),
        budget_current_total: snapshot.budget_current_total,
        era_decay_accum: snapshot.era_decay_accum,
    }
}

/// plan-offscreen-war-v1 P0：`bong:qi/ledger` HASH 字段前缀——per-account 余额行。
/// 字段形如 `account:zone:spawn`（kind 为 `QiAccountKind::as_wire_str` 稳定 lowercase 串），
/// 值是该账户当前 balance（字符串化 f64）。
pub const QI_LEDGER_ACCOUNT_FIELD_PREFIX: &str = "account:";

/// plan-offscreen-war-v1 P0：把全服守恒快照 + ledger 各账户余额拍平成
/// `bong:qi/ledger` HASH 的 (field, value) 列表，供外部脚本做**精确**守恒断言。
///
/// 顶层聚合字段：
/// - `total_observed`：player+zone+container+ledger 的**已落位**真元（≤ 预算；minimal
///   世界起服后 zone qi 很低，远小于预算，勿误当 == DEFAULT_SPIRIT_QI_TOTAL）；
/// - `player_qi` / `zone_qi` / `container_qi` / `ledger_qi`：已落位分量明细；
/// - `budget_initial_total` / `budget_current_total` / `era_decay_accum`：天道预算（守恒总量
///   恒定的真锚点 = `DEFAULT_SPIRIT_QI_TOTAL`，仅被时代衰减拉低）与已累计衰减。
///
/// per-account 字段：每个被 ledger 记账过的账户一行 `account:<id>` → balance。
///
/// 纯函数（只读快照 + 账本），无副作用，方便单测精确锁字段。
pub fn build_qi_ledger_hash_fields(
    snapshot: &WorldQiSnapshot,
    accounts: &WorldQiAccount,
) -> Vec<(String, String)> {
    let mut fields = vec![
        (
            "total_observed".to_string(),
            snapshot.total_observed().to_string(),
        ),
        ("player_qi".to_string(), snapshot.player_qi.to_string()),
        ("zone_qi".to_string(), snapshot.zone_qi.to_string()),
        (
            "container_qi".to_string(),
            snapshot.container_qi.to_string(),
        ),
        ("ledger_qi".to_string(), snapshot.ledger_qi.to_string()),
        (
            "budget_initial_total".to_string(),
            snapshot.budget_initial_total.to_string(),
        ),
        (
            "budget_current_total".to_string(),
            snapshot.budget_current_total.to_string(),
        ),
        (
            "era_decay_accum".to_string(),
            snapshot.era_decay_accum.to_string(),
        ),
    ];
    // BTreeMap 有序迭代 → per-account 字段确定性排序，外部脚本可稳定 diff。
    for (account, balance) in accounts.iter_balances() {
        fields.push((
            format!("{QI_LEDGER_ACCOUNT_FIELD_PREFIX}{account}"),
            balance.to_string(),
        ));
    }
    fields
}

pub fn summarize_world_qi(world: &mut bevy_ecs::world::World) -> WorldQiSnapshot {
    let budget = world
        .get_resource::<WorldQiBudget>()
        .copied()
        .unwrap_or_default();

    let zone_qi = world
        .get_resource::<ZoneRegistry>()
        .map(|zones| {
            zones
                .zones
                .iter()
                .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
                .sum()
        })
        .unwrap_or(0.0);

    let player_qi = {
        let mut query = world.query::<&Cultivation>();
        query.iter(world).map(|cult| cult.qi_current.max(0.0)).sum()
    };

    let container_qi = {
        let mut query = world.query::<&PlayerInventory>();
        query.iter(world).map(inventory_qi).sum()
    };

    let ledger_qi = world
        .get_resource::<WorldQiAccount>()
        .map(WorldQiAccount::total)
        .unwrap_or(0.0);

    WorldQiSnapshot {
        player_qi,
        zone_qi,
        container_qi,
        ledger_qi,
        era_decay_accum: budget.era_decay_accum,
        budget_initial_total: budget.initial_total,
        budget_current_total: budget.current_total,
    }
}

fn inventory_qi(inventory: &PlayerInventory) -> f64 {
    let containers = inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .map(|placed| item_qi(&placed.instance))
        .sum::<f64>();
    let equipped = inventory
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .map(item_qi)
        .sum::<f64>();
    let hotbar = inventory
        .hotbar
        .iter()
        .filter_map(|item| item.as_ref())
        .map(item_qi)
        .sum::<f64>();

    containers + equipped + hotbar
}

fn item_qi(item: &ItemInstance) -> f64 {
    item.spirit_quality.clamp(0.0, 1.0) * item.stack_count.max(1) as f64
}

pub fn assert_conservation(
    before: &WorldQiSnapshot,
    after: &WorldQiSnapshot,
    era_decay: f64,
) -> Result<(), QiPhysicsError> {
    let era_decay = finite_non_negative(era_decay, "era_decay")?;
    let expected = before.total_observed() - era_decay;
    let actual = after.total_observed();
    let tolerance = QI_EPSILON.max(expected.abs() * 1e-9);
    if (expected - actual).abs() <= tolerance {
        Ok(())
    } else {
        Err(QiPhysicsError::ConservationDrift {
            expected,
            actual,
            tolerance,
        })
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
