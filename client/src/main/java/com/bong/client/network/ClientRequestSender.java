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

/**
 * 向服务端 {@code bong:client_request} 通道发送 CustomPayload。
 *
 * <p>默认实现使用 {@link ClientPlayNetworking}；测试通过 {@link #setBackendForTests(Backend)}
 * 注入替身捕获发送的 JSON。</p>
 */
public final class ClientRequestSender {

    /** 可测试的发送后端 seam。 */
    @FunctionalInterface
    public interface Backend {
        void send(Identifier channel, byte[] payload);
    }

    private static final Identifier CHANNEL = new Identifier(
        ClientRequestProtocol.CHANNEL_NAMESPACE,
        ClientRequestProtocol.CHANNEL_PATH
    );

    private static final Backend DEFAULT_BACKEND = (channel, payload) -> {
        // 注意：不能用 ClientPlayNetworking.canSend() —— 它只对 Fabric 注册过通道的
        // server 返 true，而 Bong server 是定制 Valence，没走 Fabric `minecraft:register`
        // 协商。但 Valence 实际接收任何 channel 的 CustomPayload，所以 force send。
        // canSend 失败时 vanilla MC 会丢包（unregistered_channel），不会崩，最差是
        // server 端的 client_request_handler 没看到 packet —— 与 canSend 拒发等价。
        PacketByteBuf buf = new PacketByteBuf(Unpooled.buffer(payload.length));
        buf.writeBytes(payload);
        ClientPlayNetworking.send(channel, buf);
    };

    private static volatile Backend backend = DEFAULT_BACKEND;

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

    // ─── Inventory move intent ──────────────────────────────────────────────

    public static void sendInventoryMove(
        long instanceId,
        ClientRequestProtocol.InvLocation from,
        ClientRequestProtocol.InvLocation to
    ) {
        dispatch(ClientRequestProtocol.encodeInventoryMove(instanceId, from, to));
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

    public static void sendMineralProbe(int x, int y, int z) {
        dispatch(ClientRequestProtocol.encodeMineralProbe(x, y, z));
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

    public static void sendSpiritNichePlace(int x, int y, int z, long itemInstanceId) {
        dispatch(ClientRequestProtocol.encodeSpiritNichePlace(x, y, z, itemInstanceId));
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

    public static void sendZhenfaTrigger(Long instanceId) {
        dispatch(ClientRequestProtocol.encodeZhenfaTrigger(instanceId));
    }

    public static void sendZhenfaDisarm(BlockPos pos, ClientRequestProtocol.ZhenfaDisarmMode mode) {
        dispatch(ClientRequestProtocol.encodeZhenfaDisarm(pos, mode));
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

    public static void sendQuickSlotBind(int slot, String itemId) {
        dispatch(ClientRequestProtocol.encodeQuickSlotBind(slot, itemId));
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

    private static void dispatch(String json) {
        backend.send(CHANNEL, json.getBytes(StandardCharsets.UTF_8));
    }

    public static void setBackendForTests(Backend b) {
        backend = b;
    }

    public static void resetBackendForTests() {
        backend = DEFAULT_BACKEND;
    }
}
