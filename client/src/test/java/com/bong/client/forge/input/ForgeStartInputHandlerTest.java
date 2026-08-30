package com.bong.client.forge.input;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.input.ClientInputPolicy;
import com.bong.client.tsy.ExtractStateStore;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-forge-session-entry-wiring-v1 §4.1 P1 —— {@link ForgeStartInputHandler} 发送契约测试。
 *
 * <p>覆盖三前置条件（station pos / blueprint id / materials）各自缺失时静默不发送，以及
 * happy path 单材料 / 多材料下发出的确切 JSON 形状。</p>
 */
class ForgeStartInputHandlerTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        ExtractStateStore.resetForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    private static Map<String, Integer> materials(Object... kv) {
        Map<String, Integer> map = new LinkedHashMap<>();
        for (int i = 0; i < kv.length; i += 2) {
            map.put((String) kv[i], (Integer) kv[i + 1]);
        }
        return map;
    }

    @Test
    void startsForgeWithSingleMaterial() {
        install();
        boolean handled = ForgeStartInputHandler.tryStartForge(
            new BlockPos(0, 64, 0), "iron_sword_v0", materials("fan_tie", 3)
        );

        assertTrue(handled);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"forge_start_session\",\"v\":1,\"station_pos\":[0,64,0],"
                + "\"blueprint_id\":\"iron_sword_v0\",\"materials\":[[\"fan_tie\",3]]}",
            sent.get(0).body()
        );
    }

    @Test
    void startsForgeWithMultipleMaterialsPreservingInsertionOrder() {
        install();
        Map<String, Integer> mats = materials("fan_tie", 4, "za_gang", 1);
        boolean handled = ForgeStartInputHandler.tryStartForge(
            new BlockPos(12, 66, -8), "qing_feng_v0", mats
        );

        assertTrue(handled);
        assertEquals(
            "{\"type\":\"forge_start_session\",\"v\":1,\"station_pos\":[12,66,-8],"
                + "\"blueprint_id\":\"qing_feng_v0\",\"materials\":[[\"fan_tie\",4],[\"za_gang\",1]]}",
            sent.get(0).body()
        );
    }

    @Test
    void refusesWhenStationPosUnknown() {
        install();
        boolean handled = ForgeStartInputHandler.tryStartForge(
            null, "iron_sword_v0", materials("fan_tie", 3)
        );

        assertFalse(handled, "station pos 未知（尚未收到任何 forge_station 快照）时不应发送");
        assertTrue(sent.isEmpty());
    }

    @Test
    void refusesWhenBlueprintIdNull() {
        install();
        boolean handled = ForgeStartInputHandler.tryStartForge(
            new BlockPos(0, 64, 0), null, materials("fan_tie", 3)
        );

        assertFalse(handled, "未选中任何图谱（BlueprintScrollStore.current()==null）时不应发送");
        assertTrue(sent.isEmpty());
    }

    @Test
    void refusesWhenBlueprintIdBlank() {
        install();
        boolean handled = ForgeStartInputHandler.tryStartForge(
            new BlockPos(0, 64, 0), "   ", materials("fan_tie", 3)
        );

        assertFalse(handled);
        assertTrue(sent.isEmpty());
    }

    @Test
    void refusesWhenMaterialsNull() {
        install();
        boolean handled = ForgeStartInputHandler.tryStartForge(
            new BlockPos(0, 64, 0), "iron_sword_v0", null
        );

        assertFalse(handled, "未点选任何投料时不应发送");
        assertTrue(sent.isEmpty());
    }

    @Test
    void refusesWhenMaterialsEmpty() {
        install();
        boolean handled = ForgeStartInputHandler.tryStartForge(
            new BlockPos(0, 64, 0), "iron_sword_v0", materials()
        );

        assertFalse(handled);
        assertTrue(sent.isEmpty());
    }

    // ── startModeAvailable（review #1141 major：门禁按 active 不按 sessionId） ──

    @Test
    void startModeAvailableWhenNoSessionEverReceived() {
        assertTrue(
            ForgeStartInputHandler.startModeAvailable(
                com.bong.client.forge.state.ForgeSessionStore.Snapshot.empty()),
            "从未收到会话快照（empty，active=false）应可起炉"
        );
    }

    @Test
    void startModeBlockedWhileSessionActive() {
        var active = new com.bong.client.forge.state.ForgeSessionStore.Snapshot(
            7, "qing_feng_v0", "青锋剑", true, "tempering", 1, 1, "{}");
        assertFalse(
            ForgeStartInputHandler.startModeAvailable(active),
            "活跃会话进行中不得再次起炉"
        );
    }

    @Test
    void startModeAvailableAfterSessionCompletesWithResidualSessionId() {
        // 结算后最后一帧快照 active=false 但 sessionId 残留 >0——按 sessionId 判定
        // 会把起炉入口永久卡死（打完一炉再也点不动投料/I 键），必须放行。
        var done = new com.bong.client.forge.state.ForgeSessionStore.Snapshot(
            7, "qing_feng_v0", "青锋剑", false, "done", 2, 2, "{}");
        assertTrue(
            ForgeStartInputHandler.startModeAvailable(done),
            "已完成会话（sessionId>0 但 active=false）必须放行再次起炉"
        );
    }

    @Test
    void forgeOpenInputIsAvailableWhenExtractionIsIdle() {
        assertTrue(
            ClientInputPolicy.shouldDispatchForgeOpen(),
            "非撤离状态允许消费 Forge 按键并请求打开锻炉屏幕"
        );
    }

    @Test
    void forgeOpenInputIsRejectedWhileExtractionIsActive() {
        ExtractStateStore.markStarted(42L, "tsy_lingxu_01", 20, 1_000L);

        assertFalse(
            ClientInputPolicy.shouldDispatchForgeOpen(),
            "撤离进行时即使 Forge 暂时回滚到历史 U，也不能派发 Forge 开屏"
        );
    }
}
