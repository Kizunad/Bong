use std::collections::BTreeMap;

use valence::prelude::{bevy_ecs, Event, Resource};

use crate::cultivation::components::Cultivation;
use crate::inventory::{ItemInstance, PlayerInventory};
use crate::world::zone::ZoneRegistry;

use super::constants::{DEFAULT_SPIRIT_QI_TOTAL, QI_EPSILON};
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
    RiftCollapse,
    EraDecay,
    /// plan-craft-v1 §0/§3 — 手搓 qi_cost 一次性投入 zone，区别于 ReleaseToZone（招式释放）
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
}

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

impl WorldQiAccount {
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
        if matches!(transfer.reason, QiTransferReason::HalfStepBuff) {
            return Err(QiPhysicsError::AuditOnlyReason {
                reason: "HalfStepBuff",
            });
        }

        let amount = finite_non_negative(transfer.amount, "transfer.amount")?;
        let available = self.balance(&transfer.from);
        if amount > available {
            return Err(QiPhysicsError::InsufficientQi {
                account: transfer.from.to_string(),
                available,
                requested: amount,
            });
        }

        self.balances
            .insert(transfer.from.clone(), (available - amount).max(0.0));
        let to_balance = self.balance(&transfer.to);
        self.balances
            .insert(transfer.to.clone(), to_balance + amount);
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
        .map(|zones| zones.zones.iter().map(|zone| zone.spirit_qi).sum())
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
    let equipped = inventory.equipped.values().map(item_qi).sum::<f64>();
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
mod tests {
    use std::collections::HashMap;

    use valence::prelude::App;

    use crate::cultivation::components::Cultivation;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use crate::world::zone::ZoneRegistry;

    use super::*;

    #[test]
    fn budget_defaults_to_config_default() {
        let budget = WorldQiBudget::default();
        assert_eq!(budget.initial_total, DEFAULT_SPIRIT_QI_TOTAL);
        assert_eq!(budget.current_total, DEFAULT_SPIRIT_QI_TOTAL);
    }

    #[test]
    fn budget_rejects_invalid_total_to_default() {
        let budget = WorldQiBudget::from_total(-1.0);
        assert_eq!(budget.current_total, DEFAULT_SPIRIT_QI_TOTAL);
    }

    #[test]
    fn budget_era_decay_updates_current_total() {
        let mut budget = WorldQiBudget::from_total(100.0);
        let decay = budget.apply_era_decay(0.02).expect("valid decay");
        assert_eq!(decay, 2.0);
        assert_eq!(budget.current_total, 98.0);
        assert_eq!(budget.era_decay_accum, 2.0);
    }

    #[test]
    fn transfer_moves_qi_between_accounts() {
        let from = QiAccountId::player("a");
        let to = QiAccountId::zone("spawn");
        let mut account = WorldQiAccount::default();
        account.set_balance(from.clone(), 10.0).unwrap();
        account.set_balance(to.clone(), 1.0).unwrap();
        account
            .transfer(
                QiTransfer::new(
                    from.clone(),
                    to.clone(),
                    3.0,
                    QiTransferReason::ReleaseToZone,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(account.balance(&from), 7.0);
        assert_eq!(account.balance(&to), 4.0);
    }

    #[test]
    fn transfer_rejects_overdraft() {
        let from = QiAccountId::player("a");
        let to = QiAccountId::zone("spawn");
        let mut account = WorldQiAccount::default();
        account.set_balance(from.clone(), 1.0).unwrap();
        let err = account
            .transfer(QiTransfer::new(from, to, 3.0, QiTransferReason::ReleaseToZone).unwrap())
            .expect_err("overdraft should fail");
        assert!(matches!(err, QiPhysicsError::InsufficientQi { .. }));
    }

    #[test]
    fn transfer_rejects_halfstep_buff_audit_only_reason() {
        // plan-halfstep-buff-v1 P1：HalfStepBuff 是 audit-only 标记，绝不可走变动 balance 的 transfer 路径。
        // 调用方应直接 emit `EventWriter<QiTransfer>` event，不走 WorldQiAccount::transfer。
        let from = QiAccountId::tiandao();
        let to = QiAccountId::player("alice");
        let mut account = WorldQiAccount::default();
        account.set_balance(from.clone(), 100.0).unwrap();
        let initial_from_balance = account.balance(&from);
        let initial_to_balance = account.balance(&to);
        let transfer = QiTransfer::new(
            from.clone(),
            to.clone(),
            10.0,
            QiTransferReason::HalfStepBuff,
        )
        .expect("QiTransfer::new must accept HalfStepBuff reason (event-level allowed)");
        let err = account.transfer(transfer).expect_err(
            "WorldQiAccount::transfer 必须拒绝 HalfStepBuff reason；audit-only 误用会破守恒律",
        );
        assert!(
            matches!(err, QiPhysicsError::AuditOnlyReason { reason } if reason == "HalfStepBuff"),
            "expected AuditOnlyReason::HalfStepBuff, got {err:?}"
        );
        // balance 必须未变动
        assert_eq!(
            account.balance(&from),
            initial_from_balance,
            "拒绝后 from balance 必须保持不变"
        );
        assert_eq!(
            account.balance(&to),
            initial_to_balance,
            "拒绝后 to balance 必须保持不变"
        );
        // transfers 记录也不应增加
        assert!(
            account.transfers().is_empty(),
            "拒绝的 transfer 不应留下 audit trail；防止统计被误污染"
        );
    }

    #[test]
    fn transfer_rejects_epsilon_sized_overdraft() {
        let from = QiAccountId::player("a");
        let to = QiAccountId::zone("spawn");
        let mut account = WorldQiAccount::default();
        account.set_balance(from.clone(), 0.0).unwrap();
        account.set_balance(to.clone(), 0.0).unwrap();

        let err = account
            .transfer(
                QiTransfer::new(from, to, QI_EPSILON * 0.5, QiTransferReason::ReleaseToZone)
                    .unwrap(),
            )
            .expect_err("tiny positive overdraft should fail");
        assert!(matches!(err, QiPhysicsError::InsufficientQi { .. }));
    }

    #[test]
    fn repeated_transfers_preserve_total() {
        let from = QiAccountId::player("a");
        let to = QiAccountId::zone("spawn");
        let mut account = WorldQiAccount::default();
        account.set_balance(from.clone(), 100.0).unwrap();
        account.set_balance(to.clone(), 0.0).unwrap();
        for _ in 0..100 {
            account
                .transfer(
                    QiTransfer::new(from.clone(), to.clone(), 0.5, QiTransferReason::Channeling)
                        .unwrap(),
                )
                .unwrap();
        }
        assert!((account.total() - 100.0).abs() < QI_EPSILON);
    }

    #[test]
    fn conservation_accepts_era_decay() {
        let before = snapshot(100.0);
        let after = snapshot(97.0);
        assert!(assert_conservation(&before, &after, 3.0).is_ok());
    }

    #[test]
    fn conservation_rejects_drift() {
        let before = snapshot(100.0);
        let after = snapshot(90.0);
        let err = assert_conservation(&before, &after, 3.0).expect_err("drift should fail");
        assert!(matches!(err, QiPhysicsError::ConservationDrift { .. }));
    }

    #[test]
    fn conservation_accepts_preserved_negative_observed_total() {
        let before = snapshot(-0.6);
        let after = snapshot(-0.6);
        assert!(assert_conservation(&before, &after, 0.0).is_ok());
    }

    #[test]
    fn snapshot_for_ipc_keeps_budget_and_observed_total() {
        let snap = WorldQiSnapshot {
            player_qi: 1.0,
            zone_qi: 2.0,
            container_qi: 3.0,
            ledger_qi: 4.0,
            era_decay_accum: 5.0,
            budget_initial_total: 100.0,
            budget_current_total: 95.0,
        };
        let ipc = snapshot_for_ipc(&snap);
        assert_eq!(ipc.observed_total, 10.0);
        assert_eq!(ipc.budget_current_total, 95.0);
        assert_eq!(ipc.era_decay_accum, 5.0);
    }

    #[test]
    fn summarize_world_qi_reads_budget_zones_players_and_inventory() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(50.0));
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.5;
        app.insert_resource(zones);
        app.world_mut().spawn(Cultivation {
            qi_current: 7.0,
            ..Default::default()
        });
        app.world_mut().spawn(inventory_with_item(0.8, 2));

        let snap = summarize_world_qi(app.world_mut());
        assert_eq!(snap.budget_current_total, 50.0);
        assert_eq!(snap.zone_qi, 0.5);
        assert_eq!(snap.player_qi, 7.0);
        assert_eq!(snap.container_qi, 1.6);
    }

    #[test]
    fn summarize_world_qi_preserves_negative_zone_qi() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = -0.6;
        app.insert_resource(zones);

        let snap = summarize_world_qi(app.world_mut());
        assert_eq!(snap.zone_qi, -0.6);
        assert_eq!(snap.total_observed(), -0.6);
    }

    #[test]
    fn world_qi_snapshot_asserts_conservation_across_ledger_transfer() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(100.0));
        let mut account = WorldQiAccount::default();
        let from = QiAccountId::player("p1");
        let to = QiAccountId::zone("spawn");
        account.set_balance(from.clone(), 10.0).unwrap();
        account.set_balance(to.clone(), 5.0).unwrap();
        app.insert_resource(account);
        let before = summarize_world_qi(app.world_mut());

        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .transfer(QiTransfer::new(from, to, 3.0, QiTransferReason::ReleaseToZone).unwrap())
            .unwrap();
        let after = summarize_world_qi(app.world_mut());

        assert_eq!(before.budget_current_total, 100.0);
        assert_eq!(after.budget_current_total, 100.0);
        assert!(assert_conservation(&before, &after, 0.0).is_ok());
    }

    // ── plan-offscreen-war-v1 P0：bong:qi/ledger telemetry builder ────────

    fn field<'a>(fields: &'a [(String, String)], name: &str) -> &'a str {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or_else(|| {
                panic!("expected ledger HASH to carry field {name:?}, got {fields:?}")
            })
    }

    #[test]
    fn qi_ledger_hash_fields_carry_conservation_aggregates() {
        // 顶层守恒分量必须可被外部脚本精确读出：total_observed == player+zone+container+ledger。
        let snap = WorldQiSnapshot {
            player_qi: 40.0,
            zone_qi: 35.0,
            container_qi: 15.0,
            ledger_qi: 10.0,
            era_decay_accum: 0.0,
            budget_initial_total: DEFAULT_SPIRIT_QI_TOTAL,
            budget_current_total: DEFAULT_SPIRIT_QI_TOTAL,
        };
        let accounts = WorldQiAccount::default();
        let fields = build_qi_ledger_hash_fields(&snap, &accounts);

        assert_eq!(
            field(&fields, "total_observed"),
            "100",
            "total_observed 必须等于四个分量之和（40+35+15+10），外部 e2e 据此做精确守恒断言"
        );
        assert_eq!(field(&fields, "player_qi"), "40");
        assert_eq!(field(&fields, "zone_qi"), "35");
        assert_eq!(field(&fields, "container_qi"), "15");
        assert_eq!(field(&fields, "ledger_qi"), "10");
        assert_eq!(
            field(&fields, "budget_initial_total"),
            DEFAULT_SPIRIT_QI_TOTAL.to_string(),
            "预算字段取 const，不写字面 100"
        );
        assert_eq!(
            field(&fields, "budget_current_total"),
            DEFAULT_SPIRIT_QI_TOTAL.to_string()
        );
        assert_eq!(field(&fields, "era_decay_accum"), "0");
    }

    #[test]
    fn qi_ledger_hash_fields_emit_one_row_per_account() {
        // per-zone / per-npc 账户余额各占一行 account:<id>，外部脚本可逐账户对账。
        let mut accounts = WorldQiAccount::default();
        accounts
            .set_balance(QiAccountId::zone("spawn"), 25.0)
            .unwrap();
        accounts
            .set_balance(QiAccountId::npc("dormant:rogue:7"), 3.5)
            .unwrap();
        let fields = build_qi_ledger_hash_fields(&snapshot(0.0), &accounts);

        assert_eq!(
            field(&fields, "account:zone:spawn"),
            "25",
            "zone 账户余额必须以 account:zone:spawn 行暴露（kind 为稳定 lowercase wire 串）"
        );
        assert_eq!(
            field(&fields, "account:npc:dormant:rogue:7"),
            "3.5",
            "npc 账户余额必须以 account:npc:<char_id> 行暴露，供 P2 战死还灵气对账"
        );

        let account_rows = fields
            .iter()
            .filter(|(key, _)| key.starts_with(QI_LEDGER_ACCOUNT_FIELD_PREFIX))
            .count();
        assert_eq!(
            account_rows, 2,
            "两个账户应产出两行 account: 字段，不多不少"
        );
    }

    #[test]
    fn qi_account_id_display_uses_stable_lowercase_wire_strings() {
        // wire 契约 pin：`bong:qi/ledger` 的 `account:<kind>:<id>` key 形如 `zone:spawn`。
        // 任一变体的 Display 串改动都会撞红——防止 Debug rename 静默漂移外部 schema。
        let cases: [(QiAccountId, &str); 7] = [
            (QiAccountId::player("alice"), "player:alice"),
            (QiAccountId::npc("dormant:rogue:7"), "npc:dormant:rogue:7"),
            (QiAccountId::zone("spawn"), "zone:spawn"),
            (QiAccountId::container("chest:3"), "container:chest:3"),
            (QiAccountId::rift("rift:north"), "rift:rift:north"),
            (QiAccountId::tiandao(), "tiandao:tiandao"),
            (QiAccountId::overflow("sink"), "overflow:sink"),
        ];
        for (account, expected) in cases {
            assert_eq!(
                account.to_string(),
                expected,
                "QiAccountId Display 必须输出稳定 lowercase wire 串 `{expected}`，\
                 因为外部 Redis schema 据此 diff；若此处红了说明改动了 wire 契约（须有意为之）"
            );
        }
    }

    #[test]
    fn qi_ledger_hash_fields_empty_ledger_has_only_aggregates() {
        // 起服后 ledger 尚未记账：无 account: 行，但 total_observed 已可读（来自 zone/player 快照）。
        let fields = build_qi_ledger_hash_fields(&snapshot(100.0), &WorldQiAccount::default());
        assert!(
            fields
                .iter()
                .all(|(key, _)| !key.starts_with(QI_LEDGER_ACCOUNT_FIELD_PREFIX)),
            "空账本不应有 account: 行"
        );
        assert_eq!(
            field(&fields, "total_observed"),
            "100",
            "空账本下 total_observed 仍来自 player/zone 快照，起服后应 ≈ DEFAULT_SPIRIT_QI_TOTAL"
        );
    }

    fn snapshot(total: f64) -> WorldQiSnapshot {
        WorldQiSnapshot {
            player_qi: total,
            zone_qi: 0.0,
            container_qi: 0.0,
            ledger_qi: 0.0,
            era_decay_accum: 0.0,
            budget_initial_total: total,
            budget_current_total: total,
        }
    }

    fn inventory_with_item(spirit_quality: f64, stack_count: u32) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "main".to_string(),
                rows: 1,
                cols: 1,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: ItemInstance {
                        instance_id: 1,
                        template_id: "bone_coin".to_string(),
                        display_name: "bone coin".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 1.0,
                        rarity: ItemRarity::Common,
                        description: String::new(),
                        stack_count,
                        spirit_quality,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    // ── plan-qi-handling-attrition-v1 P0 — AttritionTax 变体 pin 测试 ───────

    #[test]
    fn attrition_tax_variant_neq_half_step_buff() {
        // AttritionTax 与 HalfStepBuff 是独立变体，语义不同（税 vs 容量扩张）
        let attrition = QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::Pickup,
        };
        let halfstep = QiTransferReason::HalfStepBuff;
        assert_ne!(
            attrition, halfstep,
            "AttritionTax 与 HalfStepBuff 应为不同变体"
        );
    }

    #[test]
    fn attrition_tax_variant_neq_release_to_zone() {
        // AttritionTax 与 ReleaseToZone 是独立变体，审计轨迹不应混淆
        let attrition = QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::Pickup,
        };
        let release = QiTransferReason::ReleaseToZone;
        assert_ne!(
            attrition, release,
            "AttritionTax 与 ReleaseToZone 应为不同变体，否则审计轨迹混淆"
        );
    }

    #[test]
    fn attrition_tax_op_kind_all_five_distinct() {
        // 5 个 AttritionOpKind 变体应各自独立，pin 其相等性
        use AttritionOpKind::*;
        let variants = [Pickup, SlotMove, ContainerSearch, ForgeLoad, AlchemyLoad];
        for i in 0..variants.len() {
            for j in 0..variants.len() {
                if i == j {
                    assert_eq!(variants[i], variants[j], "同一变体应相等");
                } else {
                    assert_ne!(
                        variants[i], variants[j],
                        "{:?} 与 {:?} 应为不同变体",
                        variants[i], variants[j]
                    );
                }
            }
        }
    }

    #[test]
    fn attrition_tax_is_not_audit_only_in_transfer() {
        // AttritionTax 不是 audit-only，应可以走 emit QiTransfer::new（不被拒绝）
        let from = QiAccountId::container("item:123");
        let to = QiAccountId::zone("spawn");
        let result = QiTransfer::new(
            from,
            to,
            3.0,
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::Pickup,
            },
        );
        assert!(
            result.is_ok(),
            "AttritionTax QiTransfer::new 不应失败，实际 {:?}",
            result.err()
        );
    }

    // ── plan-qi-conservation-leaks-v1 P2 — ArtifactMaintenance/ArtifactEvolution pin 测试 ──

    #[test]
    fn artifact_maintenance_reason_is_distinct_from_other_variants() {
        // pin 测试：ArtifactMaintenance 与其他变体互不相等，审计轨迹不混淆。
        let m = QiTransferReason::ArtifactMaintenance;
        let e = QiTransferReason::ArtifactEvolution;
        let c = QiTransferReason::Crafting;
        let b = QiTransferReason::BossDrain;
        let r = QiTransferReason::ReleaseToZone;

        assert_ne!(
            m, e,
            "ArtifactMaintenance 与 ArtifactEvolution 应为不同变体"
        );
        assert_ne!(m, c, "ArtifactMaintenance 与 Crafting 应为不同变体");
        assert_ne!(m, b, "ArtifactMaintenance 与 BossDrain 应为不同变体");
        assert_ne!(m, r, "ArtifactMaintenance 与 ReleaseToZone 应为不同变体");
        assert_eq!(m, m, "ArtifactMaintenance 与自身应相等");
    }

    #[test]
    fn artifact_evolution_reason_is_distinct_from_other_variants() {
        // pin 测试：ArtifactEvolution 与其他变体互不相等，审计轨迹不混淆。
        let e = QiTransferReason::ArtifactEvolution;
        let m = QiTransferReason::ArtifactMaintenance;
        let c = QiTransferReason::Crafting;
        let b = QiTransferReason::BossDrain;
        let r = QiTransferReason::ReleaseToZone;

        assert_ne!(
            e, m,
            "ArtifactEvolution 与 ArtifactMaintenance 应为不同变体"
        );
        assert_ne!(e, c, "ArtifactEvolution 与 Crafting 应为不同变体");
        assert_ne!(e, b, "ArtifactEvolution 与 BossDrain 应为不同变体");
        assert_ne!(e, r, "ArtifactEvolution 与 ReleaseToZone 应为不同变体");
        assert_eq!(e, e, "ArtifactEvolution 与自身应相等");
    }

    #[test]
    fn artifact_maintenance_and_evolution_not_audit_only() {
        // 两个新变体均可通过 QiTransfer::new 构建（非 audit-only）。
        let from = QiAccountId::player("entity:1");
        let to = QiAccountId::zone("spawn");

        let maint = QiTransfer::new(
            from.clone(),
            to.clone(),
            2.0,
            QiTransferReason::ArtifactMaintenance,
        );
        assert!(
            maint.is_ok(),
            "ArtifactMaintenance QiTransfer::new 不应失败，实际 {:?}",
            maint.err()
        );

        let evo = QiTransfer::new(from, to, 30.0, QiTransferReason::ArtifactEvolution);
        assert!(
            evo.is_ok(),
            "ArtifactEvolution QiTransfer::new 不应失败，实际 {:?}",
            evo.err()
        );
    }
}
