package com.bong.client.network;

import com.bong.client.BongClient;
import com.bong.client.botany.BotanyHarvestMode;
import io.netty.buffer.Unpooled;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Objects;
import java.util.UUID;
import java.util.function.Supplier;

/**
 * 向服务端 {@code bong:client_request} 通道发送 CustomPayload。
 *
 * <p>默认实现使用 {@link ClientPlayNetworking}；测试通过 {@link #setBackendForTests(Backend)}
 * 注入替身捕获发送的 JSON。</p>
 */
public final class ClientRequestSender {

    /**
     * 兼容既有 payload 捕获测试的发送 seam；正常返回即视为本地传输已接受，抛异常表示拒绝。
     * 需要显式返回 false 的失败测试使用 {@link AttemptBackend}。
     */
    @FunctionalInterface
    public interface Backend {
        void send(Identifier channel, byte[] payload);
    }

    /** 可显式报告本地传输是否接受请求的测试/生产 seam。 */
    @FunctionalInterface
    public interface AttemptBackend {
        boolean trySend(Identifier channel, byte[] payload);
    }

    private static final Identifier CHANNEL = new Identifier(
        ClientRequestProtocol.CHANNEL_NAMESPACE,
        ClientRequestProtocol.CHANNEL_PATH
    );

    private static final AttemptBackend DEFAULT_BACKEND = (channel, payload) -> {
        // 不能用 ClientPlayNetworking.canSend()：Bong server 是 Valence，未走 Fabric
        // minecraft:register 协商；Fabric 的 send() 会在已有 play network handler 时直接
        // enqueue CustomPayload，未连接则抛 IllegalStateException。tryDispatch() 统一捕获异常，
        // 因而 true 的精确定义是“本地 play transport 已接受”，不是伪造 server ACK。
        PacketByteBuf buf = new PacketByteBuf(Unpooled.buffer(payload.length));
        buf.writeBytes(payload);
        ClientPlayNetworking.send(channel, buf);
        return true;
    };

    private static volatile AttemptBackend backend = DEFAULT_BACKEND;
    private static final Supplier<String> DEFAULT_REQUEST_ID_SUPPLIER =
        () -> UUID.randomUUID().toString();
    private static volatile Supplier<String> requestIdSupplier = DEFAULT_REQUEST_ID_SUPPLIER;

    private ClientRequestSender() {}

    public static void sendSetMeridianTarget(ClientRequestProtocol.MeridianId meridian) {
        dispatch(ClientRequestProtocol.encodeSetMeridianTarget(meridian));
    }

    public static void sendBreakthroughRequest() {
        dispatch(ClientRequestProtocol.encodeBreakthroughRequest());
    }

    public static void sendStartDuXuRequest() {
        dispatch(ClientRequestProtocol.encodeStartDuXuRequest());
    }

    public static void sendAbortTribulationRequest() {
        dispatch(ClientRequestProtocol.encodeAbortTribulationRequest());
    }

    public static void sendVoidActionSuppressTsy(String zoneId) {
        dispatch(ClientRequestProtocol.encodeVoidActionSuppressTsy(zoneId));
    }

    public static void sendVoidActionExplodeZone(String zoneId) {
        dispatch(ClientRequestProtocol.encodeVoidActionExplodeZone(zoneId));
    }

    public static void sendVoidActionBarrier(String zoneId, double centerX, double centerY, double centerZ, double radius) {
        dispatch(ClientRequestProtocol.encodeVoidActionBarrier(zoneId, centerX, centerY, centerZ, radius));
    }

    public static void sendVoidActionLegacyAssign(String inheritorId, List<Long> itemInstanceIds, String message) {
        dispatch(ClientRequestProtocol.encodeVoidActionLegacyAssign(inheritorId, itemInstanceIds, message));
    }

    public static void sendHeartDemonDecision(Integer chosenIdx) {
        dispatch(ClientRequestProtocol.encodeHeartDemonDecision(chosenIdx));
    }

    /** 顿悟决定：{@code chosenIdx = null} 表示拒绝或超时。 */
    public static void sendInsightDecision(String triggerId, Integer chosenIdx) {
        dispatch(ClientRequestProtocol.encodeInsightDecision(triggerId, chosenIdx));
    }

    public static void sendForgeRequest(
        ClientRequestProtocol.MeridianId meridian,
        ClientRequestProtocol.ForgeAxis axis
    ) {
        dispatch(ClientRequestProtocol.encodeForgeRequest(meridian, axis));
    }

    public static void sendBotanyHarvestRequest(String sessionId, BotanyHarvestMode mode) {
        dispatch(ClientRequestProtocol.encodeBotanyHarvestRequest(sessionId, mode));
    }

    /** Combat UI 系列 C2S 通用发送入口。 */
    public static void send(String type, com.google.gson.JsonObject payload) {
        dispatch(ClientRequestProtocol.encodeGeneric(type, payload));
    }

    /** 终结屏：请求服务端创建新角色。 */
    public static void sendCombatCreateNewCharacter() {
        dispatch(ClientRequestProtocol.encodeCombatCreateNewCharacter());
    }

    // ─── Inventory move intent ──────────────────────────────────────────────

    /**
     * plan-rotate-v1 — {@code rotated} 透传拖拽中的 R 键旋转状态；
     * 非网格目标（装备槽 / hotbar / 丢弃等）恒传 false。
     */
    public static boolean sendInventoryMove(
        long instanceId,
        ClientRequestProtocol.InvLocation from,
        ClientRequestProtocol.InvLocation to,
        boolean rotated
    ) {
        return tryDispatch(ClientRequestProtocol.encodeInventoryMove(instanceId, from, to, rotated));
    }

    public static void sendEquipFalseSkin(long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeEquipFalseSkin(itemInstanceId));
    }

    public static void sendForgeFalseSkin(ClientRequestProtocol.FalseSkinKind kind) {
        dispatch(ClientRequestProtocol.encodeForgeFalseSkin(kind));
    }

    public static void sendPickupDroppedItem(long instanceId) {
        dispatch(ClientRequestProtocol.encodePickupDroppedItem(instanceId));
    }

    /** plan-remains-suite P0 — 遗骸 G 键统一交互。 */
    public static void sendRemainsLoot(String remainsId) {
        dispatch(ClientRequestProtocol.encodeRemainsLoot(remainsId));
    }

    public static void sendMineralProbe(int x, int y, int z) {
        dispatch(ClientRequestProtocol.encodeMineralProbe(x, y, z));
    }

    /**
     * plan-exploration-probe-return-v1 P1 — 神识感知保鲜 C2S 请求。
     *
     * @param instanceId 被查询物品的 inventory instance_id（来自 InventoryItem.getInstanceId()）
     */
    public static void sendFreshnessProbe(long instanceId) {
        dispatch(ClientRequestProtocol.encodeFreshnessProbe(instanceId));
    }

    public static void sendInventoryDiscardItem(long instanceId, ClientRequestProtocol.InvLocation from) {
        dispatch(ClientRequestProtocol.encodeInventoryDiscardItem(instanceId, from));
    }

    public static void sendDropWeapon(long instanceId, ClientRequestProtocol.InvLocation from) {
        dispatch(ClientRequestProtocol.encodeDropWeapon(instanceId, from));
    }

    public static void sendRepairWeapon(long instanceId, int x, int y, int z) {
        dispatch(ClientRequestProtocol.encodeRepairWeapon(instanceId, x, y, z));
    }

    public static void sendForgeStationPlace(int x, int y, int z, long itemInstanceId, int stationTier) {
        dispatch(ClientRequestProtocol.encodeForgeStationPlace(x, y, z, itemInstanceId, stationTier));
    }

    public static void sendBlockPlace(
        net.minecraft.util.math.BlockPos pos,
        long itemInstanceId,
        ClientRequestProtocol.ZhenfaTargetFace targetFace
    ) {
        dispatch(ClientRequestProtocol.encodeBlockPlace(pos, itemInstanceId, targetFace));
    }

    public static void sendSpiritNichePlace(int x, int y, int z, long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeSpiritNichePlace(x, y, z, itemInstanceId));
    }

    public static void sendSpiritNicheRepair(int x, int y, int z, long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeSpiritNicheRepair(x, y, z, itemInstanceId));
    }

    public static void sendSpiritNicheGaze(int x, int y, int z) {
        dispatch(ClientRequestProtocol.encodeSpiritNicheGaze(x, y, z));
    }

    public static void sendSpiritNicheMarkCoordinate(int x, int y, int z) {
        dispatch(ClientRequestProtocol.encodeSpiritNicheMarkCoordinate(x, y, z));
    }

    public static void sendSpiritNicheActivateGuardian(
        int x,
        int y,
        int z,
        String guardianKind,
        java.util.List<String> materials
    ) {
        dispatch(ClientRequestProtocol.encodeSpiritNicheActivateGuardian(x, y, z, guardianKind, materials));
    }

    public static void sendZhenfaPlace(
        BlockPos pos,
        ClientRequestProtocol.ZhenfaKind kind,
        ClientRequestProtocol.ZhenfaCarrierKind carrier,
        double qiInvestRatio,
        String trigger
    ) {
        dispatch(ClientRequestProtocol.encodeZhenfaPlace(pos, kind, carrier, qiInvestRatio, trigger));
    }

    public static void sendZhenfaPlace(
        BlockPos pos,
        ClientRequestProtocol.ZhenfaKind kind,
        ClientRequestProtocol.ZhenfaCarrierKind carrier,
        double qiInvestRatio,
        String trigger,
        Long itemInstanceId,
        ClientRequestProtocol.ZhenfaTargetFace targetFace
    ) {
        dispatch(ClientRequestProtocol.encodeZhenfaPlace(
            pos,
            kind,
            carrier,
            qiInvestRatio,
            trigger,
            itemInstanceId,
            targetFace
        ));
    }

    public static void sendZhenfaTrigger(Long instanceId) {
        dispatch(ClientRequestProtocol.encodeZhenfaTrigger(instanceId));
    }

    // plan-layered-equip-v1 P4（决议 #8）— 法宝激活/卸下到灵宝 UI 触发位。
    public static void sendTreasureActivate(long instanceId, boolean activate) {
        dispatch(ClientRequestProtocol.encodeTreasureActivate(instanceId, activate));
    }

    public static void sendZhenfaDisarm(BlockPos pos, ClientRequestProtocol.ZhenfaDisarmMode mode) {
        dispatch(ClientRequestProtocol.encodeZhenfaDisarm(pos, mode));
    }

    public static void sendQiScatterBeadUse(long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeQiScatterBeadUse(itemInstanceId));
    }

    public static void sendQiScatterBeadUse(long itemInstanceId, BlockPos buryPos) {
        dispatch(ClientRequestProtocol.encodeQiScatterBeadUse(itemInstanceId, buryPos));
    }

    public static void sendSparringInviteResponse(String inviteId, boolean accepted, boolean timedOut) {
        dispatch(ClientRequestProtocol.encodeSparringInviteResponse(inviteId, accepted, timedOut));
    }

    public static void sendTradeOfferRequest(String target, long offeredInstanceId) {
        dispatch(ClientRequestProtocol.encodeTradeOfferRequest(target, offeredInstanceId));
    }

    public static void sendTradeOfferResponse(String offerId, boolean accepted, Long requestedInstanceId) {
        dispatch(ClientRequestProtocol.encodeTradeOfferResponse(offerId, accepted, requestedInstanceId));
    }

    public static void sendNpcInspectRequest(int npcEntityId) {
        dispatch(ClientRequestProtocol.encodeNpcInspectRequest(npcEntityId));
    }

    public static void sendNpcDialogueChoice(int npcEntityId, String optionId) {
        dispatch(ClientRequestProtocol.encodeNpcDialogueChoice(npcEntityId, optionId));
    }

    public static void sendNpcTradeRequest(int npcEntityId, List<Long> offeredItems, String requestedItemId) {
        dispatch(ClientRequestProtocol.encodeNpcTradeRequest(npcEntityId, offeredItems, requestedItemId));
    }

    // ─── plan-forge-session-entry-wiring-v1 §4.1#2/#3：起炉 / 图谱翻页 C2S ────────────

    /**
     * plan-forge-session-entry-wiring-v1 §4.1#3 —— 起炉请求。station 用坐标寻址
     * （对齐 {@code sendAlchemyOpenFurnace(BlockPos)} 的 BlockPos 参数形状）。
     */
    public static void sendForgeStartSession(
        BlockPos stationPos,
        String blueprintId,
        List<ClientRequestProtocol.ForgeMaterial> materials
    ) {
        dispatch(ClientRequestProtocol.encodeForgeStartSession(stationPos, blueprintId, materials));
    }

    /** plan-forge-session-entry-wiring-v1 §4.1#2 —— 图谱书翻页请求，server 权威页码。 */
    public static void sendForgeBlueprintTurnPage(int delta) {
        dispatch(ClientRequestProtocol.encodeForgeBlueprintTurnPage(delta));
    }

    public static void sendForgeTemperingHit(
        long sessionId,
        ClientRequestProtocol.TemperBeat beat,
        int ticksRemaining
    ) {
        dispatch(ClientRequestProtocol.encodeForgeTemperingHit(sessionId, beat, ticksRemaining));
    }

    public static void sendForgeInscriptionScroll(long sessionId, String inscriptionId) {
        dispatch(ClientRequestProtocol.encodeForgeInscriptionScroll(sessionId, inscriptionId));
    }

    public static void sendForgeConsecrationInject(long sessionId, double qiAmount) {
        dispatch(ClientRequestProtocol.encodeForgeConsecrationInject(sessionId, qiAmount));
    }

    // ─── HUD combat intents (plan-HUD-v1 §11.3) ─────────────────────────────

    public static void sendUseQuickSlot(int slot) {
        dispatch(ClientRequestProtocol.encodeUseQuickSlot(slot));
    }

    public static void sendSelfAntidote(long instanceId) {
        dispatch(ClientRequestProtocol.encodeSelfAntidote(instanceId));
    }

    public static boolean sendQuickSlotBind(int slot, String itemId) {
        return sendQuickSlotBindTracked(slot, itemId) != null;
    }

    /** 返回本地 transport 接受的唯一 request_id；拒绝时返回 null。 */
    public static String sendQuickSlotBindTracked(int slot, String itemId) {
        String requestId = Objects.requireNonNull(requestIdSupplier.get(), "requestId");
        if (requestId.isBlank()) {
            throw new IllegalStateException("quick-slot requestId must not be blank");
        }
        return tryDispatch(ClientRequestProtocol.encodeQuickSlotBind(slot, itemId, requestId))
            ? requestId
            : null;
    }

    public static void sendSkillBarCast(int slot) {
        dispatch(ClientRequestProtocol.encodeSkillBarCast(slot));
    }

    public static void sendSkillBarCast(int slot, String target) {
        dispatch(ClientRequestProtocol.encodeSkillBarCast(slot, target));
    }

    public static void sendSkillBarBindClear(int slot) {
        dispatch(ClientRequestProtocol.encodeSkillBarBindClear(slot));
    }

    public static void sendSkillBarBindSkill(int slot, String skillId) {
        dispatch(ClientRequestProtocol.encodeSkillBarBindSkill(slot, skillId));
    }

    public static void sendSkillBarBindItem(int slot, String templateId) {
        dispatch(ClientRequestProtocol.encodeSkillBarBindItem(slot, templateId));
    }

    public static void sendSkillConfigIntent(String skillId, com.google.gson.JsonObject config) {
        dispatch(ClientRequestProtocol.encodeSkillConfigIntent(skillId, config));
    }

    public static void sendChargeCarrier(String slot, double qiTarget) {
        dispatch(ClientRequestProtocol.encodeChargeCarrier(slot, qiTarget));
    }

    public static void sendThrowCarrier(String slot, double x, double y, double z, double power) {
        dispatch(ClientRequestProtocol.encodeThrowCarrier(slot, x, y, z, power));
    }

    public static void sendAnqiContainerSwitch() {
        dispatch(ClientRequestProtocol.encodeAnqiContainerSwitch());
    }

    public static void sendAnqiContainerSwitch(ClientRequestProtocol.AnqiContainerKind to) {
        dispatch(ClientRequestProtocol.encodeAnqiContainerSwitch(to));
    }

    public static void sendJiemai() {
        dispatch(ClientRequestProtocol.encodeJiemai());
    }

    public static void sendMovementAction(ClientRequestProtocol.MovementAction action) {
        dispatch(ClientRequestProtocol.encodeMovementAction(action));
    }

    public static void sendMovementAction(ClientRequestProtocol.MovementAction action, double yawDegrees) {
        dispatch(ClientRequestProtocol.encodeMovementAction(action, yawDegrees));
    }

    public static void sendQiColorInspect(String observed) {
        dispatch(ClientRequestProtocol.encodeQiColorInspect(observed));
    }

    public static void sendStartExtract(long portalEntityId) {
        dispatch(ClientRequestProtocol.encodeStartExtractRequest(portalEntityId));
    }

    public static void sendCancelExtract() {
        dispatch(ClientRequestProtocol.encodeCancelExtractRequest());
    }

    public static void sendStartSearch(long containerEntityId) {
        dispatch(ClientRequestProtocol.encodeStartSearch(containerEntityId));
    }

    public static void sendCancelSearch() {
        dispatch(ClientRequestProtocol.encodeCancelSearch());
    }

    // ─── 炼丹 (plan-alchemy-v1 §4) ──────────────────────────────────────────

    public static void sendAlchemyTurnPage(int delta) {
        dispatch(ClientRequestProtocol.encodeAlchemyTurnPage(delta));
    }

    public static void sendAlchemyLearnRecipe(String recipeId) {
        dispatch(ClientRequestProtocol.encodeAlchemyLearnRecipe(recipeId));
    }

    public static void sendAlchemyOpenFurnace(BlockPos pos) {
        dispatch(ClientRequestProtocol.encodeAlchemyOpenFurnace(pos));
    }

    public static void sendAlchemyIgnite(BlockPos pos, String recipeId) {
        dispatch(ClientRequestProtocol.encodeAlchemyIgnite(pos, recipeId));
    }

    public static void sendAlchemyFeedSlot(BlockPos pos, int slotIdx, String material, int count) {
        dispatch(ClientRequestProtocol.encodeAlchemyFeedSlot(pos, slotIdx, material, count));
    }

    public static void sendAlchemyTakeBack(BlockPos pos, int slotIdx) {
        dispatch(ClientRequestProtocol.encodeAlchemyTakeBack(pos, slotIdx));
    }

    public static void sendAlchemyInjectQi(BlockPos pos, double qi) {
        dispatch(ClientRequestProtocol.encodeAlchemyInjectQi(pos, qi));
    }

    public static void sendAlchemyAdjustTemp(BlockPos pos, double temp) {
        dispatch(ClientRequestProtocol.encodeAlchemyAdjustTemp(pos, temp));
    }

    public static void sendAlchemyFurnacePlace(BlockPos pos, long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeAlchemyFurnacePlace(pos, itemInstanceId));
    }

    public static void sendCoffinOpen(BlockPos pos) {
        dispatch(ClientRequestProtocol.encodeCoffinOpen(pos));
    }

    public static void sendCoffinPlace(BlockPos pos, long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeCoffinPlace(pos, itemInstanceId));
    }

    public static void sendCoffinEnter(BlockPos pos) {
        dispatch(ClientRequestProtocol.encodeCoffinEnter(pos));
    }

    public static void sendCoffinLeave() {
        dispatch(ClientRequestProtocol.encodeCoffinLeave());
    }

    // ─── plan-coffin-tiers-v1 P3：延寿棺 marker 交互 C2S ───────────────────

    /** plan-coffin-tiers-v1 P3 — 左键攻击 marker 实体，破坏延寿棺（体验同破坏方块）。 */
    public static void sendCoffinBreak(BlockPos pos) {
        dispatch(ClientRequestProtocol.encodeCoffinBreak(pos));
    }

    /** plan-coffin-tiers-v1 P3 — G 菜单 [回收] 按钮，主动回收延寿棺（较全材料返还）。 */
    public static void sendCoffinMenuReclaim(BlockPos pos) {
        dispatch(ClientRequestProtocol.encodeCoffinMenuReclaim(pos));
    }

    public static void sendAlchemyTakePill(String pillItemId) {
        dispatch(ClientRequestProtocol.encodeAlchemyTakePill(pillItemId));
    }

    public static void sendApplyPill(long instanceId, ClientRequestProtocol.ApplyPillTarget target) {
        dispatch(ClientRequestProtocol.encodeApplyPill(instanceId, target));
    }

    public static void sendApplyPillSelf(long instanceId) {
        dispatch(ClientRequestProtocol.encodeApplyPillSelf(instanceId));
    }

    public static void sendDuoSheRequest(String targetId) {
        dispatch(ClientRequestProtocol.encodeDuoSheRequest(targetId));
    }

    public static void sendUseLifeCore(long instanceId) {
        dispatch(ClientRequestProtocol.encodeUseLifeCore(instanceId));
    }

    public static void sendLearnSkillScroll(long instanceId) {
        dispatch(ClientRequestProtocol.encodeLearnSkillScroll(instanceId));
    }

    public static void sendTechniqueScrollUse(long instanceId) {
        dispatch(ClientRequestProtocol.encodeTechniqueScrollUse(instanceId));
    }

    // ─── plan-dying-elder-v1 P3：垂死大能交互 C2S ────────────────────────────

    /**
     * plan-dying-elder-v1 P3 — 向垂死大能交付回元丹。
     * 守恒：server 校验背包持有 pill_instance_id，qi_gain 走 QiTransfer{TradeDan}，不旁路 ledger。
     *
     * @param pillInstanceId 玩家背包中回元丹的 inventory instance_id
     * @param elderEntityId  垂死大能的 MC protocol entity_id（i32）
     */
    public static void sendGiveDanToElder(long pillInstanceId, int elderEntityId) {
        dispatch(ClientRequestProtocol.encodeGiveDanToElder(pillInstanceId, elderEntityId));
    }

    // ─── 灵田 (plan-lingtian-v1 §1.2-§1.7) ──────────────────────────────────

    public static void sendLingtianStartTill(int x, int y, int z, long hoeInstanceId, String mode) {
        dispatch(ClientRequestProtocol.encodeLingtianStartTill(x, y, z, hoeInstanceId, mode));
    }

    public static void sendLingtianStartRenew(int x, int y, int z, long hoeInstanceId) {
        dispatch(ClientRequestProtocol.encodeLingtianStartRenew(x, y, z, hoeInstanceId));
    }

    public static void sendLingtianStartPlanting(int x, int y, int z, String plantId) {
        dispatch(ClientRequestProtocol.encodeLingtianStartPlanting(x, y, z, plantId));
    }

    public static void sendLingtianStartHarvest(int x, int y, int z, String mode) {
        dispatch(ClientRequestProtocol.encodeLingtianStartHarvest(x, y, z, mode));
    }

    public static void sendLingtianStartReplenish(int x, int y, int z, String source) {
        dispatch(ClientRequestProtocol.encodeLingtianStartReplenish(x, y, z, source));
    }

    public static void sendLingtianStartDrainQi(int x, int y, int z) {
        dispatch(ClientRequestProtocol.encodeLingtianStartDrainQi(x, y, z));
    }

    // ─── 通用手搓 (plan-craft-v1 P2) ────────────────────────────────────────

    /** plan-craft-v1 §2 — 玩家点 [开始手搓]。 */
    public static void sendCraftStart(String recipeId) {
        dispatch(ClientRequestProtocol.encodeCraftStart(recipeId));
    }

    /** plan-craft-ux-v1 P2 — 带数量的起手请求。 */
    public static void sendCraftStart(String recipeId, int quantity) {
        dispatch(ClientRequestProtocol.encodeCraftStart(recipeId, quantity));
    }

    /** plan-craft-v1 §5 决策门 #3 — 取消进行中的 craft session（70% 材料返还，qi 不退）。 */
    public static void sendCraftCancel() {
        dispatch(ClientRequestProtocol.encodeCraftCancel());
    }

    // ─── plan-supply-coffin-loot-ui P2：supply coffin open (entity-based) ──

    public static void sendSupplyCoffinOpen(int entityId) {
        dispatch(ClientRequestProtocol.encodeSupplyCoffinOpen(entityId));
    }

    public static void sendContainerOpen(int entityId) {
        dispatch(ClientRequestProtocol.encodeContainerOpen(entityId));
    }

    public static void sendWorkbenchOpen(int entityId) {
        dispatch(ClientRequestProtocol.encodeWorkbenchOpen(entityId));
    }

    // ─── plan-supply-coffin-loot-ui P1：外部容器 C2S ──────────────

    public static void sendExternalContainerMove(
        long sessionId, long instanceId,
        ClientRequestProtocol.InvLocation from,
        ClientRequestProtocol.InvLocation to
    ) {
        dispatch(ClientRequestProtocol.encodeExternalContainerMove(sessionId, instanceId, from, to));
    }

    public static void sendExternalContainerClose(long sessionId) {
        dispatch(ClientRequestProtocol.encodeExternalContainerClose(sessionId));
    }

    // ─── plan-shield-block-v1 P1：盾牌举盾 / 放盾 C2S ──────────────────────

    /** plan-shield-block-v1 P1 — 通知 server 玩家举盾（对应 ClientRequestV1::RaiseShield）。 */
    public static void sendRaiseShield() {
        dispatch(ClientRequestProtocol.encodeRaiseShield());
    }

    /** plan-shield-block-v1 P1 — 通知 server 玩家放盾（对应 ClientRequestV1::LowerShield）。 */
    public static void sendLowerShield() {
        dispatch(ClientRequestProtocol.encodeLowerShield());
    }

    /**
     * plan-scroll-reading-v1 P0 — 通知 server 玩家请求阅读一本可阅读残卷
     * （对应 ClientRequestV1::ScrollReadRequest）。
     *
     * @param instanceId 待阅读残卷的 inventory instance_id
     */
    public static void sendScrollReadRequest(long instanceId) {
        dispatch(ClientRequestProtocol.encodeScrollReadRequest(instanceId));
    }

    /** plan-scroll-reading-v1 P1 — 通知 server 玩家关闭阅读屏（对应 ClientRequestV1::ScrollReadClosed）。 */
    public static boolean sendScrollReadClosed() {
        return tryDispatch(ClientRequestProtocol.encodeScrollReadClosed());
    }

    // ─── plan-worldgen-v4 P5 §8.1#5：画廊审阅 owo 方块面板 dev-only give-block C2S ──

    /**
     * plan-worldgen-v4 P5 §8.1#5 — dev-only 方块面板点击某 vanilla 方块后发送 give-block 请求。
     *
     * <p>对应 server {@code ClientRequestV1::BlockPickerGive}（dev/creative 门控，把
     * {@code vanilla:<block_id>} 模板物品放进背包，**非生产 gameplay**）。</p>
     *
     * @param blockId 不含 namespace 的 vanilla 方块短名（如 {@code "stone_bricks"}）
     * @param count   给予数量，1..=64
     */
    public static void sendBlockPickerGive(String blockId, int count) {
        dispatch(ClientRequestProtocol.encodeBlockPickerGive(blockId, count));
    }

    // ─── plan-agent-ui-data-v1 P1：天道 UI 响应 C2S ─────────────────────────

    /**
     * plan-agent-ui-data-v1 P1 — 向 server 发送天道 UI 面板交互响应。
     *
     * @param requestId  server 下发的 request_id
     * @param action     动作字面量（"button_click" / "dismissed" / "parse_error" 等）
     * @param params     附加参数（如 button_click 时的 {@code {"button_id":"enter_realm"}}）
     */
    public static void sendAgentUiResponse(
        String requestId,
        String action,
        java.util.Map<String, String> params
    ) {
        dispatch(ClientRequestProtocol.encodeAgentUiResponse(requestId, action, params));
    }

    private static void dispatch(String json) {
        if (!tryDispatch(json)) {
            throw new IllegalStateException("client request transport rejected payload");
        }
    }

    private static boolean tryDispatch(String json) {
        try {
            return backend.trySend(CHANNEL, json.getBytes(StandardCharsets.UTF_8));
        } catch (RuntimeException error) {
            BongClient.LOGGER.warn(
                "[bong][client_request] local transport rejected payload_bytes={}",
                json.getBytes(StandardCharsets.UTF_8).length,
                error
            );
            return false;
        }
    }

    public static void setBackendForTests(Backend b) {
        Backend accepted = Objects.requireNonNull(b, "backend");
        backend = (channel, payload) -> {
            accepted.send(channel, payload);
            return true;
        };
    }

    public static void setAttemptBackendForTests(AttemptBackend b) {
        backend = Objects.requireNonNull(b, "backend");
    }

    public static void setRequestIdSupplierForTests(Supplier<String> supplier) {
        requestIdSupplier = Objects.requireNonNull(supplier, "supplier");
    }

    public static void resetBackendForTests() {
        backend = DEFAULT_BACKEND;
        requestIdSupplier = DEFAULT_REQUEST_ID_SUPPLIER;
    }
}
