package com.bong.client.network;

import com.bong.client.BongClient;
import com.bong.client.botany.BotanyHarvestMode;
import io.netty.buffer.Unpooled;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;

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

    // ─── HUD combat intents (plan-HUD-v1 §11.3) ─────────────────────────────

    public static void sendUseQuickSlot(int slot) {
        dispatch(ClientRequestProtocol.encodeUseQuickSlot(slot));
    }

    public static void sendQuickSlotBind(int slot, String itemId) {
        dispatch(ClientRequestProtocol.encodeQuickSlotBind(slot, itemId));
    }

    public static void sendJiemai() {
        dispatch(ClientRequestProtocol.encodeJiemai());
    }

    // ─── 炼丹 (plan-alchemy-v1 §4) ──────────────────────────────────────────

    public static void sendAlchemyTurnPage(int delta) {
        dispatch(ClientRequestProtocol.encodeAlchemyTurnPage(delta));
    }

    public static void sendAlchemyLearnRecipe(String recipeId) {
        dispatch(ClientRequestProtocol.encodeAlchemyLearnRecipe(recipeId));
    }

    public static void sendAlchemyOpenFurnace(String furnaceId) {
        dispatch(ClientRequestProtocol.encodeAlchemyOpenFurnace(furnaceId));
    }

    public static void sendAlchemyIgnite(String recipeId) {
        dispatch(ClientRequestProtocol.encodeAlchemyIgnite(recipeId));
    }

    public static void sendAlchemyFeedSlot(int slotIdx, String material, int count) {
        dispatch(ClientRequestProtocol.encodeAlchemyFeedSlot(slotIdx, material, count));
    }

    public static void sendAlchemyTakeBack(int slotIdx) {
        dispatch(ClientRequestProtocol.encodeAlchemyTakeBack(slotIdx));
    }

    public static void sendAlchemyInjectQi(double qi) {
        dispatch(ClientRequestProtocol.encodeAlchemyInjectQi(qi));
    }

    public static void sendAlchemyAdjustTemp(double temp) {
        dispatch(ClientRequestProtocol.encodeAlchemyAdjustTemp(temp));
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

    public static void sendLearnSkillScroll(long instanceId) {
        dispatch(ClientRequestProtocol.encodeLearnSkillScroll(instanceId));
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
