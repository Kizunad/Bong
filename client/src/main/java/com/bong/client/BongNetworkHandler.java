package com.bong.client;

import com.bong.client.animation.ClientAnimationBridge;
import com.bong.client.fauna.FaunaActionBridge;
import com.bong.client.daozhan.DaoZhanDisguiseHandler;
import com.bong.client.spider.SpiderDisguiseHandler;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.craft.CraftStore;
import com.bong.client.dandao.MutationPayloadHandler;
import com.bong.client.dandao.MutationVisualState;
import com.bong.client.environment.EnvironmentEffectController;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.network.AmbientZoneHandler;
import com.bong.client.network.AudioEventRouter;
import com.bong.client.network.LocustSwarmWarningHandler;
import com.bong.client.network.QiAttritionPayload;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.network.VfxEventRouter;
import com.bong.client.npc.NpcLodHandler;
import com.bong.client.npc.NpcLodStore;
import com.bong.client.npc.NpcMetadataHandler;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.npc.NpcBubbleHandler;
import com.bong.client.npc.NpcDialogueBubbleRenderer;
import com.bong.client.npc.NpcMoodHandler;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.visual.particle.BongVfxParticleBridge;
import com.bong.client.visual.particle.QiAttritionVfxPlayer;
import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.RealmCollapseHudStateStore;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.UiOpenState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import com.bong.client.tiandao.TiandaoPresencePayloadHandler;
import com.bong.client.tsy.TsyBossHealthHandler;
import com.bong.client.tsy.TsyBossHealthStore;
import com.bong.client.tsy.TsyDeathVfxHandler;
import com.bong.client.tsy.TsyDeathVfxStore;
import com.bong.client.ui.ClientConnectionStatusStore;
import com.bong.client.ui.UiOpenScreens;
import com.bong.client.visual.VisualEffectController;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.Util;

import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.BiConsumer;
import java.util.function.Consumer;
import java.util.function.LongSupplier;

public class BongNetworkHandler {
    public static final int EXPECTED_VERSION = ServerDataEnvelope.EXPECTED_VERSION;

    private static final ServerDataRouter ROUTER = ServerDataRouter.createDefault();
    private static final VfxEventRouter VFX_ROUTER =
        new VfxEventRouter(
            new ClientAnimationBridge(),
            new BongVfxParticleBridge(),
            new FaunaActionBridge());
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
        registerNpcLodChannel();
        registerNpcBubbleChannel();
        registerNpcMoodChannel();
        registerTsyBossHealthChannel();
        registerTsyDeathVfxChannel();
        registerLocustSwarmWarningChannel();
        registerVfxEventChannel();
        registerQiAttritionChannel();
        registerAudioPlayChannel();
        registerAudioStopChannel();
        registerTiandaoPresenceChannel();
        registerAmbientZoneChannel();
        registerZoneEnvironmentChannel();
        registerMutationVisualChannel();
        // plan-combat-skill-feedback-bridges-v1 P1 — baomai_v4 反馈整桥
        registerCrackReadingChannel();
        registerResonanceLockChannel();
        registerResonanceLockEndChannel();
        // plan-combat-skill-feedback-bridges-v1 P3 — 我流虚蚀视觉同步
        registerVoidErosionVisualChannel();
        // plan-fauna-mimic-spider-v1 P2 — 拟态蛛伪装渲染 CustomPayload
        registerSpiderDisguiseEnterChannel();
        registerSpiderAmbushTriggerChannel();
        // plan-daozhan-v1 P1 — 道伥伪装渲染 CustomPayload
        registerDaoZhanDisguiseEnterChannel();
        registerDaoZhanRevealChannel();
        // plan-fauna-stitched-beast-v1 P3 — 兽核吸收幻觉 HUD
        registerCoreAbsorptionHallucinationChannel();
        // plan-dying-elder-v1 P3 — 垂死大能遭遇 HUD
        registerElderEncounterChannel();
        // plan-era-state-v1 P3 — 时代天象 S2C CustomPayload（realm gate 由 server 执行）
        registerEraAmbianceChannel();
        // plan-agent-ui-data-v1 P1 — 天道动态 UI 面板（专属 JSON channel，绕开 proto 路径）
        registerAgentUiChannels();
        // plan-halfstep-rechallenge-integration-v1 P0 — 半步化虚重渡触发 HUD（专属 JSON channel）
        registerHalfStepRechallengeChannel();
        // plan-era-state-v1 M2 — EraAmbianceState 渐变计数器每游戏 tick 推进（非渲染帧）。
        // 必须在游戏 tick 而非渲染帧调用，保证 transitionTicks 按 @20TPS 推进。
        ClientTickEvents.END_CLIENT_TICK.register(
            client -> com.bong.client.era.EraAmbianceState.tick()
        );
        // 旧 server 推过的 realm_collapse evac HUD 是 static volatile 字段倒计时，
        // 不会在断线 / 切服 / 重连时自清。Disconnect 时强制清掉，避免上一 server
        // 的 "域崩撤离 48s" 倒计时跨 session 续命。
        // Fabric 在 ClientPlayNetworkHandler 构造末尾同步触发 INIT，早于任何 PLAY payload。
        // 每个物理 handler 在这里取得不可变 token；JOIN 只激活，不得再次换代。
        ClientPlayConnectionEvents.INIT.register(
            (handler, client) -> ClientConnectionStatusStore.initializeSession(handler)
        );
        ClientPlayConnectionEvents.DISCONNECT.register(
            (handler, client) -> disconnectSession(
                handler,
                Util.getMeasuringTimeMs(),
                BongNetworkHandler::clearClientStateOnDisconnect,
                task -> runOnClientThread(client.isOnThread(), task, client::execute)
            )
        );
        ClientPlayConnectionEvents.JOIN.register(
            (handler, sender, client) -> {
                // onGameJoin 已由 NetworkThreadUtils 切到 client thread；JOIN 回调本身处在
                // GameJoin task 内。这里若再 client.execute，会因 ReentrantThreadExecutor
                // 正在执行 task 而排到队尾，让已排队的首批 server_data 先于 activation 运行。
                long joinedAtMs = Util.getMeasuringTimeMs();
                boolean activated = joinSession(handler, joinedAtMs, () -> {
                    // plan-combat-skill-feedback-bridges-v1 P1 — 初始化本玩家 wire id，
                    // 供 ResonanceLockHandler 区分 partner vs. self（格式：offline:<name>）。
                    if (client.player != null) {
                        String localPlayerId = "offline:" + client.player.getName().getString();
                        com.bong.client.combat.baomai.v4.ResonanceLockHandler.setLocalPlayerId(localPlayerId);
                    }
                });
                if (!activated) {
                    BongClient.LOGGER.error(
                        "Ignoring ClientPlayConnectionEvents.JOIN for handler without INIT session token"
                    );
                }
            }
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

    private static void registerNpcLodChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier(NpcLodHandler.CHANNEL_NAMESPACE, NpcLodHandler.CHANNEL_PATH), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            client.execute(() -> {
                boolean handled = NpcLodHandler.handle(jsonPayload, readableBytes);
                if (!handled) {
                    BongClient.LOGGER.warn("Ignoring bong:npc_lod payload");
                }
            });
        });
    }

    private static void registerNpcBubbleChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier(NpcBubbleHandler.CHANNEL_NAMESPACE, NpcBubbleHandler.CHANNEL_PATH), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            client.execute(() -> {
                boolean handled = NpcBubbleHandler.handle(jsonPayload, readableBytes, Util.getMeasuringTimeMs());
                if (!handled) {
                    BongClient.LOGGER.warn("Ignoring bong:npc_bubble payload");
                }
            });
        });
    }

    private static void registerNpcMoodChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier(NpcMoodHandler.CHANNEL_NAMESPACE, NpcMoodHandler.CHANNEL_PATH), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            client.execute(() -> {
                boolean handled = NpcMoodHandler.handle(jsonPayload, readableBytes, Util.getMeasuringTimeMs());
                if (!handled) {
                    BongClient.LOGGER.warn("Ignoring bong:npc_mood payload");
                }
            });
        });
    }

    private static void registerTsyBossHealthChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier(TsyBossHealthHandler.CHANNEL_NAMESPACE, TsyBossHealthHandler.CHANNEL_PATH), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            client.execute(() -> {
                boolean handled = TsyBossHealthHandler.handle(jsonPayload, readableBytes, Util.getMeasuringTimeMs());
                if (!handled) {
                    BongClient.LOGGER.warn("Ignoring bong:tsy_boss_health payload");
                }
            });
        });
    }

    private static void registerTsyDeathVfxChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier(TsyDeathVfxHandler.CHANNEL_NAMESPACE, TsyDeathVfxHandler.CHANNEL_PATH), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            client.execute(() -> {
                boolean handled = TsyDeathVfxHandler.handle(jsonPayload, readableBytes, Util.getMeasuringTimeMs());
                if (!handled) {
                    BongClient.LOGGER.warn("Ignoring bong:tsy_death_vfx payload");
                }
            });
        });
    }

    /**
     * JOIN 已在 GameJoin client task 内执行；activation 与依赖本地玩家身份的初始化必须同步完成，
     * 不能再嵌套排队，否则队列里已有的 pre-JOIN payload 会在 token 激活前被误丢弃。
     */
    static boolean joinSession(Object handler, long joinedAtMs, Runnable afterActivation) {
        if (!ClientConnectionStatusStore.activateSession(handler, joinedAtMs)) {
            return false;
        }
        afterActivation.run();
        return true;
    }

    /** client thread 上立即执行；其他线程只排一次，避免 MinecraftClient 重入 executor 再次延后。 */
    static void runOnClientThread(
        boolean onClientThread,
        Runnable task,
        Consumer<Runnable> clientExecutor
    ) {
        if (onClientThread) {
            task.run();
        } else {
            clientExecutor.accept(task);
        }
    }

    /**
     * DISCONNECT 的可测试 client-thread 原子边界。Fabric 从
     * ClientPlayNetworkHandler.onDisconnected 调用该事件；该方法先完成 handler token 失效，
     * 随后在同一 client-thread 顺序点清理全局 store。若调用方不在 client thread，executor
     * 会把整个边界排入队列；旧 cleanup 不会脱离 token 判定单独悬到后续 session。
     */
    static void disconnectSession(
        Object handler,
        long disconnectedAtMs,
        Runnable cleanupTask,
        Consumer<Runnable> clientExecutor
    ) {
        clientExecutor.accept(() -> {
            if (ClientConnectionStatusStore.invalidateSession(handler, disconnectedAtMs)) {
                cleanupTask.run();
            }
        });
    }

    private static void registerServerDataChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier("bong", "server_data"),
            BongNetworkHandler::receiveServerDataPayload
        );
    }

    private static void receiveServerDataPayload(
        net.minecraft.client.MinecraftClient client,
        ClientPlayNetworkHandler handler,
        PacketByteBuf buf,
        net.fabricmc.fabric.api.networking.v1.PacketSender responseSender
    ) {
        boolean scheduled = receiveServerDataPayload(
            handler,
            buf,
            client::execute,
            (dispatch, envelopeType) -> applyDispatch(client, dispatch, envelopeType),
            Util::getMeasuringTimeMs,
            () -> {},
            ROUTER,
            com.bong.client.network.ProtoServerDataBridge::bridge
        );
        if (!scheduled) {
            BongClient.LOGGER.error(
                "Ignoring bong:server_data payload from handler without INIT session token"
            );
        }
    }

    /**
     * Raw Fabric callback seam. The concrete handler's immutable token and receive timestamp
     * are captured before the PacketByteBuf is inspected or copied.
     */
    static boolean receiveServerDataPayload(
        ClientPlayNetworkHandler handler,
        PacketByteBuf buf,
        Consumer<Runnable> clientExecutor,
        BiConsumer<ServerDataDispatch, String> dispatchApplier,
        LongSupplier receivedAtMs,
        Runnable afterEnvelopeCapture,
        ServerDataRouter router,
        ServerDataPayloadBridge payloadBridge
    ) {
        ClientConnectionStatusStore.SessionToken sessionToken =
            ClientConnectionStatusStore.sessionToken(handler).orElse(null);
        if (sessionToken == null) {
            return false;
        }
        long capturedReceivedAtMs = receivedAtMs.getAsLong();
        afterEnvelopeCapture.run();

        int readableBytes = buf.readableBytes();
        byte[] bytes = new byte[readableBytes];
        buf.readBytes(bytes);
        return dispatchServerDataPayload(
            bytes,
            router,
            clientExecutor,
            dispatchApplier,
            capturedReceivedAtMs,
            sessionToken,
            payloadBridge
        );
    }

    /**
     * {@code bong:server_data} 的可测试调度边界。
     *
     * <p>production receiver 只在 Netty thread 做四件事：按传入的具体
     * ClientPlayNetworkHandler 捕获 INIT token、捕获 receivedAt、复制 buffer、排入单一
     * client-thread task。token 与时间必须先于任何 buffer access 捕获，防止旧 callback
     * 在重连后把旧 bytes 错绑到新 session。未注册 handler fail closed，连 buffer 都不读。
     * 后续 bridge、route、handler/store/listener side effect 与最终 dispatch 全在该 task 内；
     * JOIN 只激活同一 token，因此 pre-JOIN 首包会在 JOIN task 之后 exactly once 落地；
     * DISCONNECT 后 stale token 整段 no-op。</p>
     *
     * @return true iff handler 已在 INIT 注册且 payload task 已入队
     */
    static boolean dispatchServerDataPayload(
        Object handler,
        byte[] bytes,
        ServerDataRouter router,
        Consumer<Runnable> clientExecutor,
        BiConsumer<ServerDataDispatch, String> dispatchApplier
    ) {
        return dispatchServerDataPayload(
            handler,
            bytes,
            router,
            clientExecutor,
            dispatchApplier,
            Util.getMeasuringTimeMs(),
            com.bong.client.network.ProtoServerDataBridge::bridge
        );
    }

    /** 可注入收包时刻与 bridge 的测试缝；token 仍必须从传入 handler 的 INIT 注册表捕获。 */
    static boolean dispatchServerDataPayload(
        Object handler,
        byte[] bytes,
        ServerDataRouter router,
        Consumer<Runnable> clientExecutor,
        BiConsumer<ServerDataDispatch, String> dispatchApplier,
        long receivedAtMs,
        ServerDataPayloadBridge payloadBridge
    ) {
        ClientConnectionStatusStore.SessionToken sessionToken =
            ClientConnectionStatusStore.sessionToken(handler).orElse(null);
        if (sessionToken == null) {
            return false;
        }
        return dispatchServerDataPayload(
            bytes,
            router,
            clientExecutor,
            dispatchApplier,
            receivedAtMs,
            sessionToken,
            payloadBridge
        );
    }

    private static boolean dispatchServerDataPayload(
        byte[] bytes,
        ServerDataRouter router,
        Consumer<Runnable> clientExecutor,
        BiConsumer<ServerDataDispatch, String> dispatchApplier,
        long receivedAtMs,
        ClientConnectionStatusStore.SessionToken sessionToken,
        ServerDataPayloadBridge payloadBridge
    ) {
        clientExecutor.accept(() -> processServerDataPayload(
            bytes,
            router,
            dispatchApplier,
            receivedAtMs,
            sessionToken,
            payloadBridge
        ));
        return true;
    }

    @FunctionalInterface
    interface ServerDataPayloadBridge {
        com.bong.client.network.ProtoServerDataBridge.BridgeResult bridge(byte[] bytes);
    }

    private static void processServerDataPayload(
        byte[] bytes,
        ServerDataRouter router,
        BiConsumer<ServerDataDispatch, String> dispatchApplier,
        long receivedAtMs,
        ClientConnectionStatusStore.SessionToken sessionToken,
        ServerDataPayloadBridge payloadBridge
    ) {
        // task 执行时 token 必须仍是当前已 JOIN 激活的 session。JOIN 在 GameJoin task 内
        // 同步 activation，因此排在该 task 后的 pre-JOIN payload 会看到 active token；
        // DISCONNECT 的 invalidate + cleanup 与 payload 共用 client-thread 顺序，不会中途切断 route。
        if (!ClientConnectionStatusStore.isActiveSession(sessionToken)) {
            return;
        }
        ClientConnectionStatusStore.markPayloadReceived(receivedAtMs, sessionToken);

        int readableBytes = bytes.length;

        // P3: wire format is now protobuf. Use ProtoServerDataBridge to decode proto
        // and convert back to legacy JSON for existing handlers.
        // Fallback to raw JSON for payloads not yet migrated to proto (e.g. status_snapshot).
        String jsonPayload;
        com.bong.client.network.ProtoServerDataBridge.BridgeResult bridgeResult =
                payloadBridge.bridge(bytes);
        if (bridgeResult.isSuccess()) {
            jsonPayload = bridgeResult.legacyJson();
        } else {
            // Proto bridge failed — log the reason, then try legacy JSON fallback
            // (for payloads like status_snapshot that still send raw JSON).
            BongClient.LOGGER.warn("[bong][network] proto bridge failed: {}", bridgeResult.errorMessage());
            jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
        }

        ServerDataRouter.RouteResult result = router.route(jsonPayload, readableBytes);

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
            dispatchApplier.accept(dispatch, result.envelope().type());
        }
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

    private static void registerQiAttritionChannel() {
        ClientPlayNetworking.registerGlobalReceiver(QiAttritionVfxPlayer.CHANNEL, (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            if (readableBytes < 0 || readableBytes > QiAttritionPayload.MAX_PAYLOAD_BYTES) {
                markConnectionPayload();
                BongClient.LOGGER.error("Rejected bong:vfx/qi_attrition payload with invalid size: {}", readableBytes);
                return;
            }
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = QiAttritionPayload.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(jsonPayload, readableBytes);
                if (!result.success()) {
                    BongClient.LOGGER.error("Failed to parse bong:vfx/qi_attrition payload: {}", result.errorMessage());
                    return;
                }
                QiAttritionVfxPlayer.play(client, result.payload());
                BongClient.LOGGER.debug(
                    "Processed bong:vfx/qi_attrition item={} amount={}",
                    result.payload().itemEntityId(),
                    result.payload().amountLost()
                );
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

    private static void registerTiandaoPresenceChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "tiandao_presence"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                TiandaoPresencePayloadHandler.Result result =
                    TiandaoPresencePayloadHandler.handle(jsonPayload, readableBytes);
                if (!result.handled()) {
                    BongClient.LOGGER.error("Failed to parse bong:tiandao_presence payload: {}", result.logMessage());
                    return;
                }
                BongClient.LOGGER.info("Processed bong:tiandao_presence payload: {}", result.logMessage());
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

    private static void registerMutationVisualChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "mutation_visual"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                try {
                    JsonObject json = JsonParser.parseString(jsonPayload).getAsJsonObject();
                    MutationPayloadHandler.handle(json);
                    BongClient.LOGGER.info("Processed bong:mutation_visual payload ({} bytes)", readableBytes);
                } catch (Exception e) {
                    BongClient.LOGGER.error("Failed to parse bong:mutation_visual payload: {}", e.getMessage());
                }
            });
        });
    }

    // ── plan-combat-skill-feedback-bridges-v1 P1 — baomai_v4 channels ──────────

    /**
     * 注册 {@code bong:crack_reading} channel（仿 registerMutationVisualChannel）。
     * payload JSON → CrackReadingHandler.handle → CrackReadingHudStateStore。
     */
    private static void registerCrackReadingChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "crack_reading"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);
            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                try {
                    com.bong.client.combat.baomai.v4.CrackReadingHandler.handle(jsonPayload);
                    BongClient.LOGGER.debug("Processed bong:crack_reading payload ({} bytes)", readableBytes);
                } catch (Exception e) {
                    BongClient.LOGGER.error("Failed to parse bong:crack_reading payload: {}", e.getMessage());
                }
            });
        });
    }

    /**
     * 注册 {@code bong:resonance_lock} channel。
     * payload JSON → ResonanceLockHandler.handleLock → ResonanceLockHudStateStore + VFX。
     */
    private static void registerResonanceLockChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "resonance_lock"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);
            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                try {
                    com.bong.client.combat.baomai.v4.ResonanceLockHandler.handleLock(jsonPayload);
                    BongClient.LOGGER.debug("Processed bong:resonance_lock payload ({} bytes)", readableBytes);
                } catch (Exception e) {
                    BongClient.LOGGER.error("Failed to parse bong:resonance_lock payload: {}", e.getMessage());
                }
            });
        });
    }

    /**
     * 注册 {@code bong:resonance_lock_end} channel。
     * payload JSON → ResonanceLockHandler.handleLockEnd → 清空状态 + 解锁 VFX。
     */
    private static void registerResonanceLockEndChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "resonance_lock_end"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);
            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                try {
                    com.bong.client.combat.baomai.v4.ResonanceLockHandler.handleLockEnd(jsonPayload);
                    BongClient.LOGGER.debug("Processed bong:resonance_lock_end payload ({} bytes)", readableBytes);
                } catch (Exception e) {
                    BongClient.LOGGER.error("Failed to parse bong:resonance_lock_end payload: {}", e.getMessage());
                }
            });
        });
    }

    // ── plan-combat-skill-feedback-bridges-v1 P3 — void erosion visual channel ─────────

    /**
     * 注册 {@code bong:void_erosion_visual} channel（仿 registerMutationVisualChannel）。
     * payload JSON → {@link com.bong.client.visual.VoidErosionVisualHandler#handle} →
     * {@link com.bong.client.visual.VoidErosionVisualStore}。
     *
     * <p>stage ≥ 3 时 {@code sound_distortion_active=true}，驱动
     * {@link com.bong.client.visual.VoidErosionHudOverlay} 渲染声音扭曲 overlay。
     */
    private static void registerVoidErosionVisualChannel() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "void_erosion_visual"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);
            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            markConnectionPayload();
            client.execute(() -> {
                try {
                    com.bong.client.visual.VoidErosionVisualHandler.handle(jsonPayload);
                    BongClient.LOGGER.debug("Processed bong:void_erosion_visual payload ({} bytes)", readableBytes);
                } catch (Exception e) {
                    BongClient.LOGGER.error("Failed to parse bong:void_erosion_visual payload: {}", e.getMessage());
                }
            });
        });
    }

    // ── plan-fauna-mimic-spider-v1 P2 — spider disguise CustomPayload channels ─────────

    /**
     * 注册 {@code bong:spider_disguise_enter} channel。
     * payload JSON → {@link SpiderDisguiseHandler#handleEnter} → 全量替换 disguised entity id 集合。
     *
     * <p>服务端每 40 tick 全量 broadcast；client 用 full-replace 策略（clear + addAll）确保状态同步。
     */
    private static void registerSpiderDisguiseEnterChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier(SpiderDisguiseHandler.CHANNEL_NAMESPACE, SpiderDisguiseHandler.CHANNEL_PATH_ENTER),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    boolean handled = SpiderDisguiseHandler.handleEnter(jsonPayload, readableBytes);
                    if (handled) {
                        BongClient.LOGGER.debug("Processed bong:spider_disguise_enter ({} bytes)", readableBytes);
                    } else {
                        BongClient.LOGGER.warn("Ignoring bong:spider_disguise_enter payload (parse failed)");
                    }
                });
            }
        );
    }

    /**
     * 注册 {@code bong:spider_ambush_trigger} channel。
     * payload JSON → {@link SpiderDisguiseHandler#handleAmbush} → 从 disguised 集合移除暴起的蛛。
     */
    private static void registerSpiderAmbushTriggerChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier(SpiderDisguiseHandler.CHANNEL_NAMESPACE, SpiderDisguiseHandler.CHANNEL_PATH_AMBUSH),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    boolean handled = SpiderDisguiseHandler.handleAmbush(jsonPayload, readableBytes);
                    if (handled) {
                        BongClient.LOGGER.debug("Processed bong:spider_ambush_trigger ({} bytes)", readableBytes);
                    } else {
                        BongClient.LOGGER.warn("Ignoring bong:spider_ambush_trigger payload (parse failed)");
                    }
                });
            }
        );
    }

    // ── plan-daozhan-v1 P1 — daozhan disguise CustomPayload channels ────────────

    /**
     * 注册 {@code bong:daozhan_disguise_enter} channel。
     * payload JSON → {@link DaoZhanDisguiseHandler#handleEnter} → 全量替换 disguised entity id 集合。
     *
     * <p>服务端每 40 tick 全量 broadcast；client 用 full-replace 策略（clear + addAll）确保状态同步。
     * {@link com.bong.client.daozhan.FakePlayerRendererMixin} 在渲染层查询此集合，
     * 将命中实体切换为无名玩家模型。
     */
    private static void registerDaoZhanDisguiseEnterChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier(DaoZhanDisguiseHandler.CHANNEL_NAMESPACE, DaoZhanDisguiseHandler.CHANNEL_PATH_ENTER),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    boolean handled = DaoZhanDisguiseHandler.handleEnter(jsonPayload, readableBytes);
                    if (handled) {
                        BongClient.LOGGER.debug("Processed bong:daozhan_disguise_enter ({} bytes)", readableBytes);
                    } else {
                        BongClient.LOGGER.warn("Ignoring bong:daozhan_disguise_enter payload (parse failed)");
                    }
                });
            }
        );
    }

    /**
     * 注册 {@code bong:daozhan_reveal} channel。
     * payload JSON → {@link DaoZhanDisguiseHandler#handleReveal} → 从 disguised 集合移除暴起的道伥。
     * client 侧 {@link com.bong.client.daozhan.FakePlayerRendererMixin} 自动切回正常道伥渲染。
     */
    private static void registerDaoZhanRevealChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier(DaoZhanDisguiseHandler.CHANNEL_NAMESPACE, DaoZhanDisguiseHandler.CHANNEL_PATH_REVEAL),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    boolean handled = DaoZhanDisguiseHandler.handleReveal(jsonPayload, readableBytes);
                    if (handled) {
                        BongClient.LOGGER.debug("Processed bong:daozhan_reveal ({} bytes)", readableBytes);
                    } else {
                        BongClient.LOGGER.warn("Ignoring bong:daozhan_reveal payload (parse failed)");
                    }
                });
            }
        );
    }

    // ── plan-fauna-stitched-beast-v1 P3 — 兽核吸收幻觉 HUD channel ─────────────

    /**
     * 注册 {@code bong:core_absorption_hallucination} channel。
     * payload JSON → {@link com.bong.client.fauna.HallucinationLayerHandler#handle} →
     * {@link com.bong.client.fauna.HallucinationLayerStore}。
     *
     * <p>仅写入 <em>显示层</em> 状态，不改玩家实际 HP / qi。
     */
    private static void registerCoreAbsorptionHallucinationChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier(
                com.bong.client.fauna.HallucinationLayerHandler.CHANNEL_NAMESPACE,
                com.bong.client.fauna.HallucinationLayerHandler.CHANNEL_PATH
            ),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    try {
                        com.bong.client.fauna.HallucinationLayerHandler.handle(jsonPayload);
                        BongClient.LOGGER.debug(
                            "Processed bong:core_absorption_hallucination ({} bytes)", readableBytes);
                    } catch (Exception e) {
                        BongClient.LOGGER.error(
                            "Failed to process bong:core_absorption_hallucination: {}", e.getMessage());
                    }
                });
            }
        );
    }

    // ── plan-dying-elder-v1 P3 — 垂死大能遭遇 HUD channel ──────────────────────

    /**
     * 注册 {@code bong:elder_encounter} channel。
     * payload JSON → {@link com.bong.client.dying_elder.DyingElderEncounterHandler#handle} →
     * {@link com.bong.client.dying_elder.DyingElderEncounterStore}。
     *
     * <p>仅写入 <em>显示层</em> 状态，不改玩家实际 HP / qi 或任何 gameplay 数值。
     */
    private static void registerElderEncounterChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier(
                com.bong.client.dying_elder.DyingElderEncounterHandler.CHANNEL_NAMESPACE,
                com.bong.client.dying_elder.DyingElderEncounterHandler.CHANNEL_PATH
            ),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    boolean handled = com.bong.client.dying_elder.DyingElderEncounterHandler.handle(jsonPayload);
                    if (handled) {
                        BongClient.LOGGER.debug(
                            "Processed bong:elder_encounter ({} bytes)", readableBytes);
                    } else {
                        BongClient.LOGGER.warn(
                            "Ignoring bong:elder_encounter payload (parse failed)");
                    }
                });
            }
        );
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

    static void clearClientStateOnDisconnect() {
        RealmCollapseHudStateStore.clearOnDisconnect();
        NpcMetadataStore.clearAll();
        NpcLodStore.clearAll();
        NpcMoodStore.clearAll();
        NpcDialogueBubbleRenderer.clear();
        TsyBossHealthStore.reset();
        TsyDeathVfxStore.reset();
        com.bong.client.hud.CoffinStateStore.clear();
        com.bong.client.gathering.GatheringSessionStore.clearOnDisconnect();
        com.bong.client.audio.MusicStateMachine.instance().clear();
        MutationVisualState.reset();
        com.bong.client.combat.baomai.v4.CrackReadingHudStateStore.clear();
        com.bong.client.combat.baomai.v4.ResonanceLockHudStateStore.clear();
        // plan-combat-skill-feedback-bridges-v1 P3 — 断线时清理虚蚀视觉状态
        com.bong.client.visual.VoidErosionVisualStore.reset();
        // plan-fauna-mimic-spider-v1 P2 — 断线时清理伪装蛛列表
        SpiderDisguiseHandler.clearOnDisconnect();
        // plan-daozhan-v1 P1 — 断线时清理道伥伪装列表
        DaoZhanDisguiseHandler.clearOnDisconnect();
        // plan-fauna-stitched-beast-v1 P3 — 断线时清理幻觉层状态
        com.bong.client.fauna.HallucinationLayerStore.clearOnDisconnect();
        // plan-dying-elder-v1 P3 — 断线时清理垂死大能遭遇状态
        com.bong.client.dying_elder.DyingElderEncounterStore.clearOnDisconnect();
        // F19 fix — 断线时清理天道临在状态；此前十余个 store 都在这里清理，唯独
        // TiandaoPresenceStore 漏掉，导致断线重连后旧 presence（watch/pressure/
        // tribulation/annihilate vignette）继续渲染/播放。
        com.bong.client.tiandao.TiandaoPresenceStore.clear();
        // plan-bughunt-hud-state-session-reset — 断线时重置生产 HUD snapshot（zoneState +
        // visualEffectState）；此前 static BongHudStateStore 不在这份清理清单里，导致上一
        // server 的区域 overlay/atmosphere 与 HUD tint/相机/FOV 视觉特效跨 session 残留，
        // 直到新服首个 zone_info 到达或旧 visual effect 自然过期。
        BongHudStateStore.clear();
        // plan-bughunt-search-hud-stuck-v1 — 搜刮 HUD 是静态 store；断线时无 server
        // payload 能替新 session 主动清掉旧 SEARCHING/terminal flash，必须随统一清理链复位。
        SearchHudStateStore.clearOnDisconnect();
        // plan-era-state-v1 P3 — 断线时重置时代天象状态
        com.bong.client.era.EraAmbianceState.reset();
        // plan-agent-ui-data-v1 P1 — 断线时清理天道 UI 面板状态
        com.bong.client.agentui.AgentUiStore.clear();
        // plan-halfstep-rechallenge-integration-v1 P0 — 断线时清理半步重渡触发状态
        com.bong.client.combat.store.HalfStepRechallengeStore.clear();
        // F9 跨层修复 — 断线时清理出生引导棺坐标缓存；不同 server 的棺位置不同，
        // 留着旧坐标会让 reconnect 后短暂窗口内用错误坐标误判/漏判引导棺。
        com.bong.client.coffin.TutorialCoffinPosStore.clearOnDisconnect();
        // plan-remains-suite P0 — 断线时清理遗骸缓存；不同 server 的遗骸完全无关，
        // 留着旧快照会让 reconnect 后 G 键短暂命中一具已经不存在的遗骸。
        com.bong.client.inventory.state.RemainsStore.clearOnDisconnect();
        // plan-bughunt-dropped-loot-session-leak — 断线时清理地面掉落物缓存；此前
        // DroppedItemStore.clearOnDisconnect() 定义了却从未被调用，导致切服/重连后
        // 在新 server 首个 dropped_loot_sync 抵达前，旧 server 的掉落物坐标会被当
        // 前 world 渲染 billboard、G 键还会带着旧 instanceId 发 pickup 请求。
        com.bong.client.inventory.state.DroppedItemStore.clearOnDisconnect();
        // plan-craft-session-reconnect-lock-v1 P0 — craft store 是静态跨屏状态；
        // 断线不清会把旧 active session 带进新连接，导致手搓/制作台主操作永久灰掉。
        CraftStore.clear();
        // plan-bughunt-client-toast-cross-session-leak-v1 P0 — BongToast.activeToast
        // 是静态单槽，只在自然过期后自清；断线不清会让上一 server 未过期的 warning/
        // era/event/inventory toast 在 reconnect 后的首几秒继续渲染，串成跨 session 泄漏。
        BongToast.clearOnDisconnect();
        // plan-bughunt-client-identity-panel-stale-session-v1 — IdentityPanelStateStore
        // 此前没有生产态清理入口，断线不清会让 HUD 角标和刚打开的身份面板短暂展示上一局
        // 身份数据；面板一旦在旧快照上 init 完，按钮回调还会冻结在旧 identityId 上。
        IdentityPanelStateStore.clearOnDisconnect();
        // plan-bughunt-client-false-skin-cross-session-v1 — FalseSkinHudStateStore 此前只有
        // resetForTests()，生产态断线清理清单里完全没有它。server 的 false_skin_state 只在
        // Changed/RemovedComponents 时增量发包，断线切 session 不会有任何 payload 覆盖旧
        // 快照，会让上一局的伪皮层数块（FalseSkinStackHud）和污染负载条（ContamLoadHud）
        // 无限跨 session 残留，直到再次收到一条 false_skin_state。
        com.bong.client.combat.store.FalseSkinHudStateStore.clearOnDisconnect();
        // plan-bughunt-dugu-v2-hud-disconnect-bleed-v1 P0 — DuguV2HudStateStore 此前只有
        // resetForTests()，生产态断线清理清单里完全没有它。server 的 dugu_v2_skill_cast /
        // dugu_v2_self_cure / dugu_v2_shroud_active / permanent_qi_max_decay_applied bridge
        // 只在毒蛊 v2 事件发生时推增量/状态，没有 join/disconnect reset payload；revealRisk
        // 无 expiry 字段、selfRevealed 是 sticky merge，新 session 若没再触发毒蛊 v2 事件也不
        // 会有 payload 覆盖旧快照，导致上一局的“暴露 xx%”“自蕴 xx% 已露”或遮蔽 tint 无限期
        // 跨 session 残留到下一局。
        DuguV2HudStateStore.clearOnDisconnect();
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

    // ── plan-agent-ui-data-v1 P1 — 天道动态 UI 面板专属 channel ──────────────────────────

    /**
     * 注册 {@code bong:agent_ui_request} 和 {@code bong:agent_ui_close} 专属 channel。
     *
     * <p>payload 是裸 {@code AgentUiRequestPayloadV1} / {@code AgentUiClosePayloadV1} JSON
     * （无 {@code ServerDataV1} envelope），由 {@link com.bong.client.network.AgentUiPayloadHandler}
     * 的专属方法解析后写入 {@link com.bong.client.agentui.AgentUiStore}。
     *
     * <p>设计对齐 {@code bong:halfstep_rechallenge} 专属 channel 模式，不经过 {@code ServerDataRouter}
     * / {@code ProtoServerDataBridge}（避免 proto 不支持 AgentUiRequest/AgentUiClose variant
     * 导致的 unreachable!() panic）。
     */
    private static void registerAgentUiChannels() {
        // bong:agent_ui_request — 裸 AgentUiRequestPayloadV1 JSON
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier("bong", "agent_ui_request"),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    try {
                        com.bong.client.network.AgentUiPayloadHandler.handleRawRequest(
                            jsonPayload, client);
                        BongClient.LOGGER.debug(
                            "Processed bong:agent_ui_request payload ({} bytes)", readableBytes);
                    } catch (Exception ex) {
                        BongClient.LOGGER.error(
                            "Failed to handle bong:agent_ui_request payload: {}", ex.getMessage());
                    }
                });
            }
        );
        // bong:agent_ui_close — 裸 AgentUiClosePayloadV1 JSON
        ClientPlayNetworking.registerGlobalReceiver(
            com.bong.client.network.AgentUiPayloadHandler.AGENT_UI_CLOSE_CHANNEL,
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                markConnectionPayload();
                com.bong.client.network.AgentUiPayloadHandler.dispatchRawClose(bytes, client::execute);
                BongClient.LOGGER.debug(
                    "Queued bong:agent_ui_close payload ({} bytes) for client thread", readableBytes);
            }
        );
    }

    // ── plan-halfstep-rechallenge-integration-v1 P0 — 半步化虚重渡触发 HUD channel ─────

    /**
     * 注册 {@code bong:halfstep_rechallenge} 专属 channel。
     *
     * <p>payload 是裸 {@code HalfStepRechallengeV1} JSON（无 {@code ServerDataV1} envelope），
     * 由 {@link com.bong.client.combat.handler.HalfStepRechallengeHandler#handle(String)} 解析，
     * 结果写入 {@link com.bong.client.combat.store.HalfStepRechallengeStore}。
     *
     * <p>设计对齐 {@code bong:era_ambiance} 专属 channel 模式，不经过 {@code ServerDataRouter}
     * / {@code ProtoServerDataBridge}（避免 proto 不支持此 variant 导致的 unreachable panic）。
     */
    private static void registerHalfStepRechallengeChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier("bong", "halfstep_rechallenge"),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    try {
                        com.bong.client.combat.handler.HalfStepRechallengeHandler.handle(jsonPayload);
                        BongClient.LOGGER.debug(
                            "Processed bong:halfstep_rechallenge payload ({} bytes)", readableBytes);
                    } catch (Exception ex) {
                        BongClient.LOGGER.error(
                            "Failed to handle bong:halfstep_rechallenge payload: {}", ex.getMessage());
                    }
                });
            }
        );
    }

    // ── plan-era-state-v1 P3 — 时代天象 S2C CustomPayload ──────────────────────

    /**
     * 注册 {@code bong:era_ambiance} channel。
     * payload JSON → {@link com.bong.client.era.EraAmbianceHandler#handle(String)} →
     * {@link com.bong.client.era.EraAmbianceState} 存储，渲染层按需消费。
     * Realm gate 由 server 执行，client 收到即接受，不做二次 realm 过滤。
     */
    private static void registerEraAmbianceChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier("bong", "era_ambiance"),
            (client, handler, buf, responseSender) -> {
                int readableBytes = buf.readableBytes();
                byte[] bytes = new byte[readableBytes];
                buf.readBytes(bytes);
                String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                markConnectionPayload();
                client.execute(() -> {
                    try {
                        com.bong.client.era.EraAmbianceHandler.handle(jsonPayload);
                        BongClient.LOGGER.debug(
                            "Processed bong:era_ambiance payload ({} bytes)", readableBytes);
                    } catch (Exception ex) {
                        BongClient.LOGGER.error(
                            "Failed to handle bong:era_ambiance payload: {}", ex.getMessage());
                    }
                });
            }
        );
    }

}
