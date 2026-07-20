package com.bong.client;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.craft.CraftOutcomeFeedback;
import com.bong.client.craft.CraftScreen;
import com.bong.client.craft.CraftStore;
import com.bong.client.craft.WorkbenchScreen;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.ui.ClientConnectionStatusStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BongServerDataThreadingTest {
    private static final String NETWORK_THREAD = "fabric-network-io-test";
    private static final String CLIENT_THREAD = "minecraft-client-render-test";

    private final List<Runnable> clientTasks = new CopyOnWriteArrayList<>();

    @BeforeEach
    void setUp() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        CastStateStore.resetForTests();
        UnifiedEventStore.resetForTests();
        ClientConnectionStatusStore.resetForTests();
        ClientConnectionStatusStore.markConnected(1_000L);
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
        long generation = ClientConnectionStatusStore.currentGeneration();

        runNamedThread(NETWORK_THREAD, () -> dispatch(
            craftOutcome("completed", "craft.freshness"),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            receiptAt,
            generation
        ));

        assertEquals(
            1_000L,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "receiver 返回前不得用 processing time 改 freshness；实际 lastPayloadAtMs="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertEquals(1, clientTasks.size(), "payload 应精确排入一个 client task");

        // 模拟 queue 堵塞：处理时刻远晚于收包时刻
        runNextClientTask();

        assertEquals(
            receiptAt,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "drain 后 freshness 必须是收包时刻 " + receiptAt + " 而非 processing time；实际="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertTrue(
            ClientConnectionStatusStore.connectedForTests(),
            "合法当前代次 payload 应保持 connected"
        );
    }

    @Test
    void disconnectBeforeDrainDoesNotResurrectConnectionStatus() {
        long generation = ClientConnectionStatusStore.currentGeneration();
        runNamedThread(NETWORK_THREAD, () -> dispatch(
            craftOutcome("completed", "craft.stale.after.disconnect"),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            3_000L,
            generation
        ));
        assertEquals(1, clientTasks.size());

        // 排队后、drain 前断线：generation 递增，connected=false
        ClientConnectionStatusStore.markDisconnected(4_000L);
        assertFalse(
            ClientConnectionStatusStore.connectedForTests(),
            "断线后 store 必须为 disconnected"
        );
        long disconnectedPayloadAt = ClientConnectionStatusStore.lastPayloadAtMsForTests();

        runNextClientTask();

        assertFalse(
            ClientConnectionStatusStore.connectedForTests(),
            "disconnect-before-drain 的 stale task 不得把 ClientConnectionStatusStore 复活为 connected；"
                + " actual connected=" + ClientConnectionStatusStore.connectedForTests()
        );
        assertEquals(
            disconnectedPayloadAt,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "stale task 不得刷新 lastPayloadAtMs；实际="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "stale generation task 必须整段 no-op，不得写 CraftStore；实际 lastOutcome="
                + CraftStore.lastOutcome()
        );
    }

    @Test
    void reconnectInvalidatesOldQueuedTaskWithoutPollutingNewGeneration() {
        long oldGeneration = ClientConnectionStatusStore.currentGeneration();
        runNamedThread(NETWORK_THREAD, () -> dispatch(
            craftOutcome("completed", "craft.old.generation"),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            5_000L,
            oldGeneration
        ));

        // 断线 + 重连：新 generation；新连接自身 freshness=9_000
        ClientConnectionStatusStore.markDisconnected(8_000L);
        ClientConnectionStatusStore.markConnected(9_000L);
        long newGeneration = ClientConnectionStatusStore.currentGeneration();
        assertTrue(newGeneration != oldGeneration, "reconnect 必须推进 connection generation");
        assertEquals(9_000L, ClientConnectionStatusStore.lastPayloadAtMsForTests());

        runNextClientTask();

        assertTrue(
            ClientConnectionStatusStore.connectedForTests(),
            "新连接应保持 connected"
        );
        assertEquals(
            9_000L,
            ClientConnectionStatusStore.lastPayloadAtMsForTests(),
            "旧 generation task 不得把新连接 freshness 污染为旧 receivedAt；实际="
                + ClientConnectionStatusStore.lastPayloadAtMsForTests()
        );
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "旧 generation task 不得写新连接 CraftStore；实际 lastOutcome="
                + CraftStore.lastOutcome()
        );

        // 新连接上的合法 payload 仍应按序 exactly-once 生效
        runNamedThread(NETWORK_THREAD, () -> dispatch(
            craftOutcome("completed", "craft.new.generation"),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            9_500L,
            newGeneration
        ));
        runNextClientTask();
        assertEquals(
            "craft.new.generation",
            CraftStore.lastOutcome().orElseThrow().recipeId(),
            "新 generation 合法 payload 必须仍然生效"
        );
        assertEquals(9_500L, ClientConnectionStatusStore.lastPayloadAtMsForTests());
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
            // unknown type → default no-op dispatch（handlers 里没有）
            dispatch(
                "{\"v\":1,\"type\":\"totally_unknown_type_xyz\"}",
                new ServerDataRouter(Map.of()),
                (dispatch, type) -> applied.add("apply-unknown:" + type),
                10_000L,
                ClientConnectionStatusStore.currentGeneration()
            );
            // explicit null dispatch seam
            dispatch(
                "{\"v\":1,\"type\":\"null_dispatch_probe\"}",
                router,
                (dispatch, type) -> applied.add("apply-null:" + type),
                10_100L,
                ClientConnectionStatusStore.currentGeneration()
            );
            dispatch(
                "{\"v\":1,\"type\":\"ok_probe\"}",
                router,
                (dispatch, type) -> applied.add("apply-ok:" + type + "@" + Thread.currentThread().getName()),
                10_200L,
                ClientConnectionStatusStore.currentGeneration()
            );
        });

        assertEquals(3, clientTasks.size(), "unknown/null/ok 应各保留一个有序 client task");
        assertTrue(applied.isEmpty(), "drain 前不得 apply；实际=" + applied);
        assertTrue(CraftStore.lastOutcome().isEmpty(), "no-op 路径不得写 craft store");

        runNextClientTask(); // unknown
        runNextClientTask(); // null dispatch
        assertTrue(
            applied.isEmpty(),
            "unknown type 与 null dispatch 都不得调用 dispatchApplier；实际=" + applied
        );
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "unknown type 与 null dispatch 都不得写 craft store；实际=" + CraftStore.lastOutcome()
        );

        runNextClientTask(); // ok
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
        workbenchScreen.attachOutcomeListenerForTests();

        List<String> sharedOrder = new ArrayList<>();
        AtomicInteger completeSounds = new AtomicInteger();

        // 额外挂一个共享反馈观察 listener，锁定 flash→sound→refresh 顺序契约
        // （与生产 CraftOutcomeFeedback 同序；不替代 screen flashTicks 断言）
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
        assertEquals(1, completeSounds.get(), "completed 必须恰好一声完成音；实际=" + completeSounds.get());
        assertEquals(
            List.of(
                "flash=6@" + CLIENT_THREAD,
                "sound@" + CLIENT_THREAD,
                "refresh@" + CLIENT_THREAD
            ),
            sharedOrder,
            "completed 反馈顺序必须 flash→sound→refresh 且全在 client thread；实际=" + sharedOrder
        );

        // failed：store/listener 更新，但无完成音、不改 flash
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

        // screen removed before drain：不得再写 flash / 播音
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
    void disconnectClearsCraftStoreAndDropsQueuedOutcomeSideEffects() {
        CraftStore.replaceRecipes(List.of());
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "pre.disconnect", "x", 1, 1L));
        assertTrue(
            CraftStore.lastOutcome().isPresent(),
            "precondition: disconnect 前 lastOutcome 必须有值；实际=" + CraftStore.lastOutcome()
        );

        long generation = ClientConnectionStatusStore.currentGeneration();
        runNamedThread(NETWORK_THREAD, () -> dispatch(
            craftOutcome("completed", "queued.before.disconnect"),
            ServerDataRouter.createDefault(),
            (dispatch, type) -> {},
            12_000L,
            generation
        ));

        // 生产 disconnect 清理链
        BongNetworkHandler.clearClientStateOnDisconnect();
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "clearClientStateOnDisconnect 必须清空 CraftStore outcome；实际="
                + CraftStore.lastOutcome()
        );
        assertFalse(
            ClientConnectionStatusStore.connectedForTests(),
            "disconnect 后 connected 必须为 false"
        );

        runNextClientTask();
        assertTrue(
            CraftStore.lastOutcome().isEmpty(),
            "断线后 stale queued craft_outcome 不得回写 CraftStore；实际="
                + CraftStore.lastOutcome()
        );
        assertFalse(
            ClientConnectionStatusStore.connectedForTests(),
            "stale task 不得复活 connected"
        );
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
        java.util.function.BiConsumer<ServerDataDispatch, String> dispatchApplier
    ) {
        dispatch(
            json,
            router,
            dispatchApplier,
            1_500L,
            ClientConnectionStatusStore.currentGeneration()
        );
    }

    private void dispatch(
        String json,
        ServerDataRouter router,
        java.util.function.BiConsumer<ServerDataDispatch, String> dispatchApplier,
        long receivedAtMs,
        long connectionGeneration
    ) {
        BongNetworkHandler.dispatchServerDataPayload(
            json.getBytes(StandardCharsets.UTF_8),
            router,
            clientTasks::add,
            dispatchApplier,
            receivedAtMs,
            connectionGeneration
        );
    }

    private void dispatchOnNetworkThread(
        String json,
        ServerDataRouter router,
        java.util.function.BiConsumer<ServerDataDispatch, String> dispatchApplier
    ) {
        runNamedThread(NETWORK_THREAD, () -> dispatch(json, router, dispatchApplier));
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
}
