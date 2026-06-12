package com.bong.client.combat.handler;

import com.bong.client.combat.store.HalfStepRechallengeStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-halfstep-rechallenge-integration-v1 P0 饱和测试：
 * HalfStepRechallengeHandler — active/inactive 状态转换 + readBoolean/readString/readLong
 * 畸形字段 fallback + ServerDataRouter wire key "half_step_rechallenge" 契约 pin。
 */
class HalfStepRechallengeHandlerTest {

    private HalfStepRechallengeHandler handler;

    @BeforeEach
    void setUp() {
        handler = new HalfStepRechallengeHandler();
        HalfStepRechallengeStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        HalfStepRechallengeStore.resetForTests();
    }

    // ─── 1. active=true → store replace ──────────────────────────────────────

    @Test
    void activeTruePayloadWritesStoreReplaceAndReturnsHandled() {
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\","
                + "\"active\":true,"
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":4032000,"
                + "\"at_tick\":10000}"
        ));

        assertTrue(dispatch.handled(),
            "active=true payload 应返回 handled=true；实际=" + dispatch.logMessage());

        HalfStepRechallengeStore.State state = HalfStepRechallengeStore.snapshot();
        assertTrue(state.active(),
            "active=true 后 store.active 必须为 true；实际=" + state.active());
        assertEquals("offline:Azure", state.charId(),
            "char_id 必须从 payload 正确写入 store；实际=" + state.charId());
        assertEquals(4032000L, state.windowUntilTick(),
            "rechallenge_window_until 必须从 payload 正确写入 store；实际=" + state.windowUntilTick());
        assertEquals(10000L, state.atTick(),
            "at_tick 必须从 payload 正确写入 store；实际=" + state.atTick());
    }

    @Test
    void activeTrueSetStoreIsNotNone() {
        HalfStepRechallengeStore.State before = HalfStepRechallengeStore.snapshot();
        assertFalse(before.active(), "store 初始应为 NONE/active=false");

        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\","
                + "\"active\":true,\"char_id\":\"offline:Beryl\","
                + "\"rechallenge_window_until\":2000000,\"at_tick\":5000}"
        ));

        HalfStepRechallengeStore.State after = HalfStepRechallengeStore.snapshot();
        assertTrue(after.active(),
            "active=true payload 后 store 不再是 NONE；实际=" + after.active());
        assertNotNull(after.charId(),
            "store.charId 不应为 null 在 active=true 之后");
    }

    // ─── 2. active=false → clear() ───────────────────────────────────────────

    @Test
    void activeFalsePayloadClearsStore() {
        // 先置为 active
        HalfStepRechallengeStore.replace(
            new HalfStepRechallengeStore.State(true, "offline:Azure", 4032000L, 10000L, 1000L)
        );
        assertTrue(HalfStepRechallengeStore.snapshot().active(),
            "前提：store 应为 active=true");

        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\","
                + "\"active\":false,"
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":0,"
                + "\"at_tick\":0}"
        ));

        assertTrue(dispatch.handled(),
            "active=false payload 应返回 handled=true；实际=" + dispatch.logMessage());

        HalfStepRechallengeStore.State state = HalfStepRechallengeStore.snapshot();
        assertFalse(state.active(),
            "active=false 后 store.active 必须为 false（clear() 应被调用）；实际=" + state.active());
        assertEquals(HalfStepRechallengeStore.State.NONE, state,
            "active=false 后 store 应等于 State.NONE；实际=" + state);
    }

    @Test
    void activeFalseOnAlreadyClearStoreIsNoOp() {
        // store 已是 NONE — active=false 应该不崩溃仍返回 handled
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":false,"
                + "\"char_id\":\"\",\"rechallenge_window_until\":0,\"at_tick\":0}"
        ));

        assertTrue(dispatch.handled(),
            "active=false 对已清空 store 不崩溃且返回 handled；实际=" + dispatch.logMessage());
        assertFalse(HalfStepRechallengeStore.snapshot().active(),
            "store 仍应为 inactive；实际=" + HalfStepRechallengeStore.snapshot().active());
    }

    // ─── 3. readBoolean fallback：缺字段、null、非法类型 ─────────────────────

    @Test
    void missingActiveFallsBackToTrueDefault() {
        // "active" 字段缺失 → fallback = true → trigger
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\","
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":1000,\"at_tick\":0}"
        ));

        assertTrue(HalfStepRechallengeStore.snapshot().active(),
            "缺少 active 字段时应 fallback=true 触发 replace；实际="
                + HalfStepRechallengeStore.snapshot().active());
    }

    @Test
    void nullActiveFallsBackToTrue() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":null,"
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":1000,\"at_tick\":0}"
        ));

        assertTrue(HalfStepRechallengeStore.snapshot().active(),
            "active=null 应 fallback=true；实际=" + HalfStepRechallengeStore.snapshot().active());
    }

    @Test
    void numericActiveZeroTreatedAsFalse() {
        // active=0 (numeric) → false → clear
        HalfStepRechallengeStore.replace(
            new HalfStepRechallengeStore.State(true, "offline:Azure", 1000L, 0L, 0L)
        );

        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":0,"
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":0,\"at_tick\":0}"
        ));

        assertFalse(HalfStepRechallengeStore.snapshot().active(),
            "active=0 (numeric) 应被解析为 false → clear()；实际="
                + HalfStepRechallengeStore.snapshot().active());
    }

    @Test
    void numericActiveNonZeroTreatedAsTrue() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":1,"
                + "\"char_id\":\"offline:Beryl\","
                + "\"rechallenge_window_until\":2000,\"at_tick\":500}"
        ));

        assertTrue(HalfStepRechallengeStore.snapshot().active(),
            "active=1 (numeric, nonzero) 应被解析为 true → replace；实际="
                + HalfStepRechallengeStore.snapshot().active());
    }

    // ─── 4. readString fallback：缺字段、null、非字符串 ──────────────────────

    @Test
    void missingCharIdFallsBackToEmptyString() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"rechallenge_window_until\":1000,\"at_tick\":0}"
        ));

        HalfStepRechallengeStore.State state = HalfStepRechallengeStore.snapshot();
        assertTrue(state.active(), "前提：store 应 active=true");
        assertEquals("", state.charId(),
            "缺失 char_id 字段时 charId 应 fallback 为空字符串；实际='" + state.charId() + "'");
    }

    @Test
    void nullCharIdFallsBackToEmptyString() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":null,"
                + "\"rechallenge_window_until\":1000,\"at_tick\":0}"
        ));

        assertEquals("", HalfStepRechallengeStore.snapshot().charId(),
            "char_id=null 应 fallback 为空字符串；实际='"
                + HalfStepRechallengeStore.snapshot().charId() + "'");
    }

    @Test
    void numericCharIdFallsBackToEmptyString() {
        // char_id 是数字 (非法类型) → fallback ""
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":42,"
                + "\"rechallenge_window_until\":1000,\"at_tick\":0}"
        ));

        assertEquals("", HalfStepRechallengeStore.snapshot().charId(),
            "char_id=42 (numeric) 应 fallback 为空字符串；实际='"
                + HalfStepRechallengeStore.snapshot().charId() + "'");
    }

    // ─── 5. readLong fallback：缺字段、null、非数字 ───────────────────────────

    @Test
    void missingWindowUntilFallsBackToZero() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:Azure\",\"at_tick\":100}"
        ));

        assertEquals(0L, HalfStepRechallengeStore.snapshot().windowUntilTick(),
            "缺失 rechallenge_window_until 应 fallback=0；实际="
                + HalfStepRechallengeStore.snapshot().windowUntilTick());
    }

    @Test
    void missingAtTickFallsBackToZero() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:Azure\",\"rechallenge_window_until\":5000}"
        ));

        assertEquals(0L, HalfStepRechallengeStore.snapshot().atTick(),
            "缺失 at_tick 应 fallback=0；实际="
                + HalfStepRechallengeStore.snapshot().atTick());
    }

    @Test
    void stringWindowUntilFallsBackToZero() {
        // rechallenge_window_until 是字符串 → readLong fallback 0
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":\"bad\",\"at_tick\":0}"
        ));

        assertEquals(0L, HalfStepRechallengeStore.snapshot().windowUntilTick(),
            "rechallenge_window_until=\"bad\" (string) 应 fallback=0；实际="
                + HalfStepRechallengeStore.snapshot().windowUntilTick());
    }

    @Test
    void nullWindowUntilFallsBackToZero() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:Azure\","
                + "\"rechallenge_window_until\":null,\"at_tick\":500}"
        ));

        assertEquals(0L, HalfStepRechallengeStore.snapshot().windowUntilTick(),
            "rechallenge_window_until=null 应 fallback=0；实际="
                + HalfStepRechallengeStore.snapshot().windowUntilTick());
    }

    // ─── 6. 全字段缺失（仅 v + type）不崩溃 ────────────────────────────────

    @Test
    void bareMinimalPayloadDoesNotThrow() {
        // 只有 v 和 type，其余全缺 — handler 不应抛出异常（所有 fallback 应生效）
        assertDoesNotThrow(() -> handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\"}"
        )), "仅有 v+type 的 payload 不应抛出异常（所有字段均有 fallback）");
    }

    @Test
    void bareMinimalPayloadTriggersReplace() {
        // active 缺失 → fallback true → replace（而非 clear）
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\"}"
        ));

        assertTrue(HalfStepRechallengeStore.snapshot().active(),
            "全字段缺失时 active fallback=true → replace；实际="
                + HalfStepRechallengeStore.snapshot().active());
    }

    // ─── 7. last-write-wins：连续两次 active=true 后发 active=false ─────────

    @Test
    void lastWriteWinsOnConsecutiveActiveTriggers() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:First\",\"rechallenge_window_until\":1000,\"at_tick\":100}"
        ));
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:Second\",\"rechallenge_window_until\":2000,\"at_tick\":200}"
        ));

        HalfStepRechallengeStore.State state = HalfStepRechallengeStore.snapshot();
        assertEquals("offline:Second", state.charId(),
            "last-write-wins: 第二条 active payload 的 char_id 应覆盖第一条；实际=" + state.charId());
        assertEquals(2000L, state.windowUntilTick(),
            "last-write-wins: 第二条 windowUntilTick 应覆盖；实际=" + state.windowUntilTick());
    }

    @Test
    void activeThenFalseClearsStore() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":true,"
                + "\"char_id\":\"offline:Azure\",\"rechallenge_window_until\":9999,\"at_tick\":50}"
        ));
        assertTrue(HalfStepRechallengeStore.snapshot().active(), "前提：active=true 已写入");

        handler.handle(parse(
            "{\"v\":1,\"type\":\"half_step_rechallenge\",\"active\":false,"
                + "\"char_id\":\"offline:Azure\",\"rechallenge_window_until\":0,\"at_tick\":0}"
        ));

        assertFalse(HalfStepRechallengeStore.snapshot().active(),
            "active→false 序列后 store 应为 NONE；实际="
                + HalfStepRechallengeStore.snapshot().active());
    }

    // ─── 8. ServerDataRouter wire key 契约 pin ───────────────────────────────

    @Test
    void routerRegistersHalfStepRechallengeKey() {
        // 锁死 server wire type "half_step_rechallenge"（plan 契约：agent_bridge.rs L:HalfStepRechallenge）
        ServerDataRouter router = ServerDataRouter.createDefault();
        assertTrue(router.registeredTypes().contains("half_step_rechallenge"),
            "'half_step_rechallenge' 必须已在 ServerDataRouter 注册（server wire type；"
                + "注意旧错误拼写 'halfstep_rechallenge' 无下划线不被接受）；"
                + "实际注册类型=" + router.registeredTypes());
        assertFalse(router.registeredTypes().contains("halfstep_rechallenge"),
            "错误拼写 'halfstep_rechallenge'（无下划线）不应存在于 router；"
                + "server agent_bridge.rs 下发的是 'half_step_rechallenge'");
    }

    @Test
    void routerDispatchesHalfStepRechallengeActiveToStore() {
        // 完整路由链：JSON payload → ServerDataRouter.route() → handler → store
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"half_step_rechallenge\","
            + "\"active\":true,\"char_id\":\"offline:Azure\","
            + "\"rechallenge_window_until\":4032000,\"at_tick\":10000}";
        int size = json.getBytes(StandardCharsets.UTF_8).length;

        ServerDataRouter.RouteResult result = router.route(json, size);

        assertFalse(result.isParseError(),
            "路由 half_step_rechallenge 不应有 parse 错误；实际=" + result.logMessage());
        assertTrue(result.isHandled(),
            "ServerDataRouter 必须将 half_step_rechallenge 路由到 handler（非 noOp）；实际="
                + result.logMessage());

        HalfStepRechallengeStore.State state = HalfStepRechallengeStore.snapshot();
        assertTrue(state.active(),
            "router 路由后 store.active 应为 true；实际=" + state.active());
        assertEquals("offline:Azure", state.charId(),
            "router 路由后 store.charId 应为 'offline:Azure'；实际=" + state.charId());
        assertEquals(4032000L, state.windowUntilTick(),
            "router 路由后 windowUntilTick 应为 4032000；实际=" + state.windowUntilTick());
        assertEquals(10000L, state.atTick(),
            "router 路由后 atTick 应为 10000；实际=" + state.atTick());
    }

    @Test
    void routerDispatchesHalfStepRechallengeHideToClear() {
        // HIDE 路径：active=false → clear()
        HalfStepRechallengeStore.replace(
            new HalfStepRechallengeStore.State(true, "offline:Azure", 9999L, 100L, 0L)
        );

        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"half_step_rechallenge\","
            + "\"active\":false,\"char_id\":\"offline:Azure\","
            + "\"rechallenge_window_until\":0,\"at_tick\":0}";
        int size = json.getBytes(StandardCharsets.UTF_8).length;

        ServerDataRouter.RouteResult result = router.route(json, size);

        assertTrue(result.isHandled(),
            "HIDE payload 的路由结果应为 handled；实际=" + result.logMessage());
        assertFalse(HalfStepRechallengeStore.snapshot().active(),
            "HIDE 路由后 store 应已清空（active=false）；实际="
                + HalfStepRechallengeStore.snapshot().active());
    }

    // ─── Helper ──────────────────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(r.isSuccess(), () -> "test fixture parse failed: " + r.errorMessage());
        return r.envelope();
    }
}
