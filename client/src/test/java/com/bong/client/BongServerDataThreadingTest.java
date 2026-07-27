package com.bong.client;

import bong.Envelope;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.craft.CraftOutcomeFeedback;
import com.bong.client.craft.CraftScreen;
import com.bong.client.craft.CraftStore;
import com.bong.client.craft.WorkbenchScreen;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.ui.ClientConnectionStatusStore;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.PacketByteBuf;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BiConsumer;

import sun.misc.Unsafe;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BongServerDataThreadingTest {
    private static final String NETWORK_THREAD = "fabric-network-io-test";
    private static final String CLIENT_THREAD = "minecraft-client-render-test";

    private final List<Runnable> clientTasks = new CopyOnWriteArrayList<>();
    private Object activeHandler;

    @BeforeEach
    void setUp() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        CastStateStore.resetForTests();
        UnifiedEventStore.resetForTests();
        ClientConnectionStatusStore.resetForTests();
        activeHandler = new Object();
        ClientConnectionStatusStore.SessionToken token =
            ClientConnectionStatusStore.initializeSession(activeHandler);
        assertSame(
            token,
            ClientConnectionStatusStore.initializeSession(activeHandler),
            "同一物理 handler 的重复 INIT 必须幂等返回原 token"
        );
        assertTrue(
            ClientConnectionStatusStore.activateSession(activeHandler, 1_000L),
            "测试前置连接必须激活 INIT 已分配的 token"
        );
    }

    @AfterEach
    void tearDown() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        CastStateStore.resetForTests();
        UnifiedEventStore.resetForTests();
        ClientConnectionStatusStore.resetForTests();
        clientTasks.clear();
    }

    @Test
    void routeAndApplyDispatchRunInOneOrderedClientTask() {
        List<String> events = new CopyOnWriteArrayList<>();
        ServerDataRouter router = new ServerDataRouter(Map.of(
            "thread_probe",
            envelope -> {
                events.add("route@" + Thread.currentThread().getName());
                return ServerDataDispatch.handledWithLegacyMessage(
                    envelope.type(), "probe", "thread probe handled");
            }
        ));

        dispatchOnNetworkThread(
            "{\"v\":1,\"type\":\"thread_probe\"}",
            router,
            (dispatch, type) -> events.add("apply:" + type + "@" + Thread.currentThread().getName())
        );

        assertTrue(
            events.isEmpty(),
            "raw receiver 返回前不得执行 route/handler side effect；实际事件=" + events
        );
        assertEquals(1, clientTasks.size(), "一条 server_data payload 应只排入一个 client-thread task");

        runNextClientTask();

        assertEquals(
            List.of("route@" + CLIENT_THREAD, "apply:thread_probe@" + CLIENT_THREAD),
            events,
            "route → applyDispatch 必须在同一个 client task 内按序执行；实际=" + events
        );
        assertTrue(
            clientTasks.isEmpty(),
            "期望单 payload 执行后 client task queue 为空，因为不得嵌套或重复调度；实际 queue="
                + clientTasks
        );
    }

    @ParameterizedTest
    @ValueSource(strings = {"completed", "failed"})
    void craftOutcomeStoreAndListenerWaitForClientThread(String kind) {
        List<String> listenerThreads = new CopyOnWriteArrayList<>();
        CraftStore.addOutcomeListener(event ->
            listenerThreads.add(event.kind().name() + "@" + Thread.currentThread().getName()));

        dispatchDefaultOnNetworkThread(craftOutcome(kind, "craft.threading.single"));

        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "期望 " + kind + " outcome 在 client task 执行前为空，因为 Fabric network thread"
                + " 不得写 CraftStore；实际 lastOutcome=" + CraftStore.lastOutcome()
        );
        assertTrue(
            listenerThreads.isEmpty(),
            "期望 " + kind + " outcome listener 在 receiver 返回前为空，因为副作用必须等待"
                + " client task；实际 listenerThreads=" + listenerThreads
        );
        assertEquals(1, clientTasks.size(), kind + " payload 应精确排入一个 client-thread task");

        runNextClientTask();

        CraftStore.CraftOutcomeEvent outcome = CraftStore.lastOutcome().orElseThrow();
        assertEquals("craft.threading.single", outcome.recipeId());
        assertEquals(
            List.of(outcome.kind().name() + "@" + CLIENT_THREAD),
            listenerThreads,
            "CraftScreen/WorkbenchScreen 共用的同步 outcome listener 必须只在 client thread 触发一次"
        );
    }

    @Test
    void castSyncStoreListenerWaitsForClientThread() {
        List<String> listenerThreads = new CopyOnWriteArrayList<>();
        CastStateStore.addListener(state ->
            listenerThreads.add(state.phase().name() + "@" + Thread.currentThread().getName()));

        dispatchDefaultOnNetworkThread("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":2,
             "duration_ms":800,"started_at_ms":1700000000000,"outcome":"none"}
            """);

        assertEquals(CastState.Phase.IDLE, CastStateStore.snapshot().phase(),
            "cast_sync handler 不得在 network thread 提前替换 HUD store");
        assertTrue(
            listenerThreads.isEmpty(),
            "期望 cast_sync listener 在 receiver 返回前为空，因为 HUD store 只能在 client task"
                + " 内通知；实际 listenerThreads=" + listenerThreads
        );
        assertEquals(1, clientTasks.size(), "cast_sync 应排入一个 client-thread task");

        runNextClientTask();

        assertEquals(CastState.Phase.CASTING, CastStateStore.snapshot().phase());
        assertEquals(
            List.of("CASTING@" + CLIENT_THREAD),
            listenerThreads,
            "cast_sync store/listener side effect 必须绑定 client thread"
        );
    }

    @Test
    void consecutivePayloadsKeepSubmissionOrderAndApplyExactlyOnce() {
        List<String> recipes = new CopyOnWriteArrayList<>();
        CraftStore.addOutcomeListener(event -> recipes.add(event.recipeId()));

        runNamedThread(NETWORK_THREAD, () -> {
            dispatchDefault(craftOutcome("completed", "craft.threading.first"));
            dispatchDefault(craftOutcome("completed", "craft.threading.second"));
        });

        assertTrue(recipes.isEmpty(), "连续 payload 在 client queue drain 前不得提前写 store；实际=" + recipes);
        assertEquals(2, clientTasks.size(), "两条 payload 应按提交顺序形成两个 client task");

        runNextClientTask();
        assertEquals(List.of("craft.threading.first"), recipes,
            "第一轮 drain 只能应用第一条 payload；实际=" + recipes);

        runNextClientTask();
        assertEquals(List.of("craft.threading.first", "craft.threading.second"), recipes,
            "第二轮 drain 后必须保持提交顺序且各应用一次；实际=" + recipes);
    }

    @Test
    void malformedPayloadDoesNotPoisonFollowingValidTask() {
        runNamedThread(NETWORK_THREAD, () -> {
            dispatchDefault("{not valid json");
            dispatchDefault(craftOutcome("completed", "craft.threading.after_bad"));
        });

        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "期望坏 payload 后的合法 payload 在 client drain 前仍未写 store，因为两条任务必须"
                + " 保持排队；实际 lastOutcome=" + CraftStore.lastOutcome()
                + "，clientTasks=" + clientTasks
        );
        assertEquals(2, clientTasks.size(), "坏 payload 与后续合法 payload 都应保留各自的有序 client task");

        runNextClientTask();
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "期望只 drain 坏 payload task 后 craft outcome 仍为空，因为 parse error 必须 no-op；"
                + "实际 lastOutcome=" + CraftStore.lastOutcome() + "，clientTasks=" + clientTasks
        );

        runNextClientTask();
        assertEquals(
            "craft.threading.after_bad",
            CraftStore.lastOutcome().orElseThrow().recipeId(),
            "前一条 parse error 不得吞掉后续合法 payload"
        );
    }

    @Test
    void handlerFailureIsContainedAndFollowingPayloadStillRuns() {
        List<String> events = new CopyOnWriteArrayList<>();
        ServerDataRouter router = new ServerDataRouter(Map.of(
            "thread_probe",
            envelope -> {
                String mode = envelope.payload().get("mode").getAsString();
                if ("fail".equals(mode)) {
                    throw new IllegalStateException("expected probe failure");
                }
                events.add(mode + "@" + Thread.currentThread().getName());
                return ServerDataDispatch.handled(envelope.type(), "probe handled");
            }
        ));

        runNamedThread(NETWORK_THREAD, () -> {
            dispatch("{\"v\":1,\"type\":\"thread_probe\",\"mode\":\"fail\"}", router, (d, t) -> {});
            dispatch("{\"v\":1,\"type\":\"thread_probe\",\"mode\":\"ok\"}", router, (d, t) -> {});
        });

        assertTrue(
            events.isEmpty(),
            "期望 handler failure 与后续 handler 在 client drain 前都未执行，因为 network thread"
                + " 只负责排队；实际 events=" + events + "，clientTasks=" + clientTasks
        );
        assertEquals(2, clientTasks.size(), "handler failure 不得取消后续 payload 的 client task");

        runNextClientTask();
        assertTrue(
            events.isEmpty(),
            "期望 drain 失败 handler 后事件仍为空，因为 router 应安全收口为 no-op；实际 events="
                + events + "，clientTasks=" + clientTasks
        );
        runNextClientTask();
        assertEquals(List.of("ok@" + CLIENT_THREAD), events,
            "失败后的合法 handler 必须仍在 client thread 正常执行一次");
    }

    @Test
    void markConnectionPayloadUsesReceiptTimestampNotProcessingTime() {
        long receiptAt = 2_500L;

        runNamedThread(NETWORK_THREAD, () -> dispatch(
            activeHandler,
            craftOutcome("completed", "craft.freshness").getBytes(StandardCharsets.UTF_8),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            receiptAt,
            ProtoServerDataBridge::bridge
        ));

        assertEquals(
            1_000L,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "receiver 返回前不得用 processing time 改 freshness；实际 lastPayloadAtMs="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertEquals(1, clientTasks.size(), "payload 应精确排入一个 client task");

        runNextClientTask();

        assertEquals(
            receiptAt,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "drain 后 freshness 必须是收包时刻 " + receiptAt + " 而非 processing time；实际="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertTrue(
            ClientConnectionStatusStore.connectedForTests(),
            "合法当前 session payload 应保持 connected"
        );
    }

    @Test
    void delayedSameTokenTaskKeepsNewerCrossChannelFreshnessAndRoutesExactlyOnce() {
        long delayedReceiptAt = 2_500L;
        long newerCrossChannelReceiptAt = 2_600L;
        ClientConnectionStatusStore.SessionToken token =
            ClientConnectionStatusStore.sessionToken(activeHandler).orElseThrow();
        AtomicInteger storeSideEffects = new AtomicInteger();
        List<String> routedRecipes = new CopyOnWriteArrayList<>();
        CraftStore.addOutcomeListener(event -> {
            storeSideEffects.incrementAndGet();
            routedRecipes.add(event.recipeId());
        });

        runNamedThread(NETWORK_THREAD, () -> dispatch(
            activeHandler,
            craftOutcome("completed", "craft.freshness.delayed").getBytes(StandardCharsets.UTF_8),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            delayedReceiptAt,
            ProtoServerDataBridge::bridge
        ));

        assertEquals(1, clientTasks.size(),
            "延迟 server_data payload 应精确排入一个 client task；实际 queue=" + clientTasks);
        assertEquals(0, storeSideEffects.get(),
            "drain 前 route→CraftStore 副作用不得提前发生；实际次数=" + storeSideEffects.get());

        ClientConnectionStatusStore.markPayloadReceived(newerCrossChannelReceiptAt, token);
        ClientConnectionStatusStore.markPayloadReceived(0L, token);
        ClientConnectionStatusStore.markPayloadReceived(-1L, token);
        assertEquals(
            newerCrossChannelReceiptAt,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "跨 channel 新 payload 应把 freshness 推进到 " + newerCrossChannelReceiptAt
                + "，同 token 的 0/负时间戳不得回退；实际="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertTrue(
            ClientConnectionStatusStore.connectedForTests(),
            "同 token 的新 payload 应保持 connected；实际 connected="
                + ClientConnectionStatusStore.connectedForTests()
        );

        runNextClientTask();

        assertEquals(
            newerCrossChannelReceiptAt,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "同 token 的延迟 task 收包时刻 " + delayedReceiptAt
                + " 不得把较新的跨 channel freshness " + newerCrossChannelReceiptAt
                + " 回退；实际=" + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertEquals(
            List.of("craft.freshness.delayed"),
            routedRecipes,
            "旧 receipt timestamp 只影响 freshness 合并，不得丢弃或重复 route→CraftStore；实际 recipes="
                + routedRecipes
        );
        assertEquals(
            1,
            storeSideEffects.get(),
            "延迟 payload 的 route→CraftStore side effect 必须恰好一次；实际次数="
                + storeSideEffects.get()
        );
        assertTrue(clientTasks.isEmpty(),
            "单 payload drain 后不得残留嵌套/重复 task；实际 queue=" + clientTasks);
    }

    @Test
    void preJoinRealProtobufHydrationAndOutcomeApplyExactlyOnceAfterSynchronousJoin() {
        ClientConnectionStatusStore.resetForTests();
        Object handler = new Object();
        ClientConnectionStatusStore.SessionToken token =
            ClientConnectionStatusStore.initializeSession(handler);
        assertSame(
            token,
            ClientConnectionStatusStore.initializeSession(handler),
            "同 handler 重复 INIT 不得为 pre-JOIN payload 换 token"
        );

        AtomicInteger bridgeCalls = new AtomicInteger();
        AtomicInteger recipeNotifications = new AtomicInteger();
        AtomicInteger sessionNotifications = new AtomicInteger();
        AtomicInteger outcomeNotifications = new AtomicInteger();
        AtomicInteger completeSounds = new AtomicInteger();
        AtomicInteger refreshes = new AtomicInteger();
        List<String> events = new CopyOnWriteArrayList<>();
        CraftStore.addRecipeListener(recipes -> {
            recipeNotifications.incrementAndGet();
            events.add("recipes=" + recipes.size() + "@" + Thread.currentThread().getName());
        });
        CraftStore.addSessionListener(session -> {
            sessionNotifications.incrementAndGet();
            events.add("session=" + session.recipeId().orElse("idle") + "@"
                + Thread.currentThread().getName());
        });
        CraftStore.addOutcomeListener(event -> {
            outcomeNotifications.incrementAndGet();
            events.add("outcome=" + event.recipeId() + "@" + Thread.currentThread().getName());
            CraftOutcomeFeedback.apply(
                event,
                ticks -> events.add("flash=" + ticks + "@" + Thread.currentThread().getName()),
                () -> {
                    completeSounds.incrementAndGet();
                    events.add("sound@" + Thread.currentThread().getName());
                },
                () -> {
                    refreshes.incrementAndGet();
                    events.add("refresh@" + Thread.currentThread().getName());
                }
            );
        });
        BongNetworkHandler.ServerDataPayloadBridge countingBridge = bytes -> {
            bridgeCalls.incrementAndGet();
            return ProtoServerDataBridge.bridge(bytes);
        };

        // Netty 可在 GameJoin 的 client task 尚未执行时先把首批 payload task 排到它后面。
        // Fabric JOIN 本身在 onGameJoin@RETURN、也就是该 GameJoin task 内同步触发；activation
        // 不得再嵌套排到队尾，否则下面三条首包会在 INIT token 尚未 active 时全部丢弃。
        runNamedThread(NETWORK_THREAD, () -> {
            assertTrue(dispatch(
                handler,
                craftRecipeListProto(),
                ServerDataRouter.createDefault(),
                (dispatch, type) -> {},
                2_100L,
                countingBridge
            ), "pre-JOIN recipe snapshot 应按 handler 捕获 INIT token 并排队");
            assertTrue(dispatch(
                handler,
                craftSessionStateProto(),
                ServerDataRouter.createDefault(),
                (dispatch, type) -> {},
                2_110L,
                countingBridge
            ), "pre-JOIN session snapshot 应按 handler 捕获 INIT token 并排队");
            assertTrue(dispatch(
                handler,
                craftOutcomeProto("craft.prejoin.outcome"),
                ServerDataRouter.createDefault(),
                (dispatch, type) -> {},
                2_120L,
                countingBridge
            ), "pre-JOIN outcome 应按 handler 捕获 INIT token 并排队");
        });

        assertEquals(3, clientTasks.size(),
            "JOIN callback 执行前队列只含三条 pre-JOIN payload；actual=" + clientTasks);
        assertFalse(ClientConnectionStatusStore.isActiveSession(token),
            "GameJoin/JOIN 尚未运行时 INIT token 不得提前 active");
        AtomicInteger joinedIdentityInitializations = new AtomicInteger();
        runNamedThread(CLIENT_THREAD, () -> assertTrue(
            BongNetworkHandler.joinSession(
                handler,
                2_000L,
                joinedIdentityInitializations::incrementAndGet
            ),
            "JOIN 必须在当前 GameJoin client task 内同步激活 INIT 分配的同一 token"
        ));

        assertEquals(3, clientTasks.size(),
            "同步 JOIN 不得额外排 activation task，否则 pre-JOIN 首包会先执行；actual=" + clientTasks);
        assertTrue(ClientConnectionStatusStore.isActiveSession(token),
            "JOIN callback 返回前必须已经激活原 INIT token");
        assertEquals(1, joinedIdentityInitializations.get(),
            "JOIN 后本地身份初始化必须同步且恰好一次");
        assertEquals(0, bridgeCalls.get(), "JOIN activation 本身不得 bridge payload");
        assertTrue(CraftStore.recipes().isEmpty(), "仅执行 JOIN 后 recipe 仍应为空");
        assertFalse(CraftStore.sessionState().active(), "仅执行 JOIN 后 session 仍应为空");
        assertTrue(CraftStore.lastOutcome().isEmpty(), "仅执行 JOIN 后 outcome 仍应为空");

        runNextClientTask();
        runNextClientTask();
        runNextClientTask();

        assertEquals(3, bridgeCalls.get(), "三条真实 protobuf payload 必须各 bridge 恰好一次");
        assertEquals(1, recipeNotifications.get(), "recipe hydration listener 必须恰好一次");
        assertEquals(1, sessionNotifications.get(), "session hydration listener 必须恰好一次");
        assertEquals(1, outcomeNotifications.get(), "outcome listener 必须恰好一次");
        assertEquals(1, completeSounds.get(), "completed outcome 必须恰好一声完成音");
        assertEquals(1, refreshes.get(), "completed outcome 必须恰好一次 refresh");
        assertEquals("craft.prejoin.recipe", CraftStore.recipes().get(0).id());
        assertEquals("workbench", CraftStore.recipes().get(0).station());
        assertTrue(CraftStore.sessionState().active());
        assertEquals("craft.prejoin.recipe", CraftStore.sessionState().recipeId().orElseThrow());
        assertEquals("craft.prejoin.outcome", CraftStore.lastOutcome().orElseThrow().recipeId());
        assertEquals(2_120L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "pre-JOIN payload drain 后 freshness 必须取同 token 收包时刻的单调最大值");
        assertEquals(
            List.of(
                "recipes=1@" + CLIENT_THREAD,
                "session=craft.prejoin.recipe@" + CLIENT_THREAD,
                "outcome=craft.prejoin.outcome@" + CLIENT_THREAD,
                "flash=6@" + CLIENT_THREAD,
                "sound@" + CLIENT_THREAD,
                "refresh@" + CLIENT_THREAD
            ),
            events,
            "三条 hydration/outcome 必须按提交顺序且所有 listener/sound/refresh 都在 client thread；实际="
                + events
        );
        assertTrue(clientTasks.isEmpty(), "完整 drain 后不得残留 task；实际=" + clientTasks);
    }

    @Test
    void rawReceiverCapturesOldHandlerBeforeBufferAccessAcrossReconnect() {
        ClientPlayNetworkHandler oldHandler = newTestHandler();
        ClientConnectionStatusStore.initializeSession(oldHandler);
        assertTrue(ClientConnectionStatusStore.activateSession(oldHandler, 4_000L),
            "old concrete handler must be active before the raw callback enters");

        AtomicInteger bridgeCalls = new AtomicInteger();
        AtomicInteger routeCalls = new AtomicInteger();
        AtomicInteger storeCalls = new AtomicInteger();
        AtomicInteger sounds = new AtomicInteger();
        AtomicInteger refreshes = new AtomicInteger();
        AtomicInteger applyCalls = new AtomicInteger();
        AtomicInteger bufferAccesses = new AtomicInteger();
        AtomicInteger receiveTimeCaptures = new AtomicInteger();
        CraftStore.addOutcomeListener(event -> {
            storeCalls.incrementAndGet();
            CraftOutcomeFeedback.apply(
                event,
                ticks -> {},
                sounds::incrementAndGet,
                refreshes::incrementAndGet
            );
        });
        ServerDataRouter router = staleProbeRouter(routeCalls);
        BongNetworkHandler.ServerDataPayloadBridge countingBridge = bytes -> {
            bridgeCalls.incrementAndGet();
            return ProtoServerDataBridge.bridge(bytes);
        };

        CountDownLatch envelopeCaptured = new CountDownLatch(1);
        CountDownLatch releaseOldReceiver = new CountDownLatch(1);
        PacketByteBuf oldPayload = trackingBuffer(
            staleProbe("craft.raw.old").getBytes(StandardCharsets.UTF_8), bufferAccesses);
        AtomicReference<Boolean> oldScheduled = new AtomicReference<>();
        AtomicReference<Throwable> receiverFailure = new AtomicReference<>();
        Thread oldReceiver = new Thread(() -> {
            try {
                oldScheduled.set(BongNetworkHandler.receiveServerDataPayload(
                    oldHandler,
                    oldPayload,
                    clientTasks::add,
                    (dispatch, type) -> applyCalls.incrementAndGet(),
                    () -> {
                        receiveTimeCaptures.incrementAndGet();
                        return 5_000L;
                    },
                    () -> {
                        envelopeCaptured.countDown();
                        awaitLatch(releaseOldReceiver, "release old raw receiver");
                    },
                    router,
                    countingBridge
                ));
            } catch (Throwable throwable) {
                receiverFailure.set(throwable);
            }
        }, NETWORK_THREAD + "-old");
        oldReceiver.start();
        awaitLatch(envelopeCaptured, "old raw receiver envelope capture");

        assertEquals(1, receiveTimeCaptures.get(),
            "raw callback must capture receivedAt exactly once before the barrier");
        assertEquals(0, bufferAccesses.get(),
            "barrier is immediately after token/time capture, so old buffer must still be untouched");
        assertTrue(ClientConnectionStatusStore.invalidateSession(oldHandler, 6_000L),
            "disconnect must invalidate old concrete handler while its callback is paused");

        ClientPlayNetworkHandler newHandler = newTestHandler();
        ClientConnectionStatusStore.SessionToken newToken =
            ClientConnectionStatusStore.initializeSession(newHandler);
        assertTrue(ClientConnectionStatusStore.activateSession(newHandler, 7_000L),
            "JOIN must activate the new concrete handler before releasing the old callback");

        releaseOldReceiver.countDown();
        joinThread(oldReceiver, receiverFailure);
        assertEquals(Boolean.TRUE, oldScheduled.get(),
            "old raw callback had a valid old token and should only become stale at task execution");
        assertTrue(bufferAccesses.get() > 0,
            "released raw callback may copy the payload through any supported ByteBuf API, but only after token/time capture");
        assertEquals(1, clientTasks.size(), "old callback should queue one token-bound client task");
        runNextClientTask();

        assertEquals(0, bridgeCalls.get(), "stale raw payload must be rejected before protobuf bridge");
        assertEquals(0, routeCalls.get(), "stale raw payload must be rejected before router");
        assertEquals(0, storeCalls.get(), "stale raw payload must not write store or notify listener");
        assertEquals(0, sounds.get(), "stale raw payload must preserve craft sound no-op semantics");
        assertEquals(0, refreshes.get(), "stale raw payload must not refresh screens");
        assertEquals(0, applyCalls.get(), "stale raw payload must not reach dispatch applier");
        assertTrue(CraftStore.lastOutcome().isEmpty(), "stale raw payload must leave CraftStore empty");
        assertTrue(ClientConnectionStatusStore.isActiveSession(newToken),
            "old callback must not invalidate the new handler token");
        assertEquals(7_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "old receivedAt must not alter new-session freshness");

        PacketByteBuf newPayload = trackingBuffer(
            staleProbe("craft.raw.new").getBytes(StandardCharsets.UTF_8), bufferAccesses);
        assertTrue(BongNetworkHandler.receiveServerDataPayload(
            newHandler,
            newPayload,
            clientTasks::add,
            (dispatch, type) -> applyCalls.incrementAndGet(),
            () -> 7_500L,
            () -> {},
            router,
            countingBridge
        ), "new concrete handler raw payload must queue successfully");
        assertEquals(1, clientTasks.size(),
            "new-handler raw callback must queue exactly one client task; actual=" + clientTasks);
        runNextClientTask();
        assertTrue(clientTasks.isEmpty(),
            "new-handler payload must not leave nested or duplicate tasks; actual=" + clientTasks);

        assertEquals(1, bridgeCalls.get(), "only the new payload must bridge exactly once");
        assertEquals(1, routeCalls.get(), "only the new payload must route exactly once");
        assertEquals(1, storeCalls.get(), "new payload store/listener must run exactly once");
        assertEquals(1, sounds.get(), "new completed payload must preserve exactly one craft sound");
        assertEquals(1, refreshes.get(), "new completed payload must refresh exactly once");
        assertEquals(1, applyCalls.get(), "new payload dispatch must apply exactly once");
        assertEquals("craft.raw.new", CraftStore.lastOutcome().orElseThrow().recipeId());
        assertEquals(7_500L, ClientConnectionStatusStore.lastPayloadAtMsForTests());
    }

    @Test
    void rawReceiverFailsClosedBeforeBufferAndClockForUnknownHandler() {
        AtomicInteger bufferAccesses = new AtomicInteger();
        AtomicInteger clockCalls = new AtomicInteger();
        PacketByteBuf payload = trackingBuffer(
            staleProbe("craft.raw.unregistered").getBytes(StandardCharsets.UTF_8), bufferAccesses);

        assertFalse(BongNetworkHandler.receiveServerDataPayload(
            newTestHandler(),
            payload,
            clientTasks::add,
            (dispatch, type) -> {},
            () -> {
                clockCalls.incrementAndGet();
                return 8_000L;
            },
            () -> {},
            staleProbeRouter(new AtomicInteger()),
            ProtoServerDataBridge::bridge
        ), "unregistered concrete handler must fail closed at raw callback entry");

        assertFalse(BongNetworkHandler.receiveServerDataPayload(
            null,
            payload,
            clientTasks::add,
            (dispatch, type) -> {},
            () -> {
                clockCalls.incrementAndGet();
                return 8_100L;
            },
            () -> {},
            staleProbeRouter(new AtomicInteger()),
            ProtoServerDataBridge::bridge
        ), "null concrete handler must fail closed at raw callback entry");

        assertEquals(0, clockCalls.get(), "unknown/null handlers must fail before receivedAt capture");
        assertEquals(0, bufferAccesses.get(), "unknown/null handlers must fail before any buffer access");
        assertTrue(clientTasks.isEmpty(), "unknown/null handlers must not queue a task");
    }

    @Test
    void staleHandlerTaskHasZeroEffectsAfterDisconnectAndNewHandlerJoin() {
        AtomicInteger bridgeCalls = new AtomicInteger();
        AtomicInteger routeCalls = new AtomicInteger();
        AtomicInteger storeCalls = new AtomicInteger();
        AtomicInteger sounds = new AtomicInteger();
        AtomicInteger refreshes = new AtomicInteger();
        AtomicInteger applyCalls = new AtomicInteger();
        CraftStore.addOutcomeListener(event -> {
            storeCalls.incrementAndGet();
            CraftOutcomeFeedback.apply(
                event,
                ticks -> {},
                sounds::incrementAndGet,
                refreshes::incrementAndGet
            );
        });
        ServerDataRouter router = staleProbeRouter(routeCalls);
        BongNetworkHandler.ServerDataPayloadBridge countingBridge = bytes -> {
            bridgeCalls.incrementAndGet();
            return ProtoServerDataBridge.bridge(bytes);
        };

        runNamedThread(NETWORK_THREAD, () -> assertTrue(dispatch(
            activeHandler,
            staleProbe("craft.old.handler").getBytes(StandardCharsets.UTF_8),
            router,
            (dispatch, type) -> applyCalls.incrementAndGet(),
            5_000L,
            countingBridge
        ), "已 INIT 的 handler A payload 必须成功排队"));
        assertEquals(1, clientTasks.size());

        assertTrue(ClientConnectionStatusStore.invalidateSession(activeHandler, 8_000L),
            "DISCONNECT 必须同步移除 handler A token");
        Object handlerB = new Object();
        ClientConnectionStatusStore.SessionToken tokenB =
            ClientConnectionStatusStore.initializeSession(handlerB);
        assertTrue(ClientConnectionStatusStore.activateSession(handlerB, 9_000L),
            "新 handler B JOIN 必须激活自己的 INIT token");
        assertEquals(9_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests());

        runNextClientTask();

        assertEquals(0, bridgeCalls.get(), "stale A task 必须在 bridge 前整段 no-op");
        assertEquals(0, routeCalls.get(), "stale A task 不得 route");
        assertEquals(0, storeCalls.get(), "stale A task 不得写 store/通知 listener");
        assertEquals(0, sounds.get(), "stale A task 不得播放完成音");
        assertEquals(0, refreshes.get(), "stale A task 不得 refresh screen");
        assertEquals(0, applyCalls.get(), "stale A task 不得 apply dispatch");
        assertTrue(CraftStore.lastOutcome().isEmpty(), "stale A task 不得污染 handler B 的 CraftStore");
        assertTrue(ClientConnectionStatusStore.isActiveSession(tokenB), "handler B 必须保持 active");
        assertEquals(9_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "stale A receivedAt 不得污染 handler B freshness");

        runNamedThread(NETWORK_THREAD, () -> assertTrue(dispatch(
            handlerB,
            staleProbe("craft.new.handler").getBytes(StandardCharsets.UTF_8),
            router,
            (dispatch, type) -> applyCalls.incrementAndGet(),
            9_500L,
            countingBridge
        ), "handler B 合法 payload 必须仍可排队"));
        runNextClientTask();

        assertEquals(1, bridgeCalls.get(), "只有 handler B 合法 payload 应 bridge 一次");
        assertEquals(1, routeCalls.get(), "只有 handler B 合法 payload 应 route 一次");
        assertEquals(1, storeCalls.get(), "handler B store/listener 必须 exactly once");
        assertEquals(1, sounds.get(), "handler B completed 反馈必须恰好一声");
        assertEquals(1, refreshes.get(), "handler B completed 反馈必须恰好一次 refresh");
        assertEquals(1, applyCalls.get(), "handler B legacy dispatch 必须 exactly once");
        assertEquals("craft.new.handler", CraftStore.lastOutcome().orElseThrow().recipeId());
        assertEquals(9_500L, ClientConnectionStatusStore.lastPayloadAtMsForTests());
    }

    @Test
    void lateOldHandlerJoinCannotReclaimNewActiveSession() {
        Object oldHandler = activeHandler;
        ClientConnectionStatusStore.SessionToken oldToken =
            ClientConnectionStatusStore.sessionToken(oldHandler).orElseThrow();
        Object newHandler = new Object();
        ClientConnectionStatusStore.SessionToken newToken =
            ClientConnectionStatusStore.initializeSession(newHandler);
        assertTrue(BongNetworkHandler.joinSession(newHandler, 9_000L, () -> {}),
            "handler B JOIN 必须先激活最新 INIT token");
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.new.session.sentinel", "rough_handle", 1, 9L));

        assertFalse(BongNetworkHandler.joinSession(oldHandler, 9_100L, () -> {}),
            "handler B 已激活后，handler A 的迟到 JOIN 必须 fail closed");

        assertFalse(ClientConnectionStatusStore.isActiveSession(oldToken),
            "迟到旧 JOIN 不得重新激活 handler A token");
        assertTrue(ClientConnectionStatusStore.isActiveSession(newToken),
            "迟到旧 JOIN 不得使 handler B token 失活");
        assertEquals(
            "craft.new.session.sentinel",
            CraftStore.lastOutcome().orElseThrow().recipeId(),
            "迟到旧 JOIN 不得清空或污染 handler B 已写入的 CraftStore"
        );
        assertEquals(9_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "迟到旧 JOIN 不得回退或覆盖 handler B freshness");
    }

    @Test
    void newerInitMakesOlderPreJoinHandlerIneligibleToActivate() {
        ClientConnectionStatusStore.resetForTests();
        Object oldHandler = new Object();
        ClientConnectionStatusStore.SessionToken oldToken =
            ClientConnectionStatusStore.initializeSession(oldHandler);
        Object newHandler = new Object();
        ClientConnectionStatusStore.SessionToken newToken =
            ClientConnectionStatusStore.initializeSession(newHandler);

        assertFalse(BongNetworkHandler.joinSession(oldHandler, 10_000L, () -> {}),
            "观察到 handler B INIT 后，handler A 的迟到首次 JOIN 必须 fail closed");
        assertFalse(ClientConnectionStatusStore.isActiveSession(oldToken),
            "较旧 pre-JOIN handler 不得夺回尚未激活的全局 session");
        assertTrue(BongNetworkHandler.joinSession(newHandler, 10_100L, () -> {}),
            "最新 INIT 的 handler B 仍必须可正常 JOIN");
        assertTrue(ClientConnectionStatusStore.isActiveSession(newToken),
            "handler B JOIN 后必须保持 active");
        assertEquals(10_100L, ClientConnectionStatusStore.lastPayloadAtMsForTests());
    }

    @Test
    void lateOldHandlerDisconnectDoesNotQueueCleanupOrClearNewSession() {
        Object oldHandler = activeHandler;
        Object newHandler = new Object();
        ClientConnectionStatusStore.SessionToken newToken =
            ClientConnectionStatusStore.initializeSession(newHandler);
        assertTrue(ClientConnectionStatusStore.activateSession(newHandler, 9_000L),
            "handler B JOIN 必须先成为当前 active session");
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.new.session.sentinel", "rough_handle", 1, 9L));
        List<Runnable> cleanupTasks = new CopyOnWriteArrayList<>();

        BongNetworkHandler.disconnectSession(
            oldHandler, 9_100L, BongNetworkHandler::clearClientStateOnDisconnect, cleanupTasks::add);
        assertEquals(1, cleanupTasks.size(),
            "late old disconnect enters one token-aware client task before it can be classified");
        runNamedThread(CLIENT_THREAD, cleanupTasks.remove(0));

        assertTrue(cleanupTasks.isEmpty(),
            "迟到旧 handler DISCONNECT 的原子 task 执行后不得再排全局 cleanup；实际=" + cleanupTasks);
        assertTrue(ClientConnectionStatusStore.sessionToken(oldHandler).isEmpty(),
            "迟到旧 handler token 仍必须同步从注册表移除");
        assertTrue(ClientConnectionStatusStore.isActiveSession(newToken),
            "迟到旧 handler DISCONNECT 不得使 handler B 失活");
        assertEquals(
            "craft.new.session.sentinel",
            CraftStore.lastOutcome().orElseThrow().recipeId(),
            "迟到旧 handler DISCONNECT 不得清空 handler B 已写入的 CraftStore"
        );
        assertEquals(9_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "迟到旧 handler DISCONNECT 不得回退 handler B freshness");
    }

    @Test
    void unregisteredOrNullHandlerFailsClosedBeforeQueueBridgeAndRoute() {
        AtomicInteger bridgeCalls = new AtomicInteger();
        AtomicInteger routeCalls = new AtomicInteger();
        AtomicInteger applyCalls = new AtomicInteger();
        ServerDataRouter router = staleProbeRouter(routeCalls);
        BongNetworkHandler.ServerDataPayloadBridge countingBridge = bytes -> {
            bridgeCalls.incrementAndGet();
            return ProtoServerDataBridge.bridge(bytes);
        };
        byte[] payload = staleProbe("craft.unregistered").getBytes(StandardCharsets.UTF_8);

        assertFalse(dispatch(
            new Object(), payload, router, (dispatch, type) -> applyCalls.incrementAndGet(),
            3_000L, countingBridge
        ), "未经过 INIT 的 handler 必须 fail closed");
        assertFalse(dispatch(
            null, payload, router, (dispatch, type) -> applyCalls.incrementAndGet(),
            3_100L, countingBridge
        ), "null handler 必须 fail closed");

        assertTrue(clientTasks.isEmpty(), "fail-closed handler 不得排 client task；实际=" + clientTasks);
        assertEquals(0, bridgeCalls.get(), "fail-closed handler 不得 bridge");
        assertEquals(0, routeCalls.get(), "fail-closed handler 不得 route");
        assertEquals(0, applyCalls.get(), "fail-closed handler 不得 apply dispatch");
        assertTrue(CraftStore.lastOutcome().isEmpty(), "fail-closed handler 不得写 store");
        assertEquals(1_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "fail-closed handler 不得刷新当前 session freshness");
    }

    @Test
    void unknownTypeAndNullDispatchAreNoOpSeamsAndDoNotPoisonFollowingPayload() {
        List<String> applied = new CopyOnWriteArrayList<>();
        ServerDataRouter router = new ServerDataRouter(Map.of(
            "null_dispatch_probe",
            envelope -> null,
            "ok_probe",
            envelope -> {
                applied.add("route@" + Thread.currentThread().getName());
                return ServerDataDispatch.handledWithLegacyMessage(
                    envelope.type(), "ok", "ok probe");
            }
        ));

        runNamedThread(NETWORK_THREAD, () -> {
            dispatch(
                activeHandler,
                "{\"v\":1,\"type\":\"totally_unknown_type_xyz\"}".getBytes(StandardCharsets.UTF_8),
                new ServerDataRouter(Map.of()),
                (dispatch, type) -> applied.add("apply-unknown:" + type),
                10_000L,
                ProtoServerDataBridge::bridge
            );
            dispatch(
                activeHandler,
                "{\"v\":1,\"type\":\"null_dispatch_probe\"}".getBytes(StandardCharsets.UTF_8),
                router,
                (dispatch, type) -> applied.add("apply-null:" + type),
                10_100L,
                ProtoServerDataBridge::bridge
            );
            dispatch(
                activeHandler,
                "{\"v\":1,\"type\":\"ok_probe\"}".getBytes(StandardCharsets.UTF_8),
                router,
                (dispatch, type) -> applied.add("apply-ok:" + type + "@" + Thread.currentThread().getName()),
                10_200L,
                ProtoServerDataBridge::bridge
            );
        });

        assertEquals(3, clientTasks.size(), "unknown/null/ok 应各保留一个有序 client task");
        assertTrue(applied.isEmpty(), "drain 前不得 apply；实际=" + applied);
        assertTrue(CraftStore.lastOutcome().isEmpty(), "no-op 路径不得写 craft store");

        runNextClientTask();
        runNextClientTask();
        assertTrue(
            applied.isEmpty(),
            "unknown type 与 null dispatch 都不得调用 dispatchApplier；实际=" + applied
        );
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "unknown type 与 null dispatch 都不得写 craft store；实际=" + CraftStore.lastOutcome()
        );

        runNextClientTask();
        assertEquals(
            List.of("route@" + CLIENT_THREAD, "apply-ok:ok_probe@" + CLIENT_THREAD),
            applied,
            "no-op seam 不得毒死后续合法 payload；实际=" + applied
        );
    }

    @Test
    void craftScreenAndWorkbenchScreenFeedbackWaitForClientThread() {
        CraftScreen craftScreen = new CraftScreen();
        WorkbenchScreen workbenchScreen = new WorkbenchScreen();
        craftScreen.attachOutcomeListenerForTests();
        craftScreen.attachOutcomeListenerForTests();
        workbenchScreen.attachOutcomeListenerForTests();
        workbenchScreen.attachOutcomeListenerForTests();

        List<String> sharedOrder = new ArrayList<>();
        AtomicInteger completeSounds = new AtomicInteger();
        CraftStore.addOutcomeListener(event -> CraftOutcomeFeedback.apply(
            event,
            ticks -> sharedOrder.add("flash=" + ticks + "@" + Thread.currentThread().getName()),
            () -> {
                completeSounds.incrementAndGet();
                sharedOrder.add("sound@" + Thread.currentThread().getName());
            },
            () -> sharedOrder.add("refresh@" + Thread.currentThread().getName())
        ));

        dispatchDefaultOnNetworkThread(craftOutcome("completed", "craft.ui.completed"));

        assertEquals(0, craftScreen.flashTicksForTests(),
            "drain 前 CraftScreen 不得写 flashTicks；实际=" + craftScreen.flashTicksForTests());
        assertEquals(0, workbenchScreen.flashTicksForTests(),
            "drain 前 WorkbenchScreen 不得写 flashTicks；实际=" + workbenchScreen.flashTicksForTests());
        assertTrue(sharedOrder.isEmpty(), "drain 前不得触发 flash/sound/refresh；实际=" + sharedOrder);
        assertEquals(0, completeSounds.get(), "drain 前不得播放完成音");
        assertEquals(1, clientTasks.size());

        runNextClientTask();

        assertEquals(
            CraftOutcomeFeedback.COMPLETED_FLASH_TICKS,
            craftScreen.flashTicksForTests(),
            "CraftScreen completed 后 flashTicks 必须为 6；实际=" + craftScreen.flashTicksForTests()
        );
        assertEquals(
            CraftOutcomeFeedback.COMPLETED_FLASH_TICKS,
            workbenchScreen.flashTicksForTests(),
            "WorkbenchScreen completed 后 flashTicks 必须为 6；实际="
                + workbenchScreen.flashTicksForTests()
        );
        assertEquals(1, completeSounds.get(), "completed 必须恰好一声共享观察音；实际=" + completeSounds.get());
        assertEquals(
            List.of(
                "flash=6@" + CLIENT_THREAD,
                "sound@" + CLIENT_THREAD,
                "refresh@" + CLIENT_THREAD
            ),
            sharedOrder,
            "completed 反馈顺序必须 flash→sound→refresh 且全在 client thread；实际=" + sharedOrder
        );

        sharedOrder.clear();
        completeSounds.set(0);
        int craftFlash = craftScreen.flashTicksForTests();
        int wbFlash = workbenchScreen.flashTicksForTests();
        dispatchDefaultOnNetworkThread(craftOutcome("failed", "craft.ui.failed"));
        assertEquals(1, clientTasks.size());
        runNextClientTask();
        assertEquals(craftFlash, craftScreen.flashTicksForTests(),
            "failed 不得改 CraftScreen flashTicks；实际=" + craftScreen.flashTicksForTests());
        assertEquals(wbFlash, workbenchScreen.flashTicksForTests(),
            "failed 不得改 WorkbenchScreen flashTicks；实际=" + workbenchScreen.flashTicksForTests());
        assertEquals(0, completeSounds.get(), "failed 不得播放完成音；实际 sounds=" + completeSounds.get());
        assertEquals(
            List.of("refresh@" + CLIENT_THREAD),
            sharedOrder,
            "failed 只应 refresh；实际=" + sharedOrder
        );
        assertEquals(
            CraftStore.CraftOutcomeEvent.Kind.FAILED,
            CraftStore.lastOutcome().orElseThrow().kind()
        );

        craftScreen.detachOutcomeListenerForTests();
        workbenchScreen.detachOutcomeListenerForTests();
        CraftStore.clearAllListenersForTests();
        CraftScreen closedCraft = new CraftScreen();
        closedCraft.attachOutcomeListenerForTests();
        closedCraft.detachOutcomeListenerForTests();
        dispatchDefaultOnNetworkThread(craftOutcome("completed", "craft.ui.after.close"));
        runNextClientTask();
        assertEquals(
            0,
            closedCraft.flashTicksForTests(),
            "screen 关闭后 delayed completed 不得写 flashTicks；实际="
                + closedCraft.flashTicksForTests()
        );
    }

    @Test
    void runOnClientThreadExecutesInlineOrQueuesExactlyOnce() {
        List<Runnable> queued = new CopyOnWriteArrayList<>();
        AtomicInteger executions = new AtomicInteger();

        BongNetworkHandler.runOnClientThread(true, executions::incrementAndGet, queued::add);

        assertEquals(1, executions.get(),
            "client-thread path must execute inline exactly once");
        assertTrue(queued.isEmpty(),
            "client-thread path must not leave a nested task; actual=" + queued);

        BongNetworkHandler.runOnClientThread(false, executions::incrementAndGet, queued::add);

        assertEquals(1, executions.get(),
            "off-thread path must not execute before its client task drains");
        assertEquals(1, queued.size(),
            "off-thread path must queue exactly one atomic task; actual=" + queued);
        runNamedThread(CLIENT_THREAD, queued.remove(0));
        assertEquals(2, executions.get(),
            "queued off-thread task must execute exactly once when drained");
        assertTrue(queued.isEmpty(),
            "off-thread path must not leave nested or duplicate tasks; actual=" + queued);
    }

    @Test
    void disconnectQueuedOffThreadCannotInvalidatePayloadMidTask() {
        AtomicInteger bridgeCalls = new AtomicInteger();
        AtomicInteger routeCalls = new AtomicInteger();
        AtomicInteger storeCalls = new AtomicInteger();
        AtomicInteger sounds = new AtomicInteger();
        AtomicInteger refreshes = new AtomicInteger();
        AtomicInteger applyCalls = new AtomicInteger();
        AtomicInteger cleanups = new AtomicInteger();
        List<Runnable> disconnectTasks = new CopyOnWriteArrayList<>();
        CraftStore.addOutcomeListener(event -> {
            storeCalls.incrementAndGet();
            CraftOutcomeFeedback.apply(
                event,
                ticks -> {},
                sounds::incrementAndGet,
                refreshes::incrementAndGet
            );
        });
        BongNetworkHandler.ServerDataPayloadBridge countingBridge = bytes -> {
            bridgeCalls.incrementAndGet();
            return ProtoServerDataBridge.bridge(bytes);
        };
        ServerDataRouter router = new ServerDataRouter(Map.of(
            "stale_probe",
            envelope -> {
                routeCalls.incrementAndGet();
                BongNetworkHandler.disconnectSession(
                    activeHandler,
                    12_050L,
                    () -> {
                        assertFalse(
                            ClientConnectionStatusStore.connectedForTests(),
                            "active handler token 必须先失活，随后同一 client task 才能执行 registry cleanup"
                        );
                        cleanups.incrementAndGet();
                        BongNetworkHandler.clearClientStateOnDisconnect();
                    },
                    disconnectTasks::add
                );
                CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
                    envelope.payload().get("recipe_id").getAsString(), "rough_handle", 1, 1L));
                return ServerDataDispatch.handledWithLegacyMessage(
                    envelope.type(), "probe", "disconnect serialization probe handled");
            }
        ));

        runNamedThread(NETWORK_THREAD, () -> assertTrue(dispatch(
            activeHandler,
            staleProbe("craft.inflight.before.disconnect").getBytes(StandardCharsets.UTF_8),
            router,
            (dispatch, type) -> applyCalls.incrementAndGet(),
            12_000L,
            countingBridge
        ), "当前 handler payload 必须先排入 client task"));
        assertEquals(1, clientTasks.size(), "测试前置必须只有一条 payload task");

        runNextClientTask();

        assertEquals(1, bridgeCalls.get(), "in-flight payload 必须 bridge exactly once");
        assertEquals(1, routeCalls.get(), "in-flight payload 必须 route exactly once");
        assertEquals(1, storeCalls.get(), "disconnect 只能排在当前 client task 后，不能中途切断 store");
        assertEquals(1, sounds.get(), "in-flight completed payload 必须保留 exactly-once sound");
        assertEquals(1, refreshes.get(), "in-flight completed payload 必须保留 exactly-once refresh");
        assertEquals(1, applyCalls.get(), "in-flight payload 必须完成 apply exactly once");
        assertEquals(1, disconnectTasks.size(),
            "非 client-thread DISCONNECT 必须把 invalidate+cleanup 作为一个原子 task 排队");
        assertEquals(0, cleanups.get(), "payload task 尚未返回前 cleanup 不得插入执行");
        assertTrue(ClientConnectionStatusStore.connectedForTests(),
            "queued disconnect 尚未 drain 时 session 必须仍 active");

        runNamedThread(CLIENT_THREAD, disconnectTasks.remove(0));

        assertEquals(1, cleanups.get(), "当前 session disconnect 必须 cleanup exactly once");
        assertFalse(ClientConnectionStatusStore.connectedForTests(),
            "disconnect task drain 后 session 必须失活");
        assertTrue(CraftStore.lastOutcome().isEmpty(),
            "disconnect cleanup 必须清掉刚完成的旧 session outcome");
        assertTrue(disconnectTasks.isEmpty(), "disconnect 原子 task 执行后不得残留嵌套 cleanup");
    }

    @Test
    void queuedOldDisconnectCannotClearNewSessionAfterNewJoin() {
        Object oldHandler = activeHandler;
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.old.session.sentinel", "rough_handle", 1, 1L));
        List<Runnable> disconnectTasks = new CopyOnWriteArrayList<>();
        AtomicInteger cleanups = new AtomicInteger();

        BongNetworkHandler.disconnectSession(
            oldHandler,
            13_000L,
            () -> {
                cleanups.incrementAndGet();
                BongNetworkHandler.clearClientStateOnDisconnect();
            },
            disconnectTasks::add
        );
        assertEquals(1, disconnectTasks.size(),
            "off-thread old disconnect 必须精确排入一个 token-aware 原子 task");
        assertTrue(ClientConnectionStatusStore.connectedForTests(),
            "原子 disconnect task drain 前不得在提交线程同步 invalidate");

        Object newHandler = new Object();
        ClientConnectionStatusStore.SessionToken newToken =
            ClientConnectionStatusStore.initializeSession(newHandler);
        assertTrue(BongNetworkHandler.joinSession(newHandler, 14_000L, () -> {}),
            "新 handler B 必须在自己的 JOIN client task 内同步激活");
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.new.session.sentinel", "rough_handle", 1, 2L));

        runNamedThread(CLIENT_THREAD, disconnectTasks.remove(0));

        assertEquals(0, cleanups.get(),
            "A 的 queued disconnect 执行时发现 B 已 active，必须跳过全局 cleanup");
        assertTrue(ClientConnectionStatusStore.sessionToken(oldHandler).isEmpty(),
            "迟到 A disconnect task 仍须移除 A 自身 handler token");
        assertTrue(ClientConnectionStatusStore.isActiveSession(newToken),
            "A 的 queued disconnect 不得使 B token 失活");
        assertEquals(
            "craft.new.session.sentinel",
            CraftStore.lastOutcome().orElseThrow().recipeId(),
            "A 的 queued cleanup 不得清空 B 已写入的 CraftStore"
        );
        assertEquals(14_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "A 的 queued disconnect 不得污染 B freshness");
        assertTrue(disconnectTasks.isEmpty(), "迟到 disconnect task 后不得残留 cleanup task");
    }

    @Test
    void disconnectCleanupBeforeOldTaskDrainCannotResurrectStoresOrFreshness() {
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "pre.disconnect", "x", 1, 1L));
        assertTrue(
            CraftStore.lastOutcome().isPresent(),
            "precondition: disconnect 前 lastOutcome 必须有值；实际=" + CraftStore.lastOutcome()
        );
        AtomicInteger bridgeCalls = new AtomicInteger();
        BongNetworkHandler.ServerDataPayloadBridge countingBridge = bytes -> {
            bridgeCalls.incrementAndGet();
            return ProtoServerDataBridge.bridge(bytes);
        };

        runNamedThread(NETWORK_THREAD, () -> assertTrue(dispatch(
            activeHandler,
            craftOutcomeProto("queued.before.disconnect"),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            12_000L,
            countingBridge
        ), "disconnect 前已 INIT handler payload 必须成功排队"));

        List<Runnable> disconnectTasks = new CopyOnWriteArrayList<>();
        BongNetworkHandler.disconnectSession(
            activeHandler, 12_100L, BongNetworkHandler::clearClientStateOnDisconnect,
            disconnectTasks::add);
        assertEquals(1, disconnectTasks.size(),
            "当前 active handler 断线必须恰好排一份 invalidate+cleanup 原子 task");
        assertTrue(
            ClientConnectionStatusStore.connectedForTests(),
            "disconnect task drain 前提交线程不得同步 invalidate 当前 session"
        );
        runNamedThread(CLIENT_THREAD, disconnectTasks.remove(0));
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "disconnect cleanup 必须清空 CraftStore outcome；实际="
                + CraftStore.lastOutcome()
        );
        assertFalse(
            ClientConnectionStatusStore.connectedForTests(),
            "disconnect 后 connected 必须为 false"
        );
        long freshnessAfterCleanup = ClientConnectionStatusStore.lastPayloadAtMsForTests();

        runNextClientTask();

        assertEquals(0, bridgeCalls.get(), "cleanup 后 stale task 必须在 bridge 前丢弃");
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "cleanup 后 stale queued craft_outcome 不得回写 CraftStore；实际="
                + CraftStore.lastOutcome()
        );
        assertFalse(
            ClientConnectionStatusStore.connectedForTests(),
            "stale task 不得复活 connected"
        );
        assertEquals(
            freshnessAfterCleanup,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "stale task 不得复活或刷新 disconnect 后 freshness"
        );
    }

    private ServerDataRouter staleProbeRouter(AtomicInteger routeCalls) {
        return new ServerDataRouter(Map.of(
            "stale_probe",
            envelope -> {
                routeCalls.incrementAndGet();
                CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
                    envelope.payload().get("recipe_id").getAsString(), "rough_handle", 1, 1L));
                return ServerDataDispatch.handledWithLegacyMessage(
                    envelope.type(), "probe", "stale probe handled");
            }
        ));
    }

    private void dispatchDefaultOnNetworkThread(String json) {
        runNamedThread(NETWORK_THREAD, () -> dispatchDefault(json));
    }

    private void dispatchDefault(String json) {
        dispatch(json, ServerDataRouter.createDefault(), (dispatch, type) -> {});
    }

    private void dispatch(
        String json,
        ServerDataRouter router,
        BiConsumer<ServerDataDispatch, String> dispatchApplier
    ) {
        assertTrue(dispatch(
            activeHandler,
            json.getBytes(StandardCharsets.UTF_8),
            router,
            dispatchApplier,
            1_500L,
            ProtoServerDataBridge::bridge
        ), "测试 active handler 必须成功排入 payload task");
    }

    private boolean dispatch(
        Object handler,
        byte[] bytes,
        ServerDataRouter router,
        BiConsumer<ServerDataDispatch, String> dispatchApplier,
        long receivedAtMs,
        BongNetworkHandler.ServerDataPayloadBridge payloadBridge
    ) {
        return BongNetworkHandler.dispatchServerDataPayload(
            handler,
            bytes,
            router,
            clientTasks::add,
            dispatchApplier,
            receivedAtMs,
            payloadBridge
        );
    }

    private void dispatchOnNetworkThread(
        String json,
        ServerDataRouter router,
        BiConsumer<ServerDataDispatch, String> dispatchApplier
    ) {
        runNamedThread(NETWORK_THREAD, () -> dispatch(json, router, dispatchApplier));
    }

    private static ClientPlayNetworkHandler newTestHandler() {
        try {
            Field unsafeField = Unsafe.class.getDeclaredField("theUnsafe");
            unsafeField.setAccessible(true);
            Unsafe unsafe = (Unsafe) unsafeField.get(null);
            return (ClientPlayNetworkHandler) unsafe.allocateInstance(ClientPlayNetworkHandler.class);
        } catch (ReflectiveOperationException failure) {
            throw new AssertionError("failed to allocate identity-only ClientPlayNetworkHandler", failure);
        }
    }

    private static PacketByteBuf trackingBuffer(byte[] payload, AtomicInteger accesses) {
        ByteBuf delegate = Unpooled.wrappedBuffer(payload);
        return new PacketByteBuf(delegate) {
            @Override
            public int readableBytes() {
                accesses.incrementAndGet();
                return super.readableBytes();
            }

            @Override
            public ByteBuf readBytes(byte[] destination) {
                accesses.incrementAndGet();
                return super.readBytes(destination);
            }

            @Override
            public ByteBuf duplicate() {
                accesses.incrementAndGet();
                return super.duplicate();
            }

            @Override
            public ByteBuf copy() {
                accesses.incrementAndGet();
                return super.copy();
            }
        };
    }

    private static void awaitLatch(CountDownLatch latch, String description) {
        try {
            assertTrue(latch.await(5, TimeUnit.SECONDS), "timed out waiting for " + description);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new AssertionError("interrupted while waiting for " + description, interrupted);
        }
    }

    private static void joinThread(Thread thread, AtomicReference<Throwable> failure) {
        try {
            thread.join(5_000L);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new AssertionError("interrupted while joining " + thread.getName(), interrupted);
        }
        assertFalse(thread.isAlive(), "timed out joining " + thread.getName());
        if (failure.get() != null) {
            throw new AssertionError("test thread failed: " + thread.getName(), failure.get());
        }
    }

    private void runNextClientTask() {
        assertFalse(
            clientTasks.isEmpty(),
            "期望执行下一项前 client task queue 非空，因为调用方已提交 payload；实际 queue="
                + clientTasks
        );
        Runnable task = clientTasks.remove(0);
        runNamedThread(CLIENT_THREAD, task);
    }

    private static void runNamedThread(String name, Runnable task) {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        Thread thread = new Thread(() -> {
            try {
                task.run();
            } catch (Throwable throwable) {
                failure.set(throwable);
            }
        }, name);
        thread.start();
        try {
            thread.join();
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new AssertionError("等待测试线程被中断: " + name, interrupted);
        }
        if (failure.get() != null) {
            throw new AssertionError("测试线程执行失败: " + name, failure.get());
        }
    }

    private static String craftOutcome(String kind, String recipeId) {
        if ("failed".equals(kind)) {
            return """
                {"v":1,"type":"craft_outcome","kind":"failed","player_id":"offline:A",
                 "recipe_id":"%s","reason":"player_cancelled","material_returned":2,
                 "qi_refunded":0.0,"ts":1}
                """.formatted(recipeId);
        }
        return """
            {"v":1,"type":"craft_outcome","kind":"completed","player_id":"offline:A",
             "recipe_id":"%s","output_template":"rough_handle","output_count":1,
             "completed_at_tick":5000,"ts":1}
            """.formatted(recipeId);
    }

    private static String staleProbe(String recipeId) {
        return """
            {"v":1,"type":"stale_probe","recipe_id":"%s"}
            """.formatted(recipeId);
    }

    private static byte[] craftRecipeListProto() {
        Envelope.CraftRecipeEntry recipe = Envelope.CraftRecipeEntry.newBuilder()
            .setId("craft.prejoin.recipe")
            .setCategory(Envelope.CraftCategory.CRAFT_CATEGORY_TOOL)
            .setDisplayName("首包制作配方")
            .addMaterials(Envelope.CraftMaterialPair.newBuilder()
                .setTemplateId("rough_wood").setCount(2))
            .setQiCost(0.0)
            .setTimeTicks(40)
            .setOutput(Envelope.CraftOutputPair.newBuilder()
                .setTemplateId("rough_handle").setCount(1))
            .setRequirements(Envelope.CraftRequirements.newBuilder())
            .setUnlocked(true)
            .setStation("workbench")
            .build();
        return Envelope.ServerDataEnvelope.newBuilder()
            .setCraftRecipeList(Envelope.CraftRecipeList.newBuilder()
                .setV(1)
                .setPlayerId("offline:A")
                .addRecipes(recipe)
                .setTs(1))
            .build()
            .toByteArray();
    }

    private static byte[] craftSessionStateProto() {
        return Envelope.ServerDataEnvelope.newBuilder()
            .setCraftSessionState(Envelope.CraftSessionState.newBuilder()
                .setV(1)
                .setPlayerId("offline:A")
                .setActive(true)
                .setRecipeId("craft.prejoin.recipe")
                .setElapsedTicks(10)
                .setTotalTicks(40)
                .setCompletedCount(0)
                .setTotalCount(1)
                .setTs(2))
            .build()
            .toByteArray();
    }

    private static byte[] craftOutcomeProto(String recipeId) {
        return Envelope.ServerDataEnvelope.newBuilder()
            .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                .setCompleted(Envelope.CraftOutcomeCompleted.newBuilder()
                    .setV(1)
                    .setPlayerId("offline:A")
                    .setRecipeId(recipeId)
                    .setOutputTemplate("rough_handle")
                    .setOutputCount(1)
                    .setCompletedAtTick(5_000L)
                    .setTs(3)))
            .build()
            .toByteArray();
    }
}
