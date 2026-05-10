package com.bong.client;

import com.bong.client.animation.ClientAnimationBridge;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.environment.EnvironmentEffectController;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.network.AmbientZoneHandler;
import com.bong.client.network.AudioEventRouter;
import com.bong.client.network.LocustSwarmWarningHandler;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.network.VfxEventRouter;
import com.bong.client.npc.NpcMetadataHandler;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.visual.particle.BongVfxParticleBridge;
import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.RealmCollapseHudStateStore;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.UiOpenState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import com.bong.client.ui.ClientConnectionStatusStore;
import com.bong.client.ui.UiOpenScreens;
import com.bong.client.visual.VisualEffectController;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.Util;

import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;

public class BongNetworkHandler {
    public static final int EXPECTED_VERSION = ServerDataEnvelope.EXPECTED_VERSION;

    private static final ServerDataRouter ROUTER = ServerDataRouter.createDefault();
    private static final VfxEventRouter VFX_ROUTER =
        new VfxEventRouter(new ClientAnimationBridge(), new BongVfxParticleBridge());
    private static final AudioEventRouter AUDIO_ROUTER = new AudioEventRouter(SoundRecipePlayer.instance());
    private static final AmbientZoneHandler AMBIENT_ZONE_HANDLER =
        new AmbientZoneHandler(com.bong.client.audio.MusicStateMachine.instance());
    private static final LocustSwarmWarningHandler LOCUST_SWARM_WARNING_HANDLER = new LocustSwarmWarningHandler();
    private static final long UNKNOWN_LOG_THROTTLE_MS = 30_000L;
    private static final int UNKNOWN_TYPE_LOG_CACHE_LIMIT = 256;
    private static final Map<String, Long> UNKNOWN_TYPE_LOG_TIMES = new LinkedHashMap<>(16, 0.75f, true);
    private static final Map<String, Long> VFX_BRIDGE_MISS_LOG_TIMES = new LinkedHashMap<>(16, 0.75f, true);
    private static final Map<String, Long> AUDIO_BRIDGE_MISS_LOG_TIMES = new LinkedHashMap<>(16, 0.75f, true);

    public static ParseResult parseServerPayload(String jsonPayload) {
        return BongServerPayloadParser.parse(jsonPayload);
    }

    public static void register() {
        registerServerDataChannel();
        registerNpcMetadataChannel();
        registerLocustSwarmWarningChannel();
        registerVfxEventChannel();
        registerAudioPlayChannel();
        registerAudioStopChannel();
        registerAmbientZoneChannel();
        registerZoneEnvironmentChannel();
        // 旧 server 推过的 realm_collapse evac HUD 是 static volatile 字段倒计时，
        // 不会在断线 / 切服 / 重连时自清。Disconnect 时强制清掉，避免上一 server
        // 的 "域崩撤离 48s" 倒计时跨 session 续命。
        ClientPlayConnectionEvents.DISCONNECT.register(
            (handler, client) -> client.execute(() -> {
                RealmCollapseHudStateStore.clearOnDisconnect();
                NpcMetadataStore.clearAll();
                ClientConnectionStatusStore.markDisconnected(Util.getMeasuringTimeMs());
                com.bong.client.audio.MusicStateMachine.instance().clear();
            })
        );
        ClientPlayConnectionEvents.JOIN.register(
            (handler, sender, client) -> client.execute(() ->
                ClientConnectionStatusStore.markConnected(Util.getMeasuringTimeMs())
            )
        );
    }

    private static void registerNpcMetadataChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier(NpcMetadataHandler.CHANNEL_NAMESPACE, NpcMetadataHandler.CHANNEL_PATH), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            client.execute(() -> {
                boolean handled = NpcMetadataHandler.handle(jsonPayload, readableBytes);
                if (!handled) {
                    BongClient.LOGGER.warn("Ignoring bong:npc_metadata payload");
                }
            });
        });
    }

    private static void registerServerDataChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "server_data"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            ServerDataRouter.RouteResult result = ROUTER.route(jsonPayload, readableBytes);

            if (result.isParseError()) {
                BongClient.LOGGER.error("Failed to parse bong:server_data payload: {}", result.logMessage());
                return;
            }

            ServerDataDispatch dispatch = result.dispatch();
            if (dispatch == null) {
                BongClient.LOGGER.warn("Ignoring bong:server_data payload without dispatch result");
                return;
            }

            if (result.isNoOp()) {
                logNoOp(result);
                return;
            }

            BongClient.LOGGER.info("Processed bong:server_data payload: {}", result.logMessage());
            if (!dispatch.chatMessages().isEmpty()
                || dispatch.narrationState().isPresent()
                || dispatch.toastNarrationState().isPresent()
                || dispatch.legacyMessage().isPresent()
                || dispatch.playerStateViewModel().isPresent()
                || dispatch.zoneState().isPresent()
                || dispatch.visualEffectState().isPresent()
                || dispatch.alertToast().isPresent()
                || dispatch.realmCollapseHudState().isPresent()
                || dispatch.uiOpenState().isPresent()
                || dispatch.identityPanelState().isPresent()) {
                client.execute(() -> applyDispatch(client, dispatch, result.envelope().type()));
            }
        });
    }

    private static void registerLocustSwarmWarningChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "locust_swarm_warning"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            ServerDataDispatch dispatch = LOCUST_SWARM_WARNING_HANDLER.handle(jsonPayload);
            if (!dispatch.handled()) {
                BongClient.LOGGER.warn("Ignoring bong:locust_swarm_warning payload: {}", dispatch.logMessage());
                return;
            }

            BongClient.LOGGER.info("Processed bong:locust_swarm_warning payload: {}", dispatch.logMessage());
            client.execute(() -> applyDispatch(client, dispatch, "locust_swarm_warning"));
        });
    }

    private static void registerVfxEventChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "vfx_event"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            // 派发到主线程：VFX bridge 里会读 world.getPlayerByUuid + 调 PlayerAnimator，
            // 这些都是主线程约束。路由/解析可以在网络线程做，但我们索性一把送到主线程，
            // 简化并避免解析成功但 bridge 落地时 world 已经切换的竞态。
            client.execute(() -> {
                VfxEventRouter.RouteResult result = VFX_ROUTER.route(jsonPayload, readableBytes);
                if (result.isParseError()) {
                    BongClient.LOGGER.error("Failed to parse bong:vfx_event payload: {}", result.logMessage());
                    return;
                }
                if (result.isBridgeMiss()) {
                    logVfxBridgeMiss(result);
                    return;
                }
                BongClient.LOGGER.info("Processed bong:vfx_event payload: {}", result.logMessage());
            });
        });
    }

    private static void registerAudioPlayChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "audio/play"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                AudioEventRouter.RouteResult result = AUDIO_ROUTER.routePlay(jsonPayload, readableBytes);
                if (result.isParseError()) {
                    BongClient.LOGGER.error("Failed to parse bong:audio/play payload: {}", result.logMessage());
                    return;
                }
                if (result.isBridgeMiss()) {
                    logAudioBridgeMiss(result);
                    return;
                }
                BongClient.LOGGER.info("Processed bong:audio/play payload: {}", result.logMessage());
            });
        });
    }

    private static void registerAudioStopChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "audio/stop"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                AudioEventRouter.RouteResult result = AUDIO_ROUTER.routeStop(jsonPayload, readableBytes);
                if (result.isParseError()) {
                    BongClient.LOGGER.error("Failed to parse bong:audio/stop payload: {}", result.logMessage());
                    return;
                }
                if (result.isBridgeMiss()) {
                    logAudioBridgeMiss(result);
                    return;
                }
                BongClient.LOGGER.info("Processed bong:audio/stop payload: {}", result.logMessage());
            });
        });
    }

    private static void registerAmbientZoneChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "audio/ambient_zone"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                AmbientZoneHandler.RouteResult result = AMBIENT_ZONE_HANDLER.route(jsonPayload, readableBytes);
                if (result.isParseError()) {
                    BongClient.LOGGER.error("Failed to parse bong:audio/ambient_zone payload: {}", result.logMessage());
                    return;
                }
                if (result.isHandled()) {
                    BongClient.LOGGER.info("Processed bong:audio/ambient_zone payload: {}", result.logMessage());
                } else {
                    BongClient.LOGGER.debug("Ignored bong:audio/ambient_zone payload: {}", result.logMessage());
                }
            });
        });
    }

    private static void registerZoneEnvironmentChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "zone_environment"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> EnvironmentEffectController.acceptPayload(jsonPayload));
        });
    }

    private static void markConnectionPayload() {
        ClientConnectionStatusStore.markPayloadReceived(Util.getMeasuringTimeMs());
    }

    private static void applyDispatch(net.minecraft.client.MinecraftClient client, ServerDataDispatch dispatch, String envelopeType) {
        dispatch.playerStateViewModel().ifPresent(PlayerStateStore::replace);
        dispatch.seasonState().ifPresent(SeasonStateStore::replace);
        dispatch.narrationState().ifPresent(BongNetworkHandler::replaceNarrationState);
        dispatch.toastNarrationState().ifPresent(toastNarrationState -> BongToast.show(toastNarrationState, System.currentTimeMillis()));
        dispatch.zoneState().ifPresent(BongNetworkHandler::replaceZoneState);
        dispatch.visualEffectState().ifPresent(visualEffectState ->
            replaceVisualEffectState(visualEffectState, System.currentTimeMillis())
        );
        dispatch.alertToast().ifPresent(alertToast -> BongToast.show(
            alertToast.text(),
            alertToast.color(),
            System.currentTimeMillis(),
            alertToast.durationMillis()
        ));
        dispatch.realmCollapseHudState().ifPresent(RealmCollapseHudStateStore::replace);
        dispatch.uiOpenState().ifPresent(uiOpenState -> applyUiOpen(client, uiOpenState, envelopeType));
        dispatch.identityPanelState().ifPresent(IdentityPanelStateStore::replace);

        if (client.player == null) {
            return;
        }

        for (Text chatMessage : dispatch.chatMessages()) {
            client.player.sendMessage(chatMessage, false);
        }

        dispatch.legacyMessage().ifPresent(message ->
            client.player.sendMessage(Text.literal("[Bong] " + envelopeType + ": " + message), false)
        );
    }

    private static void applyUiOpen(net.minecraft.client.MinecraftClient client, UiOpenState uiOpenState, String envelopeType) {
        net.minecraft.client.gui.screen.Screen screen = UiOpenScreens.createScreen(uiOpenState);
        if (screen == null) {
            BongClient.LOGGER.warn(
                "Ignoring {} ui_open dispatch for screen '{}' because no client screen could be created",
                envelopeType,
                uiOpenState.screenId()
            );
            return;
        }

        client.setScreen(screen);
    }

    private static void replaceNarrationState(NarrationState narrationState) {
        BongHudStateSnapshot currentSnapshot = BongHudStateStore.snapshot();
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            currentSnapshot.zoneState(),
            narrationState,
            currentSnapshot.visualEffectState()
        ));
    }

    private static void replaceZoneState(ZoneState zoneState) {
        BongHudStateSnapshot currentSnapshot = BongHudStateStore.snapshot();
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            zoneState,
            currentSnapshot.narrationState(),
            currentSnapshot.visualEffectState()
        ));
    }

    private static void replaceVisualEffectState(VisualEffectState visualEffectState, long nowMillis) {
        BongHudStateSnapshot currentSnapshot = BongHudStateStore.snapshot();
        VisualEffectState nextVisualEffectState = VisualEffectController.acceptIncoming(
            currentSnapshot.visualEffectState(),
            visualEffectState,
            nowMillis,
            BongClientFeatures.ENABLE_VISUAL_EFFECTS
        );
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            currentSnapshot.zoneState(),
            currentSnapshot.narrationState(),
            nextVisualEffectState
        ));
    }

    private static void logNoOp(ServerDataRouter.RouteResult result) {
        String payloadType = result.envelope() != null ? result.envelope().type() : "unknown";
        if (shouldLogNoOp(payloadType)) {
            BongClient.LOGGER.warn("Ignoring bong:server_data payload: {}", result.logMessage());
        }
    }

    private static void logVfxBridgeMiss(VfxEventRouter.RouteResult result) {
        String payloadType = result.payload() != null ? result.payload().type() : "unknown";
        if (shouldLogVfxBridgeMiss(payloadType)) {
            BongClient.LOGGER.warn("Ignoring bong:vfx_event payload: {}", result.logMessage());
        }
    }

    private static void logAudioBridgeMiss(AudioEventRouter.RouteResult result) {
        String payloadType = result.payload() != null ? result.payload().getClass().getSimpleName() : "unknown";
        if (shouldLogAudioBridgeMiss(payloadType)) {
            BongClient.LOGGER.warn("Ignoring bong:audio payload: {}", result.logMessage());
        }
    }

    static boolean shouldLogNoOp(String payloadType) {
        return shouldLogNoOp(payloadType, System.currentTimeMillis());
    }

    static boolean shouldLogNoOp(String payloadType, long nowMillis) {
        synchronized (UNKNOWN_TYPE_LOG_TIMES) {
            pruneExpiredUnknownTypeLogTimes(nowMillis);

            Long previous = UNKNOWN_TYPE_LOG_TIMES.put(payloadType, nowMillis);
            trimUnknownTypeLogTimes();
            return previous == null || nowMillis - previous >= UNKNOWN_LOG_THROTTLE_MS;
        }
    }

    static boolean shouldLogVfxBridgeMiss(String payloadType) {
        return shouldLogVfxBridgeMiss(payloadType, System.currentTimeMillis());
    }

    static boolean shouldLogVfxBridgeMiss(String payloadType, long nowMillis) {
        synchronized (VFX_BRIDGE_MISS_LOG_TIMES) {
            pruneExpired(VFX_BRIDGE_MISS_LOG_TIMES, nowMillis);
            Long previous = VFX_BRIDGE_MISS_LOG_TIMES.put(payloadType, nowMillis);
            trimToLimit(VFX_BRIDGE_MISS_LOG_TIMES);
            return previous == null || nowMillis - previous >= UNKNOWN_LOG_THROTTLE_MS;
        }
    }

    static boolean shouldLogAudioBridgeMiss(String payloadType) {
        return shouldLogAudioBridgeMiss(payloadType, System.currentTimeMillis());
    }

    static boolean shouldLogAudioBridgeMiss(String payloadType, long nowMillis) {
        synchronized (AUDIO_BRIDGE_MISS_LOG_TIMES) {
            pruneExpired(AUDIO_BRIDGE_MISS_LOG_TIMES, nowMillis);
            Long previous = AUDIO_BRIDGE_MISS_LOG_TIMES.put(payloadType, nowMillis);
            trimToLimit(AUDIO_BRIDGE_MISS_LOG_TIMES);
            return previous == null || nowMillis - previous >= UNKNOWN_LOG_THROTTLE_MS;
        }
    }

    static void resetUnknownTypeLogTimesForTests() {
        synchronized (UNKNOWN_TYPE_LOG_TIMES) {
            UNKNOWN_TYPE_LOG_TIMES.clear();
        }
    }

    static int unknownTypeLogCacheSizeForTests() {
        synchronized (UNKNOWN_TYPE_LOG_TIMES) {
            return UNKNOWN_TYPE_LOG_TIMES.size();
        }
    }

    static int unknownTypeLogCacheLimitForTests() {
        return UNKNOWN_TYPE_LOG_CACHE_LIMIT;
    }

    public static final class ParseResult {
        public final boolean success;
        public final BongServerPayload payload;
        public final String errorMessage;

        private ParseResult(boolean success, BongServerPayload payload, String errorMessage) {
            this.success = success;
            this.payload = payload;
            this.errorMessage = errorMessage;
        }

        static ParseResult success(BongServerPayload payload) {
            return new ParseResult(true, payload, null);
        }

        static ParseResult error(String errorMessage) {
            return new ParseResult(false, null, errorMessage);
        }
    }

    private static void pruneExpiredUnknownTypeLogTimes(long nowMillis) {
        pruneExpired(UNKNOWN_TYPE_LOG_TIMES, nowMillis);
    }

    private static void trimUnknownTypeLogTimes() {
        trimToLimit(UNKNOWN_TYPE_LOG_TIMES);
    }

    private static void pruneExpired(Map<String, Long> cache, long nowMillis) {
        Iterator<Map.Entry<String, Long>> iterator = cache.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<String, Long> entry = iterator.next();
            if (nowMillis - entry.getValue() >= UNKNOWN_LOG_THROTTLE_MS) {
                iterator.remove();
            }
        }
    }

    private static void trimToLimit(Map<String, Long> cache) {
        Iterator<Map.Entry<String, Long>> iterator = cache.entrySet().iterator();
        while (cache.size() > UNKNOWN_TYPE_LOG_CACHE_LIMIT && iterator.hasNext()) {
            iterator.next();
            iterator.remove();
        }
    }

}
