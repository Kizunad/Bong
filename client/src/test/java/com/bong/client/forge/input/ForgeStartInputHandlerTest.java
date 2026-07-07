package com.bong.client.forge.input;

import com.bong.client.network.ClientRequestSender;
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
}
