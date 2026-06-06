package com.bong.client.network;

import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.junit.jupiter.params.provider.ValueSource;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * MineralProbeResultHandler 单测（plan-exploration-probe-return-v1 P0）。
 *
 * <p>测试策略：
 * <ul>
 *   <li>静态 helper（{@code colorByAbundance} / {@code denialMessage}）直接断言——纯逻辑无 MC 依赖。</li>
 *   <li>{@code handle()} 在无 MC 环境（MinecraftClient.getInstance()==null / player==null）下的守卫行为。</li>
 *   <li>畸形 payload 的 no-op 守卫（缺 kind / 缺字段 / 未知 kind）。</li>
 * </ul>
 */
public class MineralProbeResultHandlerTest {

    // ─── colorByAbundance 三档颜色 ────────────────────────────────────────────

    @Test
    void colorByAbundance_aboveAbundantThreshold_returnsGreen() {
        int color = MineralProbeResultHandler.colorByAbundance(51);
        assertEquals(0x6EE7B7, color,
            "remaining=51（>50）应返回 COLOR_ABUNDANT=0x6EE7B7（青绿/丰盛），实际=" + Integer.toHexString(color));
    }

    @Test
    void colorByAbundance_atAbundantBoundary_returnsAmber() {
        // remaining==50 → 处于 >=10 且 <=50 的区间，应为琥珀色
        int color = MineralProbeResultHandler.colorByAbundance(50);
        assertEquals(0xFCD34D, color,
            "remaining=50（boundary，不 > 50）应返回 COLOR_MODERATE=0xFCD34D（琥珀），实际=" + Integer.toHexString(color));
    }

    @Test
    void colorByAbundance_moderateRange_returnsAmber() {
        int color = MineralProbeResultHandler.colorByAbundance(25);
        assertEquals(0xFCD34D, color,
            "remaining=25（10-50 中段）应返回 COLOR_MODERATE=0xFCD34D（琥珀），实际=" + Integer.toHexString(color));
    }

    @Test
    void colorByAbundance_atSparseBoundary_returnsAmber() {
        // remaining==10 → >=10，仍为琥珀
        int color = MineralProbeResultHandler.colorByAbundance(10);
        assertEquals(0xFCD34D, color,
            "remaining=10（sparse 下界，>=10 还属 moderate）应返回 COLOR_MODERATE=0xFCD34D，实际=" + Integer.toHexString(color));
    }

    @Test
    void colorByAbundance_belowSparseThreshold_returnsRed() {
        int color = MineralProbeResultHandler.colorByAbundance(9);
        assertEquals(0xF87171, color,
            "remaining=9（<10）应返回 COLOR_SPARSE=0xF87171（赤/稀薄），实际=" + Integer.toHexString(color));
    }

    @Test
    void colorByAbundance_zero_returnsRed() {
        int color = MineralProbeResultHandler.colorByAbundance(0);
        assertEquals(0xF87171, color,
            "remaining=0 应返回 COLOR_SPARSE=0xF87171，实际=" + Integer.toHexString(color));
    }

    @Test
    void colorByAbundance_maxValue_returnsGreen() {
        int color = MineralProbeResultHandler.colorByAbundance(Integer.MAX_VALUE);
        assertEquals(0x6EE7B7, color,
            "remaining=MAX_VALUE 应返回 COLOR_ABUNDANT=0x6EE7B7，实际=" + Integer.toHexString(color));
    }

    // ─── denialMessage 五档 denial_reason ────────────────────────────────────

    @Test
    void denialMessage_realmTooLow_returnsRealmText() {
        String msg = MineralProbeResultHandler.denialMessage("realm_too_low");
        assertEquals("神识未及，凝脉方可感矿脉", msg,
            "realm_too_low 应返回境界不足文案，实际=" + msg);
    }

    @Test
    void denialMessage_outOfRange_returnsRangeText() {
        String msg = MineralProbeResultHandler.denialMessage("out_of_range");
        assertEquals("神识探之不及", msg,
            "out_of_range 应返回距离过远文案，实际=" + msg);
    }

    @Test
    void denialMessage_notMineralOre_returnsNotMineralText() {
        String msg = MineralProbeResultHandler.denialMessage("not_mineral_ore");
        assertEquals("此处并无灵脉", msg,
            "not_mineral_ore 应返回非矿文案，实际=" + msg);
    }

    @Test
    void denialMessage_staleOreIndex_returnsFuzzyText() {
        String msg = MineralProbeResultHandler.denialMessage("stale_ore_index");
        assertEquals("灵脉模糊，难以辨形", msg,
            "stale_ore_index 应返回兜底模糊文案，实际=" + msg);
    }

    @Test
    void denialMessage_mineralNotRegistered_returnsFuzzyText() {
        String msg = MineralProbeResultHandler.denialMessage("mineral_not_registered");
        assertEquals("灵脉模糊，难以辨形", msg,
            "mineral_not_registered 应返回兜底模糊文案，实际=" + msg);
    }

    @Test
    void denialMessage_null_returnsFuzzyText() {
        String msg = MineralProbeResultHandler.denialMessage(null);
        assertEquals("灵脉模糊，难以辨形", msg,
            "null reason 应返回兜底文案，实际=" + msg);
    }

    @Test
    void denialMessage_unknownReason_returnsFuzzyText() {
        String msg = MineralProbeResultHandler.denialMessage("some_unknown_reason");
        assertEquals("灵脉模糊，难以辨形", msg,
            "未知 reason 应退回兜底文案，实际=" + msg);
    }

    // ─── handle() 畸形 payload → no-op 守卫 ──────────────────────────────────

    @Test
    void handle_missingKind_returnsNoOp() {
        // payload 中缺 kind 字段 → no-op（player 是否存在无所谓，kind 缺失是第一道 guard）
        String json = """
            {"v":1,"type":"mineral_probe_result","remaining_units":30}
            """;
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        ServerDataDispatch dispatch = new MineralProbeResultHandler().handle(envelope);
        assertFalse(dispatch.handled(),
            "缺 kind 字段应返回 noOp（dispatch.handled()==false），实际 handled=" + dispatch.handled());
    }

    @Test
    void handle_unknownKind_returnsNoOp() {
        String json = """
            {"v":1,"type":"mineral_probe_result","kind":"invalid_kind"}
            """;
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        ServerDataDispatch dispatch = new MineralProbeResultHandler().handle(envelope);
        assertFalse(dispatch.handled(),
            "未知 kind 应返回 noOp，实际 handled=" + dispatch.handled());
    }

    @Test
    void handle_playerNullNoOpReturnedNotThrown_forFoundKind() {
        // 在 headless 测试 JVM 里 MinecraftClient.getInstance() 返回 null（或 player==null）。
        // handler 应返回 noOp 而非抛 NullPointerException。
        String json = """
            {"v":1,"type":"mineral_probe_result","kind":"found","remaining_units":30,"display_name_zh":"铁矿"}
            """;
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        // 预期：no-op 或 noOp（取决于 mc == null），绝不 throw
        assertDoesNotThrow(() -> {
            ServerDataDispatch dispatch = new MineralProbeResultHandler().handle(envelope);
            assertNotNull(dispatch,
                "handle() 在 headless 下应返回非 null dispatch，实际 null");
        }, "handle() 在 headless 下 kind=found 不应抛出任何异常");
    }

    @Test
    void handle_playerNullNoOpReturnedNotThrown_forDeniedKind() {
        String json = """
            {"v":1,"type":"mineral_probe_result","kind":"denied","denial_reason":"out_of_range"}
            """;
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        assertDoesNotThrow(() -> {
            ServerDataDispatch dispatch = new MineralProbeResultHandler().handle(envelope);
            assertNotNull(dispatch,
                "handle() 在 headless 下应返回非 null dispatch，实际 null");
        }, "handle() 在 headless 下 kind=denied 不应抛出任何异常");
    }

    @Test
    void handle_emptyPayload_doesNotThrow() {
        // 完全缺字段的 minimal payload
        String json = "{\"v\":1,\"type\":\"mineral_probe_result\"}";
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        assertDoesNotThrow(() -> {
            ServerDataDispatch dispatch = new MineralProbeResultHandler().handle(envelope);
            assertFalse(dispatch.handled(),
                "空 payload（缺 kind）应为 noOp，实际 handled=" + dispatch.handled());
        });
    }

    // ─── colorByAbundance 三档边界完整覆盖（参数化概要）────────────────────────

    @ParameterizedTest(name = "remaining={0} → 期望颜色类别={1}")
    @CsvSource({
        "100, ABUNDANT",
        "51,  ABUNDANT",
        "50,  MODERATE",
        "25,  MODERATE",
        "10,  MODERATE",
        "9,   SPARSE",
        "1,   SPARSE",
        "0,   SPARSE",
    })
    void colorByAbundance_parametrized(int remaining, String expected) {
        int color = MineralProbeResultHandler.colorByAbundance(remaining);
        int expectedColor = switch (expected.trim()) {
            case "ABUNDANT" -> 0x6EE7B7;
            case "MODERATE" -> 0xFCD34D;
            case "SPARSE"   -> 0xF87171;
            default -> throw new IllegalArgumentException("unknown tier: " + expected);
        };
        assertEquals(expectedColor, color,
            "remaining=" + remaining + " 应属于 " + expected + " 颜色（0x" + Integer.toHexString(expectedColor) + "），实际=0x" + Integer.toHexString(color));
    }
}
