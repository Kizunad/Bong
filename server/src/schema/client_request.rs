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
use crate::cultivation::components::MeridianId;
use crate::cultivation::forging::ForgeAxis;
use crate::zhenfa::{
    trap_content::TrapTargetFace, ZhenfaCarrierKind, ZhenfaDisarmMode, ZhenfaKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplyPillTargetV1 {
    #[serde(rename = "self")]
    SelfTarget,
    Meridian {
        meridian_id: MeridianId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum ClientRequestV1 {
    SetMeridianTarget {
        v: u8,
        meridian: MeridianId,
    },
    BreakthroughRequest {
        v: u8,
    },
    StartDuXu {
        v: u8,
    },
    /// plan-void-actions-v1 — 化虚专属四类世界级 action。
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
        meridian: MeridianId,
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
    InventoryMoveIntent {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
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
    ForgeStartSession {
        v: u8,
        station_id: String,
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
mod tests {
    use super::*;

    #[test]
    fn set_meridian_target_roundtrip() {
        let json = r#"{"type":"set_meridian_target","v":1,"meridian":"Lung"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SetMeridianTarget { v, meridian } => {
                assert_eq!(v, 1);
                assert_eq!(meridian, MeridianId::Lung);
            }
            other => panic!("expected SetMeridianTarget, got {other:?}"),
        }
    }

    #[test]
    fn breakthrough_request_roundtrip() {
        let json = r#"{"type":"breakthrough_request","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::BreakthroughRequest { v: 1 }));
    }

    #[test]
    fn void_action_request_roundtrip() {
        let json =
            r#"{"type":"void_action","v":1,"request":{"kind":"suppress_tsy","zone_id":"tsy"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::VoidAction { v, request } => {
                assert_eq!(v, 1);
                assert_eq!(request.target_label(), "tsy");
            }
            other => panic!("expected VoidAction, got {other:?}"),
        }
    }

    #[test]
    fn movement_action_request_roundtrip() {
        let json = r#"{"type":"movement_action","v":1,"action":"dash"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::MovementAction {
                v,
                action,
                yaw_degrees,
            } => {
                assert_eq!(v, 1);
                assert_eq!(action, MovementActionRequestV1::Dash);
                assert_eq!(yaw_degrees, None);
            }
            other => panic!("expected MovementAction, got {other:?}"),
        }

        let encoded = serde_json::to_string(&req).expect("movement action serializes");
        let encoded_value: serde_json::Value =
            serde_json::from_str(&encoded).expect("encoded movement action is valid JSON");
        let expected_value: serde_json::Value =
            serde_json::from_str(json).expect("expected movement action JSON is valid");
        assert_eq!(
            encoded_value, expected_value,
            "movement_action roundtrip must preserve payload structure without depending on object key order"
        );
        assert!(
            encoded_value.get("yaw_degrees").is_none(),
            "movement_action without client yaw must omit yaw_degrees, actual: {encoded_value}"
        );
    }

    #[test]
    fn movement_action_request_accepts_client_yaw() {
        let json = r#"{"type":"movement_action","v":1,"action":"dash","yaw_degrees":90.5}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::MovementAction {
                v,
                action,
                yaw_degrees,
            } => {
                assert_eq!(v, 1);
                assert_eq!(action, MovementActionRequestV1::Dash);
                assert_eq!(yaw_degrees, Some(90.5));
            }
            other => panic!("expected MovementAction, got {other:?}"),
        }

        let encoded = serde_json::to_string(&req).expect("movement action serializes");
        let encoded_value: serde_json::Value =
            serde_json::from_str(&encoded).expect("encoded movement action yaw JSON is valid");
        let expected_value: serde_json::Value =
            serde_json::from_str(json).expect("expected movement action yaw JSON is valid");
        assert_eq!(
            encoded_value, expected_value,
            "movement_action with yaw_degrees must preserve payload structure without depending on object key order"
        );
        assert_eq!(
            encoded_value.get("yaw_degrees").and_then(|value| value.as_f64()),
            Some(90.5),
            "movement_action with client yaw must include numeric yaw_degrees, actual: {encoded_value}"
        );
    }

    #[test]
    fn movement_action_rejects_unknown_fields() {
        let json = r#"{"type":"movement_action","v":1,"action":"dash","extra":true}"#;
        let error =
            serde_json::from_str::<ClientRequestV1>(json).expect_err("extra field must fail");
        assert!(
            error.to_string().contains("unknown field"),
            "expected unknown-field error, got {error}"
        );
    }

    #[test]
    fn coffin_open_roundtrip() {
        let json = r#"{"type":"coffin_open","v":1,"x":0,"y":69,"z":0}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::CoffinOpen { v, x, y, z } => {
                assert_eq!(v, 1);
                assert_eq!([x, y, z], [0, 69, 0]);
            }
            other => panic!("expected CoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn coffin_lifecycle_requests_roundtrip() {
        let place = r#"{"type":"coffin_place","v":1,"x":8,"y":64,"z":8,"item_instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(place).unwrap();
        match req {
            ClientRequestV1::CoffinPlace {
                v,
                x,
                y,
                z,
                item_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (8, 64, 8));
                assert_eq!(item_instance_id, 4242);
            }
            other => panic!("expected CoffinPlace, got {other:?}"),
        }

        let enter = r#"{"type":"coffin_enter","v":1,"x":8,"y":64,"z":8}"#;
        let req: ClientRequestV1 = serde_json::from_str(enter).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::CoffinEnter {
                v: 1,
                x: 8,
                y: 64,
                z: 8
            }
        ));

        let leave = r#"{"type":"coffin_leave","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(leave).unwrap();
        assert!(matches!(req, ClientRequestV1::CoffinLeave { v: 1 }));
    }

    // ─── plan-coffin-tiers-v1 P3：coffin_break / coffin_menu_reclaim C2S schema tests ───

    #[test]
    fn coffin_break_roundtrip() {
        let json = r#"{"type":"coffin_break","v":1,"x":10,"y":64,"z":-5}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("coffin_break should parse: {e}"));
        assert!(
            matches!(
                req,
                ClientRequestV1::CoffinBreak {
                    v: 1,
                    x: 10,
                    y: 64,
                    z: -5
                }
            ),
            "coffin_break did not deserialize to expected variant, got: {req:?}"
        );
    }

    #[test]
    fn coffin_break_rejects_missing_coords() {
        let json = r#"{"type":"coffin_break","v":1}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "coffin_break without coordinates should fail deserialization"
        );
    }

    #[test]
    fn coffin_menu_reclaim_roundtrip() {
        let json = r#"{"type":"coffin_menu_reclaim","v":1,"x":3,"y":65,"z":7}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("coffin_menu_reclaim should parse: {e}"));
        assert!(
            matches!(
                req,
                ClientRequestV1::CoffinMenuReclaim {
                    v: 1,
                    x: 3,
                    y: 65,
                    z: 7
                }
            ),
            "coffin_menu_reclaim did not deserialize to expected variant, got: {req:?}"
        );
    }

    #[test]
    fn coffin_menu_reclaim_rejects_missing_coords() {
        let json = r#"{"type":"coffin_menu_reclaim","v":1}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "coffin_menu_reclaim without coordinates should fail deserialization"
        );
    }

    #[test]
    fn coffin_break_negative_coords_accepted() {
        // 负坐标合法（世界有负 XZ）。
        let json = r#"{"type":"coffin_break","v":1,"x":-128,"y":0,"z":-256}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("coffin_break with negative coords should parse: {e}"));
        assert!(
            matches!(
                req,
                ClientRequestV1::CoffinBreak {
                    v: 1,
                    x: -128,
                    y: 0,
                    z: -256
                }
            ),
            "negative coords should parse correctly, got: {req:?}"
        );
    }

    /// CodeRabbit minor G: coffin_menu_reclaim 对称负坐标测试。
    #[test]
    fn coffin_menu_reclaim_negative_coords_accepted() {
        // 负坐标合法（世界有负 XZ），与 coffin_break_negative_coords_accepted 对称。
        let json = r#"{"type":"coffin_menu_reclaim","v":1,"x":-64,"y":0,"z":-128}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap_or_else(|e| {
            panic!("coffin_menu_reclaim with negative coords should parse: {e}")
        });
        assert!(
            matches!(
                req,
                ClientRequestV1::CoffinMenuReclaim {
                    v: 1,
                    x: -64,
                    y: 0,
                    z: -128
                }
            ),
            "coffin_menu_reclaim negative coords should parse correctly; \
             期望 CoffinMenuReclaim{{x:-64,y:0,z:-128}}，实得 {req:?}"
        );
    }

    #[test]
    fn agent_ui_response_roundtrip_with_button_click() {
        let json = r#"{"type":"agent_ui_response","v":1,"request_id":"req_1","action":"button_click","params":{"button_id":"enter_realm"}}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("agent_ui_response button_click 应能反序列化");
        match req {
            ClientRequestV1::AgentUiResponse {
                v,
                request_id,
                action,
                params,
            } => {
                assert_eq!(v, 1, "wire version 期望=1，实为 {v}");
                assert_eq!(
                    request_id, "req_1",
                    "request_id 应保持 req_1，实为 {request_id}"
                );
                assert_eq!(
                    action,
                    AgentUiActionType::ButtonClick,
                    "action 应为 button_click，实为 {action:?}"
                );
                assert_eq!(
                    params.get("button_id").map(String::as_str),
                    Some("enter_realm"),
                    "params.button_id 应为 enter_realm，实为 {params:?}"
                );
            }
            other => panic!("expected AgentUiResponse, got {other:?}"),
        }
    }

    #[test]
    fn agent_ui_response_defaults_missing_params_to_empty_map() {
        let json =
            r#"{"type":"agent_ui_response","v":1,"request_id":"req_2","action":"dismissed"}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("agent_ui_response 缺 params 应默认空 map");
        assert!(
            matches!(req, ClientRequestV1::AgentUiResponse { ref params, .. } if params.is_empty()),
            "缺 params 应反序列化为空 map，实为 {req:?}"
        );
    }

    #[test]
    fn agent_ui_response_rejects_invalid_action_and_extra_fields() {
        let bad_action = r#"{"type":"agent_ui_response","v":1,"request_id":"req_3","action":"teleport","params":{}}"#;
        let bad_action_result: Result<ClientRequestV1, _> = serde_json::from_str(bad_action);
        assert!(
            bad_action_result.is_err(),
            "未知 action=teleport 应被 serde 拒绝，实为 {bad_action_result:?}"
        );

        let extra = r#"{"type":"agent_ui_response","v":1,"request_id":"req_4","action":"dismissed","params":{},"surprise":true}"#;
        let extra_result: Result<ClientRequestV1, _> = serde_json::from_str(extra);
        assert!(
            extra_result.is_err(),
            "agent_ui_response 额外字段 surprise 应被 deny_unknown_fields 拒绝，实为 {extra_result:?}"
        );
    }

    #[test]
    fn forge_request_roundtrip() {
        let json = r#"{"type":"forge_request","v":1,"meridian":"Ren","axis":"Rate"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ForgeRequest { meridian, axis, .. } => {
                assert_eq!(meridian, MeridianId::Ren);
                assert!(matches!(axis, ForgeAxis::Rate));
            }
            other => panic!("expected ForgeRequest, got {other:?}"),
        }
    }

    #[test]
    fn forge_request_capacity_axis_roundtrip() {
        let v = ClientRequestV1::ForgeRequest {
            v: 1,
            meridian: MeridianId::Du,
            axis: ForgeAxis::Capacity,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"axis\":\"Capacity\""));
        let back: ClientRequestV1 = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            ClientRequestV1::ForgeRequest {
                axis: ForgeAxis::Capacity,
                ..
            }
        ));
    }

    #[test]
    fn insight_decision_chosen_roundtrip() {
        let json =
            r#"{"type":"insight_decision","v":1,"trigger_id":"awaken_first","choice_idx":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::InsightDecision {
                v,
                trigger_id,
                choice_idx,
            } => {
                assert_eq!(v, 1);
                assert_eq!(trigger_id, "awaken_first");
                assert_eq!(choice_idx, Some(2));
            }
            other => panic!("expected InsightDecision, got {other:?}"),
        }
    }

    #[test]
    fn insight_decision_declined_roundtrip() {
        let json =
            r#"{"type":"insight_decision","v":1,"trigger_id":"awaken_first","choice_idx":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::InsightDecision {
                choice_idx: None,
                ..
            }
        ));
    }

    #[test]
    fn apply_pill_self_roundtrip() {
        let json = r#"{"type":"apply_pill","v":1,"instance_id":1001,"target":{"kind":"self"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ApplyPill {
                v,
                instance_id,
                target,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 1001);
                assert_eq!(target, ApplyPillTargetV1::SelfTarget);
            }
            other => panic!("expected ApplyPill, got {other:?}"),
        }
    }

    #[test]
    fn apply_pill_meridian_roundtrip() {
        let json = r#"{"type":"apply_pill","v":1,"instance_id":2002,"target":{"kind":"meridian","meridian_id":"Ren"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ApplyPill {
                instance_id,
                target,
                ..
            } => {
                assert_eq!(instance_id, 2002);
                assert_eq!(
                    target,
                    ApplyPillTargetV1::Meridian {
                        meridian_id: MeridianId::Ren,
                    }
                );
            }
            other => panic!("expected ApplyPill, got {other:?}"),
        }
    }

    #[test]
    fn self_antidote_roundtrip() {
        let json = r#"{"type":"self_antidote","v":1,"instance_id":3003}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SelfAntidote { v, instance_id } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 3003);
            }
            other => panic!("expected SelfAntidote, got {other:?}"),
        }
    }

    #[test]
    fn use_quick_slot_roundtrip() {
        let json = r#"{"type":"use_quick_slot","v":1,"slot":3}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::UseQuickSlot { v: 1, slot: 3 }
        ));
    }

    #[test]
    fn quick_slot_bind_roundtrip_and_clear() {
        let bind_json = r#"{"type":"quick_slot_bind","v":1,"slot":1,"item_id":"kai_mai_pill"}"#;
        let req: ClientRequestV1 = serde_json::from_str(bind_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::QuickSlotBind {
                v: 1,
                slot: 1,
                item_id: Some(ref item_id),
            } if item_id == "kai_mai_pill"
        ));

        let clear_json = r#"{"type":"quick_slot_bind","v":1,"slot":1,"item_id":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(clear_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::QuickSlotBind {
                v: 1,
                slot: 1,
                item_id: None,
            }
        ));
    }

    #[test]
    fn skill_bar_cast_roundtrip_with_optional_target() {
        let json = r#"{"type":"skill_bar_cast","v":1,"slot":0,"target":"entity:42"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SkillBarCast { v, slot, target } => {
                assert_eq!(v, 1);
                assert_eq!(slot, 0);
                assert_eq!(target.as_deref(), Some("entity:42"));
            }
            other => panic!("expected SkillBarCast, got {other:?}"),
        }

        let no_target = ClientRequestV1::SkillBarCast {
            v: 1,
            slot: 2,
            target: None,
        };
        let serialized = serde_json::to_string(&no_target).unwrap();
        assert!(
            !serialized.contains("target"),
            "target None should be omitted: {serialized}"
        );
    }

    #[test]
    fn anqi_container_switch_roundtrip() {
        let cycle_json = r#"{"type":"anqi_container_switch","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(cycle_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::AnqiContainerSwitch { v: 1, to: None }
        ));

        let direct_json = r#"{"type":"anqi_container_switch","v":1,"to":"quiver"}"#;
        let req: ClientRequestV1 = serde_json::from_str(direct_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::AnqiContainerSwitch {
                v: 1,
                to: Some(AnqiContainerKindV1::Quiver),
            }
        ));
    }

    #[test]
    fn skill_bar_bind_roundtrip_for_null_item_and_skill() {
        let clear_json = r#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(clear_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 0,
                binding: None,
            }
        ));

        let item_json = r#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"item","template_id":"iron_sword"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(item_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 1,
                binding: Some(SkillBarBindingV1::Item { ref template_id }),
            } if template_id == "iron_sword"
        ));

        let skill_json = r#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(skill_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 2,
                binding: Some(SkillBarBindingV1::Skill { ref skill_id }),
            } if skill_id == "burst_meridian.beng_quan"
        ));
    }

    #[test]
    fn skill_bar_binding_rejects_unknown_kind_and_extra_fields() {
        let wrong_kind = r#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"unknown","skill_id":"x"}}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(wrong_kind).is_err());

        let extra_field = r#"{"type":"skill_bar_cast","v":1,"slot":0,"extra":1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(extra_field).is_err());
    }

    #[test]
    fn hotbar_slot_indices_reject_out_of_range_values() {
        for json in [
            r#"{"type":"use_quick_slot","v":1,"slot":9}"#,
            r#"{"type":"quick_slot_bind","v":1,"slot":9,"item_id":null}"#,
            r#"{"type":"skill_bar_cast","v":1,"slot":9}"#,
            r#"{"type":"skill_bar_bind","v":1,"slot":9,"binding":null}"#,
        ] {
            let error = serde_json::from_str::<ClientRequestV1>(json)
                .expect_err("slot 9 should be rejected by schema");
            assert!(error.to_string().contains("slot must be between 0 and 8"));
        }
    }

    #[test]
    fn skill_config_intent_roundtrip_preserves_json_object() {
        let json = r#"{"type":"skill_config_intent","v":1,"skill_id":"zhenmai.sever_chain","config":{"meridian_id":"Pericardium","backfire_kind":"tainted_yuan"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SkillConfigIntent {
                v,
                skill_id,
                config,
            } => {
                assert_eq!(v, 1);
                assert_eq!(skill_id, "zhenmai.sever_chain");
                assert_eq!(
                    config.get("meridian_id"),
                    Some(&serde_json::json!("Pericardium"))
                );
            }
            other => panic!("expected SkillConfigIntent, got {other:?}"),
        }

        assert!(serde_json::from_str::<ClientRequestV1>(
            r#"{"type":"skill_config_intent","v":1,"skill_id":"x","config":{},"extra":1}"#
        )
        .is_err());
    }

    #[test]
    fn duo_she_request_roundtrip() {
        let json = r#"{"type":"duo_she_request","v":1,"target_id":"npc_12v0"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::DuoSheRequest { v, target_id } => {
                assert_eq!(v, 1);
                assert_eq!(target_id, "npc_12v0");
            }
            other => panic!("expected DuoSheRequest, got {other:?}"),
        }
    }

    #[test]
    fn qi_color_inspect_roundtrip() {
        let json = r#"{"type":"qi_color_inspect","v":1,"observed":"entity_bits:42"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::QiColorInspect { v, observed } => {
                assert_eq!(v, 1);
                assert_eq!(observed, "entity_bits:42");
            }
            other => panic!("expected QiColorInspect, got {other:?}"),
        }
    }

    #[test]
    fn use_life_core_roundtrip() {
        let json = r#"{"type":"use_life_core","v":1,"instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::UseLifeCore { v, instance_id } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 4242);
            }
            other => panic!("expected UseLifeCore, got {other:?}"),
        }
    }

    #[test]
    fn combat_reincarnate_roundtrip() {
        let json = r#"{"type":"combat_reincarnate","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::CombatReincarnate { v: 1 }));
    }

    #[test]
    fn combat_terminate_roundtrip() {
        let json = r#"{"type":"combat_terminate","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::CombatTerminate { v: 1 }));
    }

    #[test]
    fn combat_create_new_character_roundtrip() {
        let json = r#"{"type":"combat_create_new_character","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::CombatCreateNewCharacter { v: 1 }
        ));
    }

    #[test]
    fn pickup_dropped_item_roundtrip() {
        let json = r#"{"type":"pickup_dropped_item","v":1,"instance_id":3003}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::PickupDroppedItem { v, instance_id } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 3003);
            }
            other => panic!("expected PickupDroppedItem, got {other:?}"),
        }
    }

    #[test]
    fn mineral_probe_roundtrip() {
        let json = r#"{"type":"mineral_probe","v":1,"x":8,"y":32,"z":8}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::MineralProbe { v, x, y, z } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (8, 32, 8));
            }
            other => panic!("expected MineralProbe, got {other:?}"),
        }
    }

    /// plan-exploration-probe-return-v1 P1 — FreshnessProbe C2S serde round-trip。
    /// instance_id 字段：0 = 最小值，u64::MAX = 最大值，普通正整数 wire 对拍。
    #[test]
    fn freshness_probe_roundtrip() {
        let json = r#"{"type":"freshness_probe","v":1,"instance_id":4096}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::FreshnessProbe { v, instance_id } => {
                assert_eq!(v, 1, "version byte should be 1");
                assert_eq!(instance_id, 4096, "instance_id should round-trip as 4096");
            }
            other => panic!("expected FreshnessProbe, got {other:?}"),
        }
        // 边界：instance_id=0 合法
        let json_zero = r#"{"type":"freshness_probe","v":1,"instance_id":0}"#;
        let req_zero: ClientRequestV1 = serde_json::from_str(json_zero).unwrap();
        match req_zero {
            ClientRequestV1::FreshnessProbe { instance_id: 0, .. } => {}
            other => panic!("expected FreshnessProbe{{instance_id=0}}, got {other:?}"),
        }
        // 负例：缺少 instance_id 应反序列化失败
        let bad_json = r#"{"type":"freshness_probe","v":1}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(bad_json).is_err(),
            "FreshnessProbe without instance_id should be rejected"
        );
    }

    #[test]
    fn inventory_discard_item_roundtrip() {
        let json = r#"{"type":"inventory_discard_item","v":1,"instance_id":1001,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::InventoryDiscardItem {
                v,
                instance_id,
                from,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 1001);
                assert_eq!(
                    from,
                    InventoryLocationV1::Container {
                        container_id: "main_pack".to_string(),
                        row: 0,
                        col: 0,
                    }
                );
            }
            other => panic!("expected InventoryDiscardItem, got {other:?}"),
        }
    }

    #[test]
    fn drop_weapon_intent_roundtrip() {
        let json = r#"{"type":"drop_weapon_intent","v":1,"instance_id":1001,"from":{"kind":"equip","slot":"main_hand"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::DropWeaponIntent {
                v,
                instance_id,
                from,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 1001);
                assert_eq!(
                    from,
                    InventoryLocationV1::Equip {
                        slot: crate::schema::inventory::EquipSlotV1::MainHand,
                    }
                );
            }
            other => panic!("expected DropWeaponIntent, got {other:?}"),
        }
    }

    #[test]
    fn repair_weapon_intent_roundtrip() {
        let json =
            r#"{"type":"repair_weapon_intent","v":1,"instance_id":4242,"station_pos":[1,64,2]}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::RepairWeaponIntent {
                v,
                instance_id,
                station_pos,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 4242);
                assert_eq!(station_pos, [1, 64, 2]);
            }
            other => panic!("expected RepairWeaponIntent, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_type() {
        let json = r#"{"type":"nuke_world","v":1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(json).is_err());
    }

    #[test]
    fn botany_harvest_request_roundtrip() {
        let json = r#"{"type":"botany_harvest_request","v":1,"session_id":"session-botany-01","mode":"manual"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::BotanyHarvestRequest {
                v,
                session_id,
                mode,
            } => {
                assert_eq!(v, 1);
                assert_eq!(session_id, "session-botany-01");
                assert!(matches!(
                    mode,
                    crate::schema::botany::BotanyHarvestModeV1::Manual
                ));
            }
            other => panic!("expected BotanyHarvestRequest, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_open_furnace_roundtrip() {
        let json = r#"{"type":"alchemy_open_furnace","v":1,"furnace_pos":[-12,64,38]}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyOpenFurnace { v, furnace_pos } => {
                assert_eq!(v, 1);
                assert_eq!(furnace_pos, (-12, 64, 38));
            }
            other => panic!("expected AlchemyOpenFurnace, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_feed_slot_roundtrip() {
        let json = r#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[-12,64,38],"slot_idx":0,"material":"ci_she_hao","count":3}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyFeedSlot {
                furnace_pos,
                slot_idx,
                material,
                count,
                ..
            } => {
                assert_eq!(furnace_pos, (-12, 64, 38));
                assert_eq!(slot_idx, 0);
                assert_eq!(material, "ci_she_hao");
                assert_eq!(count, 3);
            }
            other => panic!("expected AlchemyFeedSlot, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_take_back_roundtrip() {
        let json = r#"{"type":"alchemy_take_back","v":1,"furnace_pos":[-12,64,38],"slot_idx":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyTakeBack {
                furnace_pos,
                slot_idx,
                ..
            } => {
                assert_eq!(furnace_pos, (-12, 64, 38));
                assert_eq!(slot_idx, 2);
            }
            other => panic!("expected AlchemyTakeBack, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_intervention_inject_qi_roundtrip() {
        let json = r#"{"type":"alchemy_intervention","v":1,"furnace_pos":[-12,64,38],"intervention":{"kind":"inject_qi","qi":1.0}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyIntervention {
                furnace_pos,
                intervention,
                ..
            } => {
                assert_eq!(furnace_pos, (-12, 64, 38));
                match intervention {
                    super::AlchemyInterventionV1::InjectQi { qi } => {
                        assert!((qi - 1.0).abs() < 1e-9)
                    }
                    other => panic!("expected InjectQi, got {other:?}"),
                }
            }
            other => panic!("expected AlchemyIntervention, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_turn_page_roundtrip() {
        let json = r#"{"type":"alchemy_turn_page","v":1,"delta":-1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyTurnPage { delta, .. } => assert_eq!(delta, -1),
            other => panic!("expected AlchemyTurnPage, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_furnace_place_roundtrip() {
        let json = r#"{"type":"alchemy_furnace_place","v":1,"x":-12,"y":64,"z":38,"item_instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyFurnacePlace {
                v,
                x,
                y,
                z,
                item_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (-12, 64, 38));
                assert_eq!(item_instance_id, 4242);
            }
            other => panic!("expected AlchemyFurnacePlace, got {other:?}"),
        }
    }

    #[test]
    fn spirit_niche_place_roundtrip() {
        let json =
            r#"{"type":"spirit_niche_place","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SpiritNichePlace {
                v,
                x,
                y,
                z,
                item_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (11, 64, 10));
                assert_eq!(item_instance_id, 4242);
            }
            other => panic!("expected SpiritNichePlace, got {other:?}"),
        }
    }

    #[test]
    fn spirit_niche_repair_roundtrip_and_rejects_extra_fields() {
        let json =
            r#"{"type":"spirit_niche_repair","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SpiritNicheRepair {
                v,
                x,
                y,
                z,
                item_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (11, 64, 10));
                assert_eq!(item_instance_id, 4242);
            }
            other => panic!("expected SpiritNicheRepair, got {other:?}"),
        }

        let extra = r#"{"type":"spirit_niche_repair","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242,"extra":true}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(extra).is_err(),
            "spirit_niche_repair must reject extra fields"
        );
    }

    #[test]
    fn spirit_niche_gaze_roundtrip() {
        let json = r#"{"type":"spirit_niche_gaze","v":1,"x":11,"y":64,"z":10}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SpiritNicheGaze { v, x, y, z } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (11, 64, 10));
            }
            other => panic!("expected SpiritNicheGaze, got {other:?}"),
        }
    }

    #[test]
    fn spirit_niche_mark_coordinate_roundtrip() {
        let json = r#"{"type":"spirit_niche_mark_coordinate","v":1,"x":11,"y":64,"z":10}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SpiritNicheMarkCoordinate { v, x, y, z } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (11, 64, 10));
            }
            other => panic!("expected SpiritNicheMarkCoordinate, got {other:?}"),
        }
    }

    #[test]
    fn sparring_invite_response_roundtrip() {
        let json = r#"{"type":"sparring_invite_response","v":1,"invite_id":"sparring:1:a:b","accepted":true,"timed_out":false}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SparringInviteResponse {
                v,
                invite_id,
                accepted,
                timed_out,
            } => {
                assert_eq!(v, 1);
                assert_eq!(invite_id, "sparring:1:a:b");
                assert!(accepted);
                assert!(!timed_out);
            }
            other => panic!("expected SparringInviteResponse, got {other:?}"),
        }
    }

    #[test]
    fn trade_offer_requests_roundtrip() {
        let request = r#"{"type":"trade_offer_request","v":1,"target":"entity:42","offered_instance_id":1001}"#;
        let req: ClientRequestV1 = serde_json::from_str(request).unwrap();
        match req {
            ClientRequestV1::TradeOfferRequest {
                v,
                target,
                offered_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(target, "entity:42");
                assert_eq!(offered_instance_id, 1001);
            }
            other => panic!("expected TradeOfferRequest, got {other:?}"),
        }

        let response = r#"{"type":"trade_offer_response","v":1,"offer_id":"trade:018f5a2a-7c30-7cc5-a14a-0b3c4d5e6f70","accepted":true,"requested_instance_id":2002}"#;
        let req: ClientRequestV1 = serde_json::from_str(response).unwrap();
        match req {
            ClientRequestV1::TradeOfferResponse {
                v,
                offer_id,
                accepted,
                requested_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(offer_id, "trade:018f5a2a-7c30-7cc5-a14a-0b3c4d5e6f70");
                assert!(accepted);
                assert_eq!(requested_instance_id, Some(2002));
            }
            other => panic!("expected TradeOfferResponse, got {other:?}"),
        }

        let decline = r#"{"type":"trade_offer_response","v":1,"offer_id":"trade:018f5a2a-7c30-7cc5-a14a-0b3c4d5e6f70","accepted":false}"#;
        let req: ClientRequestV1 = serde_json::from_str(decline).unwrap();
        match req {
            ClientRequestV1::TradeOfferResponse {
                requested_instance_id,
                ..
            } => assert_eq!(requested_instance_id, None),
            other => panic!("expected TradeOfferResponse, got {other:?}"),
        }
    }

    #[test]
    fn npc_engagement_requests_roundtrip() {
        let inspect = r#"{"type":"npc_inspect_request","v":1,"npc_entity_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(inspect).unwrap();
        match req {
            ClientRequestV1::NpcInspectRequest { v, npc_entity_id } => {
                assert_eq!(v, 1);
                assert_eq!(npc_entity_id, 42);
            }
            other => panic!("expected NpcInspectRequest, got {other:?}"),
        }

        let choice =
            r#"{"type":"npc_dialogue_choice","v":1,"npc_entity_id":42,"option_id":"trade"}"#;
        let req: ClientRequestV1 = serde_json::from_str(choice).unwrap();
        match req {
            ClientRequestV1::NpcDialogueChoice {
                v,
                npc_entity_id,
                option_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(npc_entity_id, 42);
                assert_eq!(option_id, "trade");
            }
            other => panic!("expected NpcDialogueChoice, got {other:?}"),
        }

        let trade = r#"{"type":"npc_trade_request","v":1,"npc_entity_id":42,"offered_items":[1001,1002],"requested_item_id":"spirit_grass"}"#;
        let req: ClientRequestV1 = serde_json::from_str(trade).unwrap();
        match req {
            ClientRequestV1::NpcTradeRequest {
                v,
                npc_entity_id,
                offered_items,
                requested_item_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(npc_entity_id, 42);
                assert_eq!(offered_items, vec![1001, 1002]);
                assert_eq!(requested_item_id, "spirit_grass");
            }
            other => panic!("expected NpcTradeRequest, got {other:?}"),
        }

        let trade_without_offers = r#"{"type":"npc_trade_request","v":1,"npc_entity_id":42,"requested_item_id":"spirit_grass"}"#;
        let req: ClientRequestV1 = serde_json::from_str(trade_without_offers).unwrap();
        match req {
            ClientRequestV1::NpcTradeRequest {
                v,
                npc_entity_id,
                offered_items,
                requested_item_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(npc_entity_id, 42);
                assert!(offered_items.is_empty());
                assert_eq!(requested_item_id, "spirit_grass");
            }
            other => panic!("expected NpcTradeRequest, got {other:?}"),
        }
    }

    #[test]
    fn block_place_roundtrip() {
        let json = r#"{"type":"block_place","v":1,"x":8,"y":64,"z":8,"item_instance_id":4242,"target_face":"north"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|error| panic!("block_place should parse: {error}"));
        match req {
            ClientRequestV1::BlockPlace {
                v,
                x,
                y,
                z,
                item_instance_id,
                target_face,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (8, 64, 8));
                assert_eq!(item_instance_id, 4242);
                assert_eq!(target_face, TrapTargetFace::North);
            }
            other => panic!("expected BlockPlace, got {other:?}"),
        }

        let encoded = serde_json::to_string(&req).expect("block_place serializes");
        let encoded_value: serde_json::Value =
            serde_json::from_str(&encoded).expect("encoded block_place JSON is valid");
        let expected_value: serde_json::Value =
            serde_json::from_str(json).expect("expected block_place JSON is valid");
        assert_eq!(
            encoded_value, expected_value,
            "block_place must preserve wire fields across serde roundtrip"
        );
    }

    #[test]
    fn block_place_rejects_unknown_fields() {
        let json = r#"{"type":"block_place","v":1,"x":8,"y":64,"z":8,"item_instance_id":4242,"target_face":"north","extra":true}"#;
        let error = serde_json::from_str::<ClientRequestV1>(json)
            .expect_err("block_place must reject unknown fields");
        assert!(
            error.to_string().contains("unknown field"),
            "expected unknown-field error for block_place, got {error}"
        );
    }

    #[test]
    fn zhenfa_requests_roundtrip() {
        let place = r#"{"type":"zhenfa_place","v":1,"x":1,"y":64,"z":-2,"kind":"blast_trap","carrier":"night_withered_vine","qi_invest_ratio":0.3,"trigger":"proximity","item_instance_id":9001,"target_face":"north"}"#;
        let req: ClientRequestV1 = serde_json::from_str(place).unwrap();
        match req {
            ClientRequestV1::ZhenfaPlace {
                v,
                x,
                y,
                z,
                kind,
                carrier,
                qi_invest_ratio,
                trigger,
                item_instance_id,
                target_face,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (1, 64, -2));
                assert_eq!(kind, ZhenfaKind::BlastTrap);
                assert_eq!(carrier, Some(ZhenfaCarrierKind::NightWitheredVine));
                assert_eq!(qi_invest_ratio, 0.3);
                assert_eq!(trigger.as_deref(), Some("proximity"));
                assert_eq!(item_instance_id, Some(9001));
                assert_eq!(target_face, Some(TrapTargetFace::North));
            }
            other => panic!("expected ZhenfaPlace, got {other:?}"),
        }

        for (kind_wire, expected_kind, item_instance_id) in [
            ("beast_trap", ZhenfaKind::BeastTrap, 9002),
            ("trip_wire", ZhenfaKind::TripWire, 9003),
            ("decoy_stake", ZhenfaKind::DecoyStake, 9004),
        ] {
            let json = format!(
                r#"{{"type":"zhenfa_place","v":1,"x":1,"y":64,"z":-2,"kind":"{kind_wire}","carrier":"common_stone","qi_invest_ratio":0.0,"item_instance_id":{item_instance_id},"target_face":"top"}}"#
            );
            let req: ClientRequestV1 = serde_json::from_str(&json).unwrap();
            match req {
                ClientRequestV1::ZhenfaPlace {
                    kind,
                    trigger,
                    item_instance_id: actual_item_instance_id,
                    target_face,
                    ..
                } => {
                    assert_eq!(kind, expected_kind);
                    assert!(
                        trigger.is_none(),
                        "runtime trap kind {kind_wire} omits trigger, so server contract must deserialize trigger=None"
                    );
                    assert_eq!(actual_item_instance_id, Some(item_instance_id));
                    assert_eq!(target_face, Some(TrapTargetFace::Top));
                }
                other => panic!("expected ZhenfaPlace for {kind_wire}, got {other:?}"),
            }
        }

        let trigger = r#"{"type":"zhenfa_trigger","v":1,"instance_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(trigger).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::ZhenfaTrigger {
                v: 1,
                instance_id: Some(42)
            }
        ));

        let nearest = r#"{"type":"zhenfa_trigger","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(nearest).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::ZhenfaTrigger {
                v: 1,
                instance_id: None
            }
        ));

        let disarm = r#"{"type":"zhenfa_disarm","v":1,"x":1,"y":64,"z":-2,"mode":"force_break"}"#;
        let req: ClientRequestV1 = serde_json::from_str(disarm).unwrap();
        match req {
            ClientRequestV1::ZhenfaDisarm { mode, .. } => {
                assert_eq!(mode, ZhenfaDisarmMode::ForceBreak);
            }
            other => panic!("expected ZhenfaDisarm, got {other:?}"),
        }

        let scatter = r#"{"type":"qi_scatter_bead_use","v":1,"item_instance_id":7001}"#;
        let req: ClientRequestV1 = serde_json::from_str(scatter).unwrap();
        match req {
            ClientRequestV1::QiScatterBeadUse {
                v,
                item_instance_id,
                x,
                y,
                z,
            } => {
                assert_eq!(v, 1);
                assert_eq!(item_instance_id, 7001);
                assert_eq!((x, y, z), (None, None, None));
            }
            other => panic!("expected QiScatterBeadUse, got {other:?}"),
        }

        let scatter_bury =
            r#"{"type":"qi_scatter_bead_use","v":1,"item_instance_id":7002,"x":1,"y":64,"z":-2}"#;
        let req: ClientRequestV1 = serde_json::from_str(scatter_bury).unwrap();
        match req {
            ClientRequestV1::QiScatterBeadUse {
                v,
                item_instance_id,
                x,
                y,
                z,
            } => {
                assert_eq!(v, 1);
                assert_eq!(item_instance_id, 7002);
                assert_eq!((x, y, z), (Some(1), Some(64), Some(-2)));
            }
            other => panic!("expected buried QiScatterBeadUse, got {other:?}"),
        }

        let unknown =
            r#"{"type":"qi_scatter_bead_use","v":1,"item_instance_id":7001,"extra":true}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(unknown).is_err(),
            "qi_scatter_bead_use must reject unknown fields"
        );
        let negative = r#"{"type":"qi_scatter_bead_use","v":1,"item_instance_id":-1}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(negative).is_err(),
            "qi_scatter_bead_use item_instance_id is u64 and must reject negative JSON"
        );
    }

    #[test]
    fn forge_station_place_roundtrip() {
        let json = r#"{"type":"forge_station_place","v":1,"x":-12,"y":64,"z":38,"item_instance_id":4242,"station_tier":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ForgeStationPlace {
                v,
                x,
                y,
                z,
                item_instance_id,
                station_tier,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (-12, 64, 38));
                assert_eq!(item_instance_id, 4242);
                assert_eq!(station_tier, 2);
            }
            other => panic!("expected ForgeStationPlace, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_ignite_roundtrip() {
        let json = r#"{"type":"alchemy_ignite","v":1,"furnace_pos":[-12,64,38],"recipe_id":"kai_mai_pill_v0"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyIgnite {
                furnace_pos,
                recipe_id,
                ..
            } => {
                assert_eq!(furnace_pos, (-12, 64, 38));
                assert_eq!(recipe_id, "kai_mai_pill_v0");
            }
            other => panic!("expected AlchemyIgnite, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_furnace_payloads_reject_unknown_fields() {
        let old_open = r#"{"type":"alchemy_open_furnace","v":1,"furnace_id":"block_-12_64_38","furnace_pos":[-12,64,38]}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(old_open).is_err());

        let typo_feed = r#"{"type":"alchemy_feed_slot","v":1,"furnace_position":[-12,64,38],"slot_idx":0,"material":"ci_she_hao","count":3}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(typo_feed).is_err());
    }

    #[test]
    fn craft_start_accepts_quantity_and_defaults_to_one() {
        let with_quantity = r#"{"type":"craft_start","v":1,"recipe_id":"craft.example.herb_knife.iron","quantity":3}"#;
        let req: ClientRequestV1 = serde_json::from_str(with_quantity).unwrap();
        match req {
            ClientRequestV1::CraftStart {
                v,
                recipe_id,
                quantity,
            } => {
                assert_eq!(v, 1);
                assert_eq!(recipe_id, "craft.example.herb_knife.iron");
                assert_eq!(quantity, 3);
            }
            other => panic!("expected CraftStart, got {other:?}"),
        }

        let legacy = r#"{"type":"craft_start","v":1,"recipe_id":"craft.example.herb_knife.iron"}"#;
        let req: ClientRequestV1 = serde_json::from_str(legacy).unwrap();
        match req {
            ClientRequestV1::CraftStart { quantity, .. } => assert_eq!(quantity, 1),
            other => panic!("expected CraftStart, got {other:?}"),
        }

        let zero = r#"{"type":"craft_start","v":1,"recipe_id":"craft.example.herb_knife.iron","quantity":0}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(zero).is_err());

        let too_large = r#"{"type":"craft_start","v":1,"recipe_id":"craft.example.herb_knife.iron","quantity":65}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(too_large).is_err());
    }

    #[test]
    fn extract_requests_roundtrip() {
        let start = r#"{"type":"start_extract_request","v":1,"portal_entity_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(start).unwrap();
        match req {
            ClientRequestV1::StartExtractRequest {
                v,
                portal_entity_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(portal_entity_id, 42);
            }
            other => panic!("expected StartExtractRequest, got {other:?}"),
        }

        let cancel = r#"{"type":"cancel_extract_request","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(cancel).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::CancelExtractRequest { v: 1 }
        ));
    }

    #[test]
    fn search_requests_roundtrip() {
        let start = r#"{"type":"start_search","v":1,"container_entity_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(start).unwrap();
        match req {
            ClientRequestV1::StartSearch {
                v,
                container_entity_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(container_entity_id, 42);
            }
            other => panic!("expected StartSearch, got {other:?}"),
        }

        let cancel = r#"{"type":"cancel_search","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(cancel).unwrap();
        assert!(matches!(req, ClientRequestV1::CancelSearch { v: 1 }));
    }

    #[test]
    fn search_requests_reject_missing_type_and_negative_id() {
        let missing_type = r#"{"v":1,"container_entity_id":42}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(missing_type).is_err());

        let negative_id = r#"{"type":"start_search","v":1,"container_entity_id":-1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(negative_id).is_err());
    }

    #[test]
    fn heart_demon_decision_roundtrip() {
        let json = r#"{"type":"heart_demon_decision","v":1,"choice_idx":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::HeartDemonDecision { v, choice_idx } => {
                assert_eq!(v, 1);
                assert_eq!(choice_idx, Some(2));
            }
            other => panic!("expected HeartDemonDecision, got {other:?}"),
        }

        let timeout = r#"{"type":"heart_demon_decision","v":1,"choice_idx":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(timeout).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::HeartDemonDecision {
                v: 1,
                choice_idx: None
            }
        ));
    }

    // ─── plan-supply-coffin-loot-ui P1：外部容器 C2S tests ──────────

    // ─── plan-supply-coffin-loot-ui P2：supply_coffin_open C2S tests ──

    #[test]
    fn supply_coffin_open_roundtrip() {
        let json = r#"{"type":"supply_coffin_open","v":1,"entity_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SupplyCoffinOpen { v, entity_id } => {
                assert_eq!(v, 1, "version should be 1");
                assert_eq!(entity_id, 42, "entity_id should be 42");
            }
            other => panic!("expected SupplyCoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn supply_coffin_open_negative_entity_id() {
        let json = r#"{"type":"supply_coffin_open","v":1,"entity_id":-1}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("negative entity_id is valid i32 at schema level");
        match req {
            ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
                assert_eq!(
                    entity_id, -1,
                    "negative entity_id should pass schema (rejected at runtime)"
                );
            }
            other => panic!("expected SupplyCoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn supply_coffin_open_rejects_missing_entity_id() {
        let json = r#"{"type":"supply_coffin_open","v":1}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "missing entity_id should fail"
        );
    }

    #[test]
    fn supply_coffin_open_rejects_extra_fields() {
        let json = r#"{"type":"supply_coffin_open","v":1,"entity_id":42,"extra":true}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "extra field should fail due to deny_unknown_fields"
        );
    }

    #[test]
    fn supply_coffin_open_with_zero_entity_id() {
        let json = r#"{"type":"supply_coffin_open","v":1,"entity_id":0}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("zero entity_id should be valid at schema level");
        match req {
            ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
                assert_eq!(entity_id, 0, "entity_id should be 0");
            }
            other => panic!("expected SupplyCoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn supply_coffin_open_with_max_entity_id() {
        let json = format!(
            r#"{{"type":"supply_coffin_open","v":1,"entity_id":{}}}"#,
            i32::MAX
        );
        let req: ClientRequestV1 =
            serde_json::from_str(&json).expect("i32::MAX entity_id should be valid");
        match req {
            ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
                assert_eq!(entity_id, i32::MAX, "entity_id should be i32::MAX");
            }
            other => panic!("expected SupplyCoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn supply_coffin_open_with_min_entity_id() {
        let json = format!(
            r#"{{"type":"supply_coffin_open","v":1,"entity_id":{}}}"#,
            i32::MIN
        );
        let req: ClientRequestV1 = serde_json::from_str(&json)
            .expect("i32::MIN entity_id should be valid at schema level");
        match req {
            ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
                assert_eq!(entity_id, i32::MIN, "entity_id should be i32::MIN");
            }
            other => panic!("expected SupplyCoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn workbench_open_roundtrip() {
        let json = r#"{"type":"workbench_open","v":1,"entity_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::WorkbenchOpen { v, entity_id } => {
                assert_eq!(v, 1, "version should be 1");
                assert_eq!(entity_id, 42, "entity_id should be 42");
            }
            other => panic!("expected WorkbenchOpen, got {other:?}"),
        }
    }

    #[test]
    fn workbench_open_rejects_missing_entity_id() {
        let json = r#"{"type":"workbench_open","v":1}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "missing entity_id should fail"
        );
    }

    #[test]
    fn workbench_open_rejects_extra_fields() {
        let json = r#"{"type":"workbench_open","v":1,"entity_id":42,"extra":true}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "extra field should fail due to deny_unknown_fields"
        );
    }

    #[test]
    fn external_container_move_roundtrip() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 42,
            "instance_id": 100,
            "from": {"kind": "container", "container_id": "ext_42", "row": 0, "col": 1},
            "to": {"kind": "container", "container_id": "body_pocket", "row": 1, "col": 0}
        }"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("external_container_move should deserialize");
        match req {
            ClientRequestV1::ExternalContainerMove {
                v,
                session_id,
                instance_id,
                from,
                to,
            } => {
                assert_eq!(v, 1, "version should be 1");
                assert_eq!(session_id, 42, "session_id should be 42");
                assert_eq!(instance_id, 100, "instance_id should be 100");
                match from {
                    InventoryLocationV1::Container {
                        container_id,
                        row,
                        col,
                    } => {
                        assert_eq!(container_id, "ext_42", "from container_id");
                        assert_eq!(row, 0, "from row");
                        assert_eq!(col, 1, "from col");
                    }
                    other => panic!("expected Container, got {other:?}"),
                }
                match to {
                    InventoryLocationV1::Container {
                        container_id,
                        row,
                        col,
                    } => {
                        assert_eq!(container_id, "body_pocket", "to container_id");
                        assert_eq!(row, 1, "to row");
                        assert_eq!(col, 0, "to col");
                    }
                    other => panic!("expected Container, got {other:?}"),
                }
            }
            other => panic!("expected ExternalContainerMove, got {other:?}"),
        }
    }

    #[test]
    fn external_container_close_roundtrip() {
        let json = r#"{"type": "external_container_close", "v": 1, "session_id": 99}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("external_container_close should deserialize");
        match req {
            ClientRequestV1::ExternalContainerClose { v, session_id } => {
                assert_eq!(v, 1, "version should be 1");
                assert_eq!(session_id, 99, "session_id should be 99");
            }
            other => panic!("expected ExternalContainerClose, got {other:?}"),
        }
    }

    #[test]
    fn external_container_move_rejects_missing_session_id() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "instance_id": 100,
            "from": {"kind": "container", "container_id": "ext_1", "row": 0, "col": 0},
            "to": {"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0}
        }"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "missing session_id should fail"
        );
    }

    #[test]
    fn external_container_move_with_equip_slot() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 5,
            "instance_id": 200,
            "from": {"kind": "container", "container_id": "ext_5", "row": 2, "col": 3},
            "to": {"kind": "equip", "slot": "main_hand"}
        }"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("move to equip slot should deserialize");
        match req {
            ClientRequestV1::ExternalContainerMove { to, .. } => {
                assert!(
                    matches!(to, InventoryLocationV1::Equip { .. }),
                    "to should be Equip, got {to:?}"
                );
            }
            other => panic!("expected ExternalContainerMove, got {other:?}"),
        }
    }

    #[test]
    fn external_container_close_rejects_missing_session_id() {
        let json = r#"{"type": "external_container_close", "v": 1}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "missing session_id should fail"
        );
    }

    #[test]
    fn external_container_move_rejects_missing_from() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 1,
            "instance_id": 100,
            "to": {"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0}
        }"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "missing from should fail"
        );
    }

    #[test]
    fn external_container_move_rejects_missing_to() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 1,
            "instance_id": 100,
            "from": {"kind": "container", "container_id": "ext_1", "row": 0, "col": 0}
        }"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "missing to should fail"
        );
    }

    #[test]
    fn external_container_move_with_zero_session_id() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 0,
            "instance_id": 0,
            "from": {"kind": "container", "container_id": "ext_0", "row": 0, "col": 0},
            "to": {"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0}
        }"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .expect("zero session_id/instance_id should be valid at the schema level");
        match req {
            ClientRequestV1::ExternalContainerMove {
                session_id,
                instance_id,
                ..
            } => {
                assert_eq!(session_id, 0, "session_id should be 0");
                assert_eq!(instance_id, 0, "instance_id should be 0");
            }
            other => panic!("expected ExternalContainerMove, got {other:?}"),
        }
    }

    #[test]
    fn external_container_move_with_max_ids() {
        let json = format!(
            r#"{{
                "type": "external_container_move",
                "v": 1,
                "session_id": {max},
                "instance_id": {max},
                "from": {{"kind": "container", "container_id": "ext_1", "row": 0, "col": 0}},
                "to": {{"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0}}
            }}"#,
            max = u64::MAX
        );
        let req: ClientRequestV1 = serde_json::from_str(&json)
            .expect("u64::MAX session_id/instance_id should be valid at schema level");
        match req {
            ClientRequestV1::ExternalContainerMove {
                session_id,
                instance_id,
                ..
            } => {
                assert_eq!(session_id, u64::MAX, "session_id should be u64::MAX");
                assert_eq!(instance_id, u64::MAX, "instance_id should be u64::MAX");
            }
            other => panic!("expected ExternalContainerMove, got {other:?}"),
        }
    }

    #[test]
    fn external_container_close_with_max_session_id() {
        let json = format!(
            r#"{{"type": "external_container_close", "v": 1, "session_id": {max}}}"#,
            max = u64::MAX
        );
        let req: ClientRequestV1 = serde_json::from_str(&json)
            .expect("u64::MAX session_id should be valid at schema level");
        match req {
            ClientRequestV1::ExternalContainerClose { session_id, .. } => {
                assert_eq!(session_id, u64::MAX, "session_id should be u64::MAX");
            }
            other => panic!("expected ExternalContainerClose, got {other:?}"),
        }
    }

    #[test]
    fn external_container_move_rejects_invalid_location_kind() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 1,
            "instance_id": 1,
            "from": {"kind": "wormhole", "container_id": "ext_1", "row": 0, "col": 0},
            "to": {"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0}
        }"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "invalid location kind 'wormhole' should fail deserialization"
        );
    }

    #[test]
    fn external_container_move_equip_to_container() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 10,
            "instance_id": 50,
            "from": {"kind": "equip", "slot": "main_hand"},
            "to": {"kind": "container", "container_id": "ext_10", "row": 1, "col": 2}
        }"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("equip→container should deserialize");
        match req {
            ClientRequestV1::ExternalContainerMove { from, to, .. } => {
                assert!(
                    matches!(from, InventoryLocationV1::Equip { .. }),
                    "from should be Equip, got {from:?}"
                );
                match to {
                    InventoryLocationV1::Container {
                        container_id,
                        row,
                        col,
                    } => {
                        assert_eq!(container_id, "ext_10", "to container_id");
                        assert_eq!(row, 1, "to row");
                        assert_eq!(col, 2, "to col");
                    }
                    other => panic!("expected Container, got {other:?}"),
                }
            }
            other => panic!("expected ExternalContainerMove, got {other:?}"),
        }
    }

    // ── plan-dying-elder-v1 P1：GiveDanToElder C2S schema tests ─────────────

    #[test]
    fn give_dan_to_elder_roundtrip() {
        // 期望：GiveDanToElder 序列化/反序列化往返，字段完全保留
        let json =
            r#"{"type":"give_dan_to_elder","v":1,"pill_instance_id":4242,"elder_entity_id":99}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("give_dan_to_elder should deserialize: {e}"));
        match req {
            ClientRequestV1::GiveDanToElder {
                v,
                pill_instance_id,
                elder_entity_id,
            } => {
                assert_eq!(v, 1, "version byte should be 1");
                assert_eq!(pill_instance_id, 4242, "pill_instance_id should be 4242");
                assert_eq!(elder_entity_id, 99, "elder_entity_id should be 99");
            }
            other => panic!("expected GiveDanToElder, got {other:?}"),
        }
    }

    #[test]
    fn give_dan_to_elder_rejects_missing_pill_instance_id() {
        // 期望：缺少 pill_instance_id 应反序列化失败
        let json = r#"{"type":"give_dan_to_elder","v":1,"elder_entity_id":99}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "GiveDanToElder 缺少 pill_instance_id 应拒绝反序列化，\
             但实际成功：{result:?}"
        );
    }

    #[test]
    fn give_dan_to_elder_rejects_missing_elder_entity_id() {
        // 期望：缺少 elder_entity_id 应反序列化失败
        let json = r#"{"type":"give_dan_to_elder","v":1,"pill_instance_id":1001}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "GiveDanToElder 缺少 elder_entity_id 应拒绝反序列化，\
             但实际成功：{result:?}"
        );
    }

    #[test]
    fn give_dan_to_elder_accepts_negative_elder_entity_id() {
        // 期望：elder_entity_id 为 i32，可以是负值（MC protocol entity_id 可负）
        let json =
            r#"{"type":"give_dan_to_elder","v":1,"pill_instance_id":1,"elder_entity_id":-1}"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("负 elder_entity_id 应合法：{e}"));
        assert!(
            matches!(
                req,
                ClientRequestV1::GiveDanToElder {
                    elder_entity_id: -1,
                    ..
                }
            ),
            "elder_entity_id=-1 应被接受（MC protocol i32 范围），实际：{req:?}"
        );
    }

    #[test]
    fn give_dan_to_elder_max_pill_instance_id() {
        // 期望：pill_instance_id = u64::MAX 是合法边界值
        let max_id = u64::MAX;
        let json = format!(
            r#"{{"type":"give_dan_to_elder","v":1,"pill_instance_id":{max_id},"elder_entity_id":1}}"#
        );
        let req: ClientRequestV1 = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("pill_instance_id=u64::MAX 应合法：{e}"));
        match req {
            ClientRequestV1::GiveDanToElder {
                pill_instance_id, ..
            } => {
                assert_eq!(pill_instance_id, max_id, "u64::MAX 应原样往返");
            }
            other => panic!("expected GiveDanToElder, got {other:?}"),
        }
    }

    #[test]
    fn external_container_move_equip_to_equip() {
        let json = r#"{
            "type": "external_container_move",
            "v": 1,
            "session_id": 1,
            "instance_id": 1,
            "from": {"kind": "equip", "slot": "main_hand"},
            "to": {"kind": "equip", "slot": "off_hand"}
        }"#;
        let req: ClientRequestV1 =
            serde_json::from_str(json).expect("equip→equip should deserialize");
        match req {
            ClientRequestV1::ExternalContainerMove { from, to, .. } => {
                assert!(
                    matches!(from, InventoryLocationV1::Equip { .. }),
                    "from should be Equip"
                );
                assert!(
                    matches!(to, InventoryLocationV1::Equip { .. }),
                    "to should be Equip"
                );
            }
            other => panic!("expected ExternalContainerMove, got {other:?}"),
        }
    }

    // ─── plan-shield-block-v1 P1 CR#1：负向样例双端 serde 对拍（include_str! pin）───

    /// CR#1 — 正向样例 include_str! pin：确认 samples/ 里的 JSON 文件内容与 Rust serde 可双端对拍。
    #[test]
    fn raise_shield_positive_sample_deserializes() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/client-request.raise-shield.sample.json"
        );
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("positive raise-shield sample should deserialize: {e}"));
        assert!(
            matches!(req, ClientRequestV1::RaiseShield { v: 1 }),
            "positive sample must deserialize to RaiseShield{{v:1}}, got {req:?}"
        );
    }

    #[test]
    fn lower_shield_positive_sample_deserializes() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/client-request.lower-shield.sample.json"
        );
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("positive lower-shield sample should deserialize: {e}"));
        assert!(
            matches!(req, ClientRequestV1::LowerShield { v: 1 }),
            "positive sample must deserialize to LowerShield{{v:1}}, got {req:?}"
        );
    }

    /// CR#1 — 负向样例 include_str! pin：缺 v 字段被 serde 拒绝。
    #[test]
    fn raise_shield_invalid_missing_v_sample_is_rejected() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/client-request.raise-shield.invalid-missing-v.sample.json"
        );
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "raise-shield invalid-missing-v sample must be rejected by serde; \
             missing v field cannot satisfy ClientRequestV1::RaiseShield{{v:u8}}"
        );
    }

    #[test]
    fn lower_shield_invalid_missing_v_sample_is_rejected() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/client-request.lower-shield.invalid-missing-v.sample.json"
        );
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "lower-shield invalid-missing-v sample must be rejected by serde"
        );
    }

    /// CR#1 — 负向样例 include_str! pin：额外字段被 deny_unknown_fields 拒绝。
    #[test]
    fn raise_shield_invalid_extra_field_sample_is_rejected() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/client-request.raise-shield.invalid-extra-field.sample.json"
        );
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "raise-shield invalid-extra-field sample must be rejected (deny_unknown_fields)"
        );
    }

    #[test]
    fn lower_shield_invalid_extra_field_sample_is_rejected() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/client-request.lower-shield.invalid-extra-field.sample.json"
        );
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "lower-shield invalid-extra-field sample must be rejected (deny_unknown_fields)"
        );
    }

    // ─── plan-shield-block-v1 P1 CR#7：RaiseShield / LowerShield serde pin tests ───

    /// CR#7 — RaiseShield happy-path round-trip：{"type":"raise_shield","v":1} 反序列化为 RaiseShield{v:1}。
    #[test]
    fn raise_shield_roundtrip() {
        let json = r#"{"type":"raise_shield","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("raise_shield should deserialize: {e}"));
        assert!(
            matches!(req, ClientRequestV1::RaiseShield { v: 1 }),
            "expected RaiseShield{{v:1}}, got {req:?}"
        );
        // round-trip：serialize 回去应与原 wire JSON 一致
        let encoded = serde_json::to_string(&req).expect("RaiseShield serializes");
        let encoded_val: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let expected_val: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            encoded_val, expected_val,
            "RaiseShield round-trip must preserve wire structure; expected={expected_val} actual={encoded_val}"
        );
    }

    /// CR#7 — LowerShield happy-path round-trip：{"type":"lower_shield","v":1} 反序列化为 LowerShield{v:1}。
    #[test]
    fn lower_shield_roundtrip() {
        let json = r#"{"type":"lower_shield","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("lower_shield should deserialize: {e}"));
        assert!(
            matches!(req, ClientRequestV1::LowerShield { v: 1 }),
            "expected LowerShield{{v:1}}, got {req:?}"
        );
        let encoded = serde_json::to_string(&req).expect("LowerShield serializes");
        let encoded_val: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let expected_val: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            encoded_val, expected_val,
            "LowerShield round-trip must preserve wire structure; expected={expected_val} actual={encoded_val}"
        );
    }

    /// CR#7 — RaiseShield 缺失 v 字段应反序列化失败。
    #[test]
    fn raise_shield_rejects_missing_v() {
        let json = r#"{"type":"raise_shield"}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "raise_shield without v field should fail deserialization; \
             v is required by serde for enum variant discrimination"
        );
    }

    /// CR#7 — LowerShield 缺失 v 字段应反序列化失败。
    #[test]
    fn lower_shield_rejects_missing_v() {
        let json = r#"{"type":"lower_shield"}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "lower_shield without v field should fail deserialization"
        );
    }

    /// CR#7 — RaiseShield 额外字段被拒绝（deny_unknown_fields）。
    #[test]
    fn raise_shield_rejects_extra_fields() {
        let json = r#"{"type":"raise_shield","v":1,"extra":true}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "raise_shield with extra field should fail deserialization (deny_unknown_fields); \
             got Ok variant instead"
        );
    }

    /// CR#7 — LowerShield 额外字段被拒绝（deny_unknown_fields）。
    #[test]
    fn lower_shield_rejects_extra_fields() {
        let json = r#"{"type":"lower_shield","v":1,"extra":true}"#;
        let result: Result<ClientRequestV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "lower_shield with extra field should fail deserialization (deny_unknown_fields)"
        );
    }

    /// CR#7 — 边界：raise_shield 与 lower_shield 是独立变体，互不混用。
    #[test]
    fn raise_shield_and_lower_shield_are_distinct_variants() {
        let raise: ClientRequestV1 =
            serde_json::from_str(r#"{"type":"raise_shield","v":1}"#).unwrap();
        let lower: ClientRequestV1 =
            serde_json::from_str(r#"{"type":"lower_shield","v":1}"#).unwrap();
        assert!(
            matches!(raise, ClientRequestV1::RaiseShield { .. }),
            "raise_shield must deserialize to RaiseShield variant, got {raise:?}"
        );
        assert!(
            matches!(lower, ClientRequestV1::LowerShield { .. }),
            "lower_shield must deserialize to LowerShield variant, got {lower:?}"
        );
        // They must serialize back to different JSON
        let raise_json = serde_json::to_string(&raise).unwrap();
        let lower_json = serde_json::to_string(&lower).unwrap();
        assert_ne!(
            raise_json, lower_json,
            "RaiseShield and LowerShield must produce different wire JSON; \
             raise={raise_json} lower={lower_json}"
        );
    }
}
