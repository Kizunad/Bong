package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.processing.state.FreshnessStore;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-interaction-intent-cleanup-v1 P1 — 感保鲜「收窄」行为锁定。
 *
 * <p>锁住的契约（{@link InspectScreen#maybeProbeFreshness}）：
 * <ul>
 *   <li>仅对「FreshnessStore 里已知有保鲜数据」的物品发 freshness_probe（不再对任意物品通配）。</li>
 *   <li>普通物品（store 无记录）不发任何包 —— 杜绝对凡物的无效探针。</li>
 *   <li>null 物品不发包。</li>
 *   <li>发包时 instance_id 透传，channel = bong:client_request。</li>
 * </ul>
 */
public class InspectScreenFreshnessProbeTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        FreshnessStore.clearForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    private static InventoryItem freshItem(long instanceId) {
        return InventoryItem.createFull(
            instanceId, "ling_cao", "灵草", 1, 1, 0.1, "common",
            "可保鲜的灵植。", 1, 0.8, 1.0
        );
    }

    @Test
    void probesWhenFreshnessStoreHasRecordForItem() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        long id = 4242L;
        FreshnessStore.upsert(Long.toString(id), 0.73f, "decay_lingcao");

        boolean sent0 = screen.maybeProbeFreshness(freshItem(id));

        assertTrue(sent0, "store 有记录的物品应发探针，期望 true 实际 false");
        assertEquals(1, sent.size(), "应恰好发一包 freshness_probe，实际 " + sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"freshness_probe\",\"v\":1,\"instance_id\":4242}",
            sent.get(0).body(),
            "instance_id 应透传为 4242"
        );
    }

    @Test
    void doesNotProbeWhenFreshnessStoreHasNoRecord() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        // store 为空：普通物品没有保鲜数据

        boolean sent0 = screen.maybeProbeFreshness(freshItem(7L));

        assertFalse(sent0, "store 无记录的物品不应发探针，期望 false 实际 true");
        assertTrue(sent.isEmpty(),
            "普通物品（无保鲜记录）不应发任何包，实际发了 " + sent.size() + " 包");
    }

    @Test
    void doesNotProbeForDifferentInstanceIdEvenIfAnotherItemHasRecord() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        FreshnessStore.upsert(Long.toString(100L), 0.5f, "decay");

        // 查的是 200，store 里只有 100 的记录 —— 不应误发
        boolean sent0 = screen.maybeProbeFreshness(freshItem(200L));

        assertFalse(sent0, "instance_id 不匹配 store 记录时不应发包");
        assertTrue(sent.isEmpty(),
            "200 无记录不应发包（store 只有 100），实际 " + sent.size());
    }

    @Test
    void doesNotProbeForNullItem() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertFalse(screen.maybeProbeFreshness(null), "null 物品不应发包");
        assertTrue(sent.isEmpty(), "null 物品不应发任何包");
    }
}
