package com.bong.client.inventory;

import com.bong.client.block.BlockPickerGate;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import net.minecraft.world.GameMode;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-worldgen-v4 P5 §8.1#5 — 方块面板 give-block 真接线 e2e（防僵尸 UI）。
 *
 * <p>锁定「点击方块条目 → 真发 BlockPickerActionV1 payload」这条链路，断言 wire
 * payload 结构与 schema 对齐，而非占位 / dragBy 死代码。owo build() 需要渲染上下文，
 * headless 测不到，故直接验证条目点击委托的 {@link BlockPickerPanel#giveBlock} 行为。</p>
 */
class BlockPickerPanelTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        BlockPickerGate.resetForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void giveBlockSendsBlockPickerActionPayload() {
        install();
        boolean sentOk = BlockPickerPanel.giveBlock("stone_bricks", BlockPickerPanel.DEFAULT_GIVE_COUNT);

        assertTrue(sentOk, "giveBlock with valid args must report a send");
        assertEquals(1, sent.size(),
            "clicking a block row must produce exactly one client_request, actual sent=" + sent);
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"block_picker_give\",\"v\":1,\"block_id\":\"stone_bricks\",\"count\":"
                + BlockPickerPanel.DEFAULT_GIVE_COUNT + "}",
            sent.get(0).body(),
            "give-block payload must match the locked BlockPickerActionV1 schema, actual=" + sent.get(0).body()
        );
    }

    @Test
    void giveBlockRejectsBlankIdAndDoesNotSend() {
        install();
        assertFalse(BlockPickerPanel.giveBlock("", 1),
            "blank block_id must be rejected");
        assertFalse(BlockPickerPanel.giveBlock(null, 1),
            "null block_id must be rejected");
        assertEquals(0, sent.size(),
            "no payload should be sent for invalid block_id, actual sent=" + sent);
    }

    @Test
    void giveBlockRejectsOutOfRangeCountAndDoesNotSend() {
        install();
        assertFalse(BlockPickerPanel.giveBlock("stone", 0),
            "count=0 must be rejected (schema lower bound is 1)");
        assertFalse(BlockPickerPanel.giveBlock("stone", 65),
            "count=65 must be rejected (schema upper bound is 64)");
        assertEquals(0, sent.size(),
            "no payload should be sent for out-of-range count, actual sent=" + sent);
    }

    @Test
    void giveBlockAcceptsFullStackCount() {
        install();
        assertTrue(BlockPickerPanel.giveBlock("sand", 64),
            "count=64 (one stack) is the upper bound and must be accepted");
        assertEquals(
            "{\"type\":\"block_picker_give\",\"v\":1,\"block_id\":\"sand\",\"count\":64}",
            sent.get(0).body()
        );
    }

    @Test
    void inspectScreenMountGatedToDevCreativeOnly() {
        // dev/creative → mount; otherwise hidden (not greyed) per HUD conditional display.
        BlockPickerGate.setGameModeSupplierForTests(() -> GameMode.CREATIVE);
        assertTrue(InspectScreen.shouldMountBlockPickerForTests(),
            "creative must mount the dev block panel");

        BlockPickerGate.setGameModeSupplierForTests(() -> GameMode.SURVIVAL);
        assertFalse(InspectScreen.shouldMountBlockPickerForTests(),
            "survival must NOT mount the dev block panel — it stays hidden, not greyed");

        BlockPickerGate.setGameModeSupplierForTests(() -> null);
        assertFalse(InspectScreen.shouldMountBlockPickerForTests(),
            "no gamemode (not connected) must not mount the panel");
    }
}
