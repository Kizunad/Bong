package com.bong.client;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.craft.CraftStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataRouter;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
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
    }

    @AfterEach
    void tearDown() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        CastStateStore.resetForTests();
        UnifiedEventStore.resetForTests();
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
        assertTrue(clientTasks.isEmpty(), "执行单 payload 后不得遗留嵌套或重复 client task");
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
            kind + " outcome 不得在 Fabric network thread 提前写入 CraftStore"
        );
        assertTrue(listenerThreads.isEmpty(), kind + " outcome listener 不得在 receiver 返回前触发");
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
        assertTrue(listenerThreads.isEmpty(), "cast_sync listener 不得在 receiver 返回前触发");
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

        assertTrue(CraftStore.lastOutcome().isEmpty(), "坏 payload 后的合法 payload 也必须等待 client drain");
        assertEquals(2, clientTasks.size(), "坏 payload 与后续合法 payload 都应保留各自的有序 client task");

        runNextClientTask();
        assertTrue(CraftStore.lastOutcome().isEmpty(), "坏 payload task 不得产生 craft side effect");

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

        assertTrue(events.isEmpty(), "handler failure 与后续 handler 都不得在 network thread 执行");
        assertEquals(2, clientTasks.size(), "handler failure 不得取消后续 payload 的 client task");

        runNextClientTask();
        assertTrue(events.isEmpty(), "失败 handler 应被 router 安全收口为 no-op");
        runNextClientTask();
        assertEquals(List.of("ok@" + CLIENT_THREAD), events,
            "失败后的合法 handler 必须仍在 client thread 正常执行一次");
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
        BongNetworkHandler.dispatchServerDataPayload(
            json.getBytes(StandardCharsets.UTF_8),
            router,
            clientTasks::add,
            dispatchApplier
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
        assertFalse(clientTasks.isEmpty(), "client task queue 为空，无法执行下一项");
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
