//! 客户端 → 服务端请求 schema（plan-cultivation-v1 §P1 剩余）。
//!
//! 与 TypeScript `agent/packages/schema/src/client-request.ts` 1:1。
//! 由 Fabric 客户端通过 Minecraft CustomPayload 发送，服务端反序列化为对应
//! Bevy Event（MeridianTarget Component 更新 / BreakthroughRequest / ForgeRequest）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::agent_ui::AgentUiActionType;
use super::alchemy::AlchemyInterventionV1;
use super::combat_carrier::AnqiContainerKindV1;
use super::inventory::{EquipSlotV1, InventoryLocationV1};
use super::movement::MovementActionRequestV1;
use super::tuike::FalseSkinKindV1;
use super::void_actions::VoidActionRequestV1;
use crate::cultivation::components::MeridianChannelId;
use crate::cultivation::forging::ForgeAxis;
use crate::network::gate::{
    DimensionRule, DistanceRule, GateSpec, GateTarget, NoGateReason, OwnershipRule, RequestGate,
    StateGateId,
};
use crate::zhenfa::{
    trap_content::TrapTargetFace, ZhenfaCarrierKind, ZhenfaDisarmMode, ZhenfaKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplyPillTargetV1 {
    #[serde(rename = "self")]
    SelfTarget,
    Meridian {
        meridian_id: MeridianChannelId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum ClientRequestV1 {
    SetMeridianTarget {
        v: u8,
        meridian: MeridianChannelId,
    },
    BreakthroughRequest {
        v: u8,
    },
    StartDuXu {
        v: u8,
    },
    /// plan-void-actions-v1 — 化虚专属三类世界级 action。
    VoidAction {
        v: u8,
        request: VoidActionRequestV1,
    },
    /// plan-movement-v1 — 冲刺 / 滑铲 / 二段跳移动动作。
    MovementAction {
        v: u8,
        action: MovementActionRequestV1,
        #[serde(skip_serializing_if = "Option::is_none")]
        yaw_degrees: Option<f32>,
    },
    AbortTribulation {
        v: u8,
    },
    HeartDemonDecision {
        v: u8,
        choice_idx: Option<u32>,
    },
    ForgeRequest {
        v: u8,
        meridian: MeridianChannelId,
        axis: ForgeAxis,
    },
    /// 顿悟邀约回执：玩家选择 / 拒绝 / 超时。
    /// `choice_idx = Some(n)` → 选中第 n 个候选；`None` → 拒绝或超时（服务端等价处理）。
    InsightDecision {
        v: u8,
        trigger_id: String,
        choice_idx: Option<u32>,
    },
    BotanyHarvestRequest {
        v: u8,
        session_id: String,
        mode: crate::schema::botany::BotanyHarvestModeV1,
    },
    // ─── 炼丹（plan-alchemy-v1 §4） ─────────────────────────
    AlchemyOpenFurnace {
        v: u8,
        furnace_pos: (i32, i32, i32),
    },
    AlchemyFeedSlot {
        v: u8,
        furnace_pos: (i32, i32, i32),
        slot_idx: u8,
        material: String,
        count: u32,
    },
    AlchemyTakeBack {
        v: u8,
        furnace_pos: (i32, i32, i32),
        slot_idx: u8,
    },
    AlchemyIgnite {
        v: u8,
        furnace_pos: (i32, i32, i32),
        recipe_id: String,
    },
    AlchemyIntervention {
        v: u8,
        furnace_pos: (i32, i32, i32),
        intervention: AlchemyInterventionV1,
    },
    AlchemyTurnPage {
        v: u8,
        delta: i32,
    },
    AlchemyLearnRecipe {
        v: u8,
        recipe_id: String,
    },
    /// plan-onboarding-loop-v1 P2.2 — 使用丹方残卷学习碎片化丹方知识。
    AlchemyLearnRecipeFragment {
        v: u8,
        item_instance_id: u64,
    },
    AlchemyTakePill {
        v: u8,
        pill_item_id: String,
    },
    /// plan-alchemy-v1 §1.2 — 玩家手持炉类物品，客户端拦截右键地面并发此请求。
    /// server 校验 `item_instance_id` 为合法炉类物品 → 消耗一个 → 在 `pos`
    /// spawn `AlchemyFurnace` ECS entity，并把对应方块刷成 `FURNACE`。
    AlchemyFurnacePlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
    },
    /// plan-spawn-tutorial-v1 P0 — 玩家右键出生石棺，服务端授予龛石一次。
    CoffinOpen {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-coffin-v1 — 手持凡物棺材右键地面，放置两个 CHEST 占位方块。
    CoffinPlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
    },
    /// plan-block-lifecycle-v1 P2 — 玩家从 Bong 背包选中方块物品后请求放置。
    BlockPlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
        target_face: TrapTargetFace,
    },
    /// plan-worldgen-v4 P5 §8.1#5 — 画廊审阅 owo 方块面板 dev-only give-block C2S。
    /// 玩家在 InspectScreen 内 BlockPickerPanel 点击 vanilla 方块条目后发送。
    /// server dev handler 经 gamemode 校验后，从 ItemRegistry 取 `vanilla:<block_id>`
    /// 模板把对应 vanilla 方块物品放进背包（**非生产 gameplay**，dev/creative 限定）。
    /// `block_id` 为不含 namespace 的短名（如 `stone_bricks`）；`count` ∈ 1..=64。
    BlockPickerGive {
        v: u8,
        block_id: String,
        /// 1..=64（一组上限）。serde 不强制 TypeBox 的 minimum/maximum，恶意 client 可发
        /// count=0 或 count>64 绕过 schema，故在反序列化处显式守门（第一道纵深防御；handler
        /// 仍复查一道，因为内部 event 可绕过 wire 直接构造）。
        #[serde(deserialize_with = "deserialize_block_picker_count")]
        count: u32,
    },
    /// plan-coffin-v1 — 右键凡物棺材任一半，进入卧棺状态。
    CoffinEnter {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-coffin-v1 — 主动离开卧棺状态。
    CoffinLeave {
        v: u8,
    },
    /// plan-coffin-tiers-v1 P3 — 左键攻击 marker 实体，破坏延寿棺（体验同破坏方块）。
    /// pos 为 marker 实体坐标反算的棺 lower 格（client 可选任一半，server 归一）。
    CoffinBreak {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-coffin-tiers-v1 P3 — G 菜单 [回收] 按钮：主动回收延寿棺，较全材料返还。
    CoffinMenuReclaim {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-social-v1 §2.1 — 消耗龛石，在目标坐标放置/替换当前角色唯一灵龛。
    SpiritNichePlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
    },
    /// plan-niche-craft-fix-v1 P1 — 使用 niche_repair_kit 修补自己的受损灵龛。
    SpiritNicheRepair {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
    },
    /// plan-social-v1 §2.3 — 客户端准星凝视同一方块满 3 秒后的揭露请求。
    SpiritNicheGaze {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-social-v1 §2.3 — 主动标记坐标以揭露命中的灵龛。
    SpiritNicheMarkCoordinate {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-niche-defense-v1 P0/P1/P2 — 灵龛守家载体激活请求。
    SpiritNicheActivateGuardian {
        v: u8,
        niche_pos: [i32; 3],
        guardian_kind: super::social::GuardianKindV1,
        #[serde(default)]
        materials: Vec<String>,
    },
    /// plan-social-v1 §6.1 — 切磋邀请 UI 回执。
    SparringInviteResponse {
        v: u8,
        invite_id: String,
        accepted: bool,
        #[serde(default)]
        timed_out: bool,
    },
    /// plan-social-v1 §6.2 — 面对面交易发起：发起者提供一个物品实例。
    TradeOfferRequest {
        v: u8,
        target: String,
        offered_instance_id: u64,
    },
    /// plan-social-v1 §6.2 — 交易目标选择一个回礼物品确认，或拒绝。
    TradeOfferResponse {
        v: u8,
        offer_id: String,
        accepted: bool,
        requested_instance_id: Option<u64>,
    },
    /// plan-npc-engagement-v1 P0 — 玩家右键 NPC 后请求服务端校验 inspect 目标。
    NpcInspectRequest {
        v: u8,
        npc_entity_id: i32,
    },
    /// plan-npc-engagement-v1 P2 — NPC 对话选项回执。
    NpcDialogueChoice {
        v: u8,
        npc_entity_id: i32,
        option_id: String,
    },
    /// plan-npc-engagement-v1 P1 — NPC 商人交易请求。
    NpcTradeRequest {
        v: u8,
        npc_entity_id: i32,
        #[serde(default)]
        offered_items: Vec<u64>,
        requested_item_id: String,
    },
    /// plan-zhenfa-v1 §3.1 / §3.2 — 持阵旗右键方块布置诡雷或警戒场。
    ZhenfaPlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        kind: ZhenfaKind,
        #[serde(default)]
        carrier: Option<ZhenfaCarrierKind>,
        qi_invest_ratio: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_instance_id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_face: Option<TrapTargetFace>,
    },
    /// plan-zhenfa-v1 §3.1.G — 主动引爆最近或指定的自有诡雷。
    ZhenfaTrigger {
        v: u8,
        #[serde(default)]
        instance_id: Option<u64>,
    },
    /// plan-zhenfa-v1 §3.1.E — 识色法发现阵眼后的拆除 / 强破流程。
    ZhenfaDisarm {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        mode: ZhenfaDisarmMode,
    },
    /// plan-zhenfa-content-v2 P2 — 主动使用散灵珠，服务端消费 inventory instance 并守恒注入所在 zone。
    QiScatterBeadUse {
        v: u8,
        item_instance_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        z: Option<i32>,
    },
    LearnSkillScroll {
        v: u8,
        instance_id: u64,
    },
    TechniqueScrollUse {
        v: u8,
        instance_id: u64,
    },
    /// 客户端拖拽完成后通知 server 把 instance_id 从 from 移动到 to。
    /// server 校验后改 PlayerInventory，回推 inventory_event::moved。
    ///
    /// plan-rotate-v1 — `rotated=true` 表示本次落位前先把该 instance 的
    /// `grid_w`/`grid_h` 互换（2x1 ↔ 1x2 等）。`#[serde(default)]` 保证旧客户端
    /// 不带该字段的请求仍按 `false`（未旋转）解析，向后兼容。
    InventoryMoveIntent {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
        #[serde(default)]
        rotated: bool,
    },
    /// plan-tuike-v1 — 装备伪皮的专用 C2S 包；服务端落到 false_skin 装备槽。
    EquipFalseSkin {
        v: u8,
        slot: EquipSlotV1,
        item_instance_id: u64,
    },
    /// plan-tuike-v1 — 即时制作伪皮，扣材料与真元后产出对应 inventory item。
    ForgeFalseSkin {
        v: u8,
        kind: FalseSkinKindV1,
    },
    InventoryDiscardItem {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
    },
    /// plan-layered-equip-v1 P4（决议 #8）— 法宝激活/卸下到灵宝 UI 触发位。
    ///
    /// `activate=true`：把 `instance_id` 指向的灵宝从背包/装备结构移入触发位（容量满则拒绝）。
    /// `activate=false`：把触发位中的该灵宝卸下，落回背包（无空位则拒绝）。
    TreasureActivate {
        v: u8,
        instance_id: u64,
        activate: bool,
    },
    DropWeaponIntent {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
    },
    RepairWeaponIntent {
        v: u8,
        instance_id: u64,
        station_pos: [i32; 3],
    },
    PickupDroppedItem {
        v: u8,
        instance_id: u64,
    },
    /// plan-remains-suite P0 — 遗骸 G 键统一交互（对应右键 `InteractEntityEvent` 路径）。
    /// `remains_id` 是遗骸实体的 `UniqueId`（标准 UUID 字符串形式），来自
    /// client `remains_sync` 缓存（[`crate::schema::server_data::RemainsEntryV1`]）。
    RemainsLoot {
        v: u8,
        remains_id: String,
    },
    /// plan-mineral-v1 §3 — 凝脉+ 右键矿块，server 反查 MineralOreIndex。
    MineralProbe {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    /// plan-exploration-probe-return-v1 P1 — 凝脉+ 检视背包槽位物品，
    /// client 直接传 instance_id，server 校验该 instance_id 属于该玩家 inventory 后
    /// emit `FreshnessProbeIntent`。彻底消除容器 tab 歧义。
    FreshnessProbe {
        v: u8,
        /// 被查询物品的 inventory instance_id（由 client 从 InventoryModel 中读取）。
        instance_id: u64,
    },
    ApplyPill {
        v: u8,
        instance_id: u64,
        target: ApplyPillTargetV1,
    },
    /// plan-dugu-v1 §3.1.E — 自解毒蛊，消耗指定解蛊蕊实例 + 20 真元。
    SelfAntidote {
        v: u8,
        instance_id: u64,
    },
    DuoSheRequest {
        v: u8,
        target_id: String,
    },
    QiColorInspect {
        v: u8,
        observed: String,
    },
    UseLifeCore {
        v: u8,
        instance_id: u64,
    },
    /// plan-HUD-v1 §3.2 截脉弹反反应键。无 payload。
    /// server 翻译为 `DefenseIntent` Bevy event，立即开 1s `incoming_window`，
    /// 并回推 `defense_window` payload 让 client 渲染红环。
    Jiemai {
        v: u8,
    },
    /// plan-anqi-v1 P0：直接封骨请求。正式 UI 可通过 skillbar 触发默认投入；
    /// 该请求保留给滑块 UI 指定 `qi_target`。
    ChargeCarrier {
        v: u8,
        slot: Option<AnqiCarrierSlotV1>,
        qi_target: f32,
    },
    /// plan-anqi-v1 P0：手中 charged 兽骨抛射。`dir_unit` 由 client crosshair 给出。
    ThrowCarrier {
        v: u8,
        slot: AnqiCarrierSlotV1,
        dir_unit: [f32; 3],
        power: f32,
    },
    /// plan-anqi-v2 §4：切换暗器活跃容器。`to = None` 表示 F 键循环到下一个
    /// 可战斗切换容器；指定 `fenglinghe` 会被 combat 模块按暴露窗口规则拒绝。
    AnqiContainerSwitch {
        v: u8,
        #[serde(default)]
        to: Option<AnqiContainerKindV1>,
    },
    /// plan-HUD-v1 §4 / §11.3 触发 F1-F9 快捷使用槽。
    /// server 校验后插入 `Casting` Component，回推 `cast_sync(Casting)`；
    /// `tick_casts` 系统在 duration 到期时移除 Component 并推 `cast_sync(Complete)`。
    UseQuickSlot {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
    },
    /// plan-HUD-v1 §10 / §11.3 InspectScreen 内拖拽配置 F1-F9 槽。
    /// `item_id` 为 None 表示清空槽位。
    QuickSlotBind {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
        item_id: Option<String>,
        #[serde(default)]
        request_id: String,
    },
    /// plan-hotbar-modify-v1 §3.2：触发 1-9 技能栏槽位。
    SkillBarCast {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    /// plan-hotbar-modify-v1 §3.2：配置 1-9 技能栏；None 表示清空槽位。
    SkillBarBind {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
        binding: Option<SkillBarBindingV1>,
    },
    /// plan-hotbar-modify-v2 §2.3：保存某个招式的配置 JSON object。
    SkillConfigIntent {
        v: u8,
        skill_id: String,
        config: BTreeMap<String, serde_json::Value>,
    },
    CombatReincarnate {
        v: u8,
    },
    CombatTerminate {
        v: u8,
    },
    CombatCreateNewCharacter {
        v: u8,
    },
    StartExtractRequest {
        v: u8,
        portal_entity_id: u64,
    },
    CancelExtractRequest {
        v: u8,
    },
    StartSearch {
        v: u8,
        container_entity_id: u64,
    },
    CancelSearch {
        v: u8,
    },
    // ─── plan-supply-coffin-loot-ui P2：entity-based supply coffin open ────
    /// 玩家右键物资棺实体（Marker entity，无 server hitbox → InteractEntityEvent
    /// 不会触发）。客户端通过 crosshair 检测到 COFFIN_* 模型实体后发送此请求，
    /// server 用 `EntityManager::get_by_id` 解析 MC protocol entity_id → ECS Entity，
    /// 再 forward 到 `handle_supply_coffin_interact` 逻辑。
    SupplyCoffinOpen {
        v: u8,
        entity_id: i32,
    },
    // ─── plan-placeable-container-blocks-v1 P1：通用世界容器 open ────
    /// 玩家右键带 `ExternalContainer` 组件的世界容器 marker 后发送此请求。
    /// server 解析 MC protocol entity_id → ECS Entity，再由通用 open system
    /// 按目标实体的 `ExternalContainer.source_kind` 构造 `LootContainerOpen`。
    ContainerOpen {
        v: u8,
        entity_id: i32,
    },
    // ─── plan-workbench-place-runtime-v1 P2：entity-based workbench open ────
    /// 玩家右键制作台 Marker entity 后发送此请求。server 解析 MC protocol
    /// entity_id → ECS Entity，再交给 `WorkbenchOpenRequest` 做距离校验与开屏。
    WorkbenchOpen {
        v: u8,
        entity_id: i32,
    },
    // ─── plan-supply-coffin-loot-ui P1：外部容器 C2S ────────────────
    ExternalContainerMove {
        v: u8,
        session_id: u64,
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
    },
    ExternalContainerClose {
        v: u8,
        session_id: u64,
    },
    // ─── 灵田（plan-lingtian-v1 §1.2 / §1.4 / §1.5 / §1.6 / §1.7） ────
    /// plan §1.2.2 — 起开垦 session。terrain / environment 由 server 从
    /// chunk_layer 读 BlockKind 自动派生（避免客户端伪造）。
    LingtianStartTill {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        hoe_instance_id: u64,
        /// "manual" / "auto"（auto 需 herbalism Lv.3+，server 暂不校验）。
        mode: String,
    },
    /// plan §1.6 — 起翻新 session。
    LingtianStartRenew {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        hoe_instance_id: u64,
    },
    /// plan §1.2.3 — 起种植 session（背包内须有该 plant 的种子）。
    LingtianStartPlanting {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        plant_id: String,
    },
    /// plan §1.5 — 起收获 session（plot.crop 须 ripe）。
    LingtianStartHarvest {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        mode: String,
    },
    /// plan §1.4 — 起补灵 session。
    LingtianStartReplenish {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        /// "zone" / "bone_coin" / "beast_core" / "ling_shui" /
        /// "pill_residue_failed_pill" / "pill_residue_flawed_pill" /
        /// "pill_residue_processing_dregs" / "pill_residue_aging_scraps"。
        source: String,
    },
    /// plan §1.7 — 起偷灵 session。
    LingtianStartDrainQi {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    // ─── 炼器（武器）（plan-forge-v1 §4） ────────────────────────
    /// plan §1.3.1 — 起炉请求。client 拖齐坯料 + 选图谱后发起。
    /// plan-forge-session-entry-wiring-v1 §4.1#3 — 寻址从 `station_id: String`
    /// 改为 `station_pos`（对齐 alchemy `furnace_pos` 的 BlockPos 寻址模式）。
    ForgeStartSession {
        v: u8,
        station_pos: (i32, i32, i32),
        blueprint_id: String,
        materials: Vec<(String, u32)>,
    },
    /// plan §1.3.2 — 淬炼击键上报。
    ForgeTemperingHit {
        v: u8,
        session_id: u64,
        beat: String,
        ticks_remaining: u32,
    },
    /// plan §1.3.3 — 铭文残卷投入。
    ForgeInscriptionScroll {
        v: u8,
        session_id: u64,
        inscription_id: String,
    },
    /// plan §1.3.4 — 开光真元注入。
    ForgeConsecrationInject {
        v: u8,
        session_id: u64,
        qi_amount: f64,
    },
    /// plan §1.3 — 步骤推进（当前步骤完成，进下一步）。
    ForgeStepAdvance {
        v: u8,
        session_id: u64,
    },
    /// plan §1.4 — 图谱书翻页。
    ForgeBlueprintTurnPage {
        v: u8,
        delta: i32,
    },
    /// plan §1.4 — 学习图谱（客户端拖残卷到图谱区）。
    ForgeLearnBlueprint {
        v: u8,
        blueprint_id: String,
    },
    /// plan §1.2 — 玩家手持砧类物品，客户端拦截右键放砧方块。
    ForgeStationPlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
        station_tier: u8,
    },
    // ─── 通用手搓（plan-craft-v1 P2） ──────────────────────────
    /// plan-craft-v1 §2 — 玩家在 inventory 手搓 tab 内点 [开始手搓]。
    /// server 校验 unlock + 材料 + qi + 已有 session 后调 `start_craft`。
    CraftStart {
        v: u8,
        recipe_id: String,
        #[serde(
            default = "default_craft_quantity",
            deserialize_with = "deserialize_craft_quantity"
        )]
        quantity: u32,
    },
    /// plan-craft-v1 §5 决策门 #3 — 取消进行中的 craft session，70% 材料返还，qi 不退。
    CraftCancel {
        v: u8,
    },
    /// plan-dying-elder-v1 P1 — 玩家向垂死大能交付一颗回元丹。
    ///
    /// ## 守恒约束
    /// - 玩家背包须持有 `hui_yuan_pill` 一颗（server 以 `instance_id` 校验）；
    /// - 消耗后，丹携带的 qi_gain 值走 `QiTransfer{TradeDan}` 转入大能；
    /// - `elder_entity_id` 为 MC protocol entity_id（由 `EntityManager::get_by_id` 解析）；
    /// - 大能须处于 `DyingElderState::Plea` 或 `DyingElderState::Recovering{..}` 状态。
    GiveDanToElder {
        v: u8,
        /// 玩家背包中回元丹的 inventory instance_id。
        pill_instance_id: u64,
        /// 垂死大能的 MC protocol entity_id（i32）。
        elder_entity_id: i32,
    },
    /// plan-shield-block-v1 P1 — 按下右键边沿（off_hand 持盾），开始持续举盾状态。
    /// server 校验 off_hand 实装盾后插入 ShieldBlocking 持续状态。
    RaiseShield {
        v: u8,
    },
    /// plan-shield-block-v1 P1 — 松开右键边沿（off_hand 持盾），结束持续举盾状态。
    /// server 移除 ShieldBlocking 状态与 ShieldBlock component。
    LowerShield {
        v: u8,
    },
    /// plan-scroll-reading-v1 P0 — 玩家请求阅读一本可阅读残卷（proto C2S tag 100，§9）。
    /// server 查 `instance_id` 对应实例模板是否有 `readable_scroll_spec`：
    /// 有 → emit S2C `ScrollOpen` + `play_anim`；无 spec / 非本人物品 → 静默拒绝 + warn。
    /// 读取不消耗物品。
    ///
    /// **§9 契约落实偏差记录**：plan §9 proto 片段写的是 `string instance_id`，但全仓
    /// instance_id 字段（20+ 处，`envelope.proto` 与本文件其余 C2S 变体）无一例外为
    /// `u64`/`uint64`（含 client Java `InventoryItem.instanceId` 为 `long`）。判定为 plan
    /// 起草笔误，此处按仓库既有惯例用 `u64`，proto tag/message 名不变。
    ScrollReadRequest {
        v: u8,
        instance_id: u64,
    },
    /// plan-scroll-reading-v1 P1 §8.1#4 — 玩家关闭阅读屏（ESC / 关闭按钮），通知
    /// server 阅读会话结束（proto C2S tag 101）。空消息——依 `ev.client` 定位实体，
    /// 无需回传 instance_id/scroll_id（镜像 `RaiseShield`/`LowerShield` 结构）。
    /// server 侧 P2 落地 `emit_scroll_read_stop_for_entity` 真正停止循环阅读动画；
    /// P1 仅落契约 + 接收确认。
    ScrollReadClosed {
        v: u8,
    },
    // ─── plan-agent-ui-data-v1 P0：天道 UI 面板交互响应 ──────────────
    /// 玩家面板交互响应（button_click / dismissed / timeout / parse_error 等）。
    /// server 校验 request_id / allowed_button_ids 后转 `bong:agent_ui_response`。
    AgentUiResponse {
        v: u8,
        request_id: String,
        action: AgentUiActionType,
        #[serde(default)]
        params: std::collections::HashMap<String, String>,
    },
}

impl ClientRequestV1 {
    /// Return the contract-first gate declaration for this request variant.
    ///
    /// This match is deliberately exhaustive.  `NoGate(InvalidState)` is an
    /// explicit fail-closed marker for a domain whose adapter is not part of
    /// this registry slice; it must never be interpreted as implicit allow.
    #[allow(clippy::match_same_arms)]
    pub fn gate_spec(&self) -> RequestGate {
        match self {
            Self::SetMeridianTarget { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::BreakthroughRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::StartDuXu { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::VoidAction { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::MovementAction { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AbortTribulation { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::HeartDemonDecision { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::InsightDecision { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::BotanyHarvestRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyOpenFurnace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyFeedSlot { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyTakeBack { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyIgnite { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyIntervention { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyTurnPage { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyLearnRecipe { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyLearnRecipeFragment { .. } => {
                RequestGate::NoGate(NoGateReason::InvalidState)
            }
            Self::AlchemyTakePill { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AlchemyFurnacePlace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CoffinOpen { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CoffinPlace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::BlockPlace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::BlockPickerGive { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CoffinEnter { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CoffinLeave { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CoffinBreak { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CoffinMenuReclaim { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SpiritNichePlace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SpiritNicheRepair { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SpiritNicheGaze { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SpiritNicheMarkCoordinate { .. } => {
                RequestGate::NoGate(NoGateReason::InvalidState)
            }
            Self::SpiritNicheActivateGuardian { .. } => {
                RequestGate::NoGate(NoGateReason::InvalidState)
            }
            Self::SparringInviteResponse { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::TradeOfferRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::TradeOfferResponse { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::NpcInspectRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::NpcDialogueChoice { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::NpcTradeRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ZhenfaPlace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ZhenfaTrigger { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ZhenfaDisarm { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::QiScatterBeadUse { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LearnSkillScroll { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::TechniqueScrollUse { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::InventoryMoveIntent { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::EquipFalseSkin { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeFalseSkin { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::InventoryDiscardItem { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::TreasureActivate { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::DropWeaponIntent { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::RepairWeaponIntent { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::PickupDroppedItem { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::RemainsLoot { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::MineralProbe { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::FreshnessProbe { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ApplyPill { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SelfAntidote { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::DuoSheRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::QiColorInspect { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::UseLifeCore { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::Jiemai { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ChargeCarrier { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ThrowCarrier { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AnqiContainerSwitch { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::UseQuickSlot { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::QuickSlotBind { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SkillBarCast { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SkillBarBind { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SkillConfigIntent { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CombatReincarnate { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CombatTerminate { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CombatCreateNewCharacter { .. } => {
                RequestGate::NoGate(NoGateReason::InvalidState)
            }
            Self::StartExtractRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CancelExtractRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::StartSearch { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CancelSearch { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::SupplyCoffinOpen { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ContainerOpen { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::WorkbenchOpen { .. } => RequestGate::Spec(GateSpec {
                target: GateTarget::ProtocolEntityId,
                distance: DistanceRule::WORKBENCH,
                dimension: DimensionRule::Same,
                ownership: OwnershipRule::Any,
                state: &[
                    StateGateId::PlayerAlive,
                    StateGateId::TargetExists,
                    StateGateId::WorkbenchPresent,
                ],
            }),
            Self::ExternalContainerMove { .. } => RequestGate::Spec(GateSpec {
                target: GateTarget::SessionId,
                distance: DistanceRule::EXTERNAL_SESSION,
                dimension: DimensionRule::Same,
                ownership: OwnershipRule::Owner,
                state: &[
                    StateGateId::PlayerAlive,
                    StateGateId::TargetExists,
                    StateGateId::SessionOpen,
                    StateGateId::SessionOwner,
                    StateGateId::ExternalSession,
                ],
            }),
            Self::ExternalContainerClose { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LingtianStartTill { .. } => RequestGate::Spec(GateSpec {
                target: GateTarget::RequestBlockPosition,
                distance: DistanceRule::NEARBY_INTERACT,
                dimension: DimensionRule::Same,
                ownership: OwnershipRule::None,
                state: &[StateGateId::PlayerAlive, StateGateId::TargetExists],
            }),
            Self::LingtianStartRenew { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LingtianStartPlanting { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LingtianStartHarvest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LingtianStartReplenish { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LingtianStartDrainQi { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeStartSession { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeTemperingHit { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeInscriptionScroll { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeConsecrationInject { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeStepAdvance { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeBlueprintTurnPage { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeLearnBlueprint { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ForgeStationPlace { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::CraftStart { .. } => RequestGate::Spec(GateSpec {
                target: GateTarget::None,
                distance: DistanceRule::None,
                dimension: DimensionRule::Same,
                ownership: OwnershipRule::None,
                state: &[StateGateId::PlayerAlive, StateGateId::InventoryOpen],
            }),
            Self::CraftCancel { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::GiveDanToElder { .. } => RequestGate::Spec(GateSpec {
                target: GateTarget::ProtocolEntityId,
                distance: DistanceRule::NEARBY_INTERACT,
                dimension: DimensionRule::Same,
                ownership: OwnershipRule::Any,
                state: &[StateGateId::PlayerAlive, StateGateId::TargetExists],
            }),
            Self::RaiseShield { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::LowerShield { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ScrollReadRequest { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::ScrollReadClosed { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
            Self::AgentUiResponse { .. } => RequestGate::NoGate(NoGateReason::InvalidState),
        }
    }
}

fn default_craft_quantity() -> u32 {
    1
}

const MAX_CRAFT_QUANTITY_V1: u32 = crate::craft::MAX_CRAFT_QUANTITY;

fn deserialize_craft_quantity<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let quantity = u32::deserialize(deserializer)?;
    if quantity == 0 {
        return Err(serde::de::Error::custom("craft quantity must be >= 1"));
    }
    if quantity > MAX_CRAFT_QUANTITY_V1 {
        return Err(serde::de::Error::custom(format!(
            "craft quantity must be <= {MAX_CRAFT_QUANTITY_V1}"
        )));
    }
    Ok(quantity)
}

/// 单次 give-block 给予数量上限（Minecraft 一组堆叠上限）。与 TypeBox
/// `BlockPickerActionV1.count` 的 `maximum: 64` 对齐。
pub const MAX_BLOCK_PICKER_COUNT_V1: u32 = 64;

/// `BlockPickerGive.count` 反序列化守门：拒绝 0（下界）与 >64（上界）。
/// serde 不强制 TypeBox 的 minimum/maximum，所以这一道是 wire 输入的纵深防御第一关。
fn deserialize_block_picker_count<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let count = u32::deserialize(deserializer)?;
    if count == 0 {
        return Err(serde::de::Error::custom("block picker count must be >= 1"));
    }
    if count > MAX_BLOCK_PICKER_COUNT_V1 {
        return Err(serde::de::Error::custom(format!(
            "block picker count must be <= {MAX_BLOCK_PICKER_COUNT_V1}"
        )));
    }
    Ok(count)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnqiCarrierSlotV1 {
    MainHand,
    OffHand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum SkillBarBindingV1 {
    Item { template_id: String },
    Skill { skill_id: String },
}

fn deserialize_slot_index<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let slot = u8::deserialize(deserializer)?;
    if slot < 9 {
        Ok(slot)
    } else {
        Err(serde::de::Error::custom("slot must be between 0 and 8"))
    }
}

#[cfg(test)]
#[path = "client_request_tests.rs"]
mod tests;
