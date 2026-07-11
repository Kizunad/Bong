package com.bong.client;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongNetworkHandlerTest {
    @AfterEach
    void resetUnknownTypeLogCache() {
        BongNetworkHandler.resetUnknownTypeLogTimesForTests();
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        DroppedItemStore.resetForTests();
    }

    @Test
    void firstUnknownTypeIsLoggable() {
        assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal", 1_000L));
    }

    @Test
    void repeatedUnknownTypeIsThrottledWithinWindow() {
        assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal", 1_000L));
        assertFalse(BongNetworkHandler.shouldLogNoOp("mystery_signal", 1_001L));
        assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal", 31_001L));
    }

    @Test
    void unknownTypeThrottleCacheStaysBounded() {
        int cacheLimit = BongNetworkHandler.unknownTypeLogCacheLimitForTests();

        for (int index = 0; index < cacheLimit * 4; index++) {
            assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal_" + index, 1_000L));
        }

        assertEquals(cacheLimit, BongNetworkHandler.unknownTypeLogCacheSizeForTests());
    }

    @Test
    void disconnectClearsCraftStoreToPreventReconnectSessionLock() {
        CraftStore.replaceRecipes(List.of(sampleCraftRecipe("basic.wood_handle", true)));
        CraftStore.replaceSession(new CraftSessionStateView(
            true, "basic.wood_handle", 20L, 100L, 1, 3, ""));
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "basic.wood_handle", "rough_handle", 1, 42L));
        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(
            "basic.wood_handle",
            new CraftStore.RecipeUnlockedEvent.Scroll("scroll.basic.wood_handle"),
            43L
        ));

        assertTrue(
            CraftStore.sessionState().active(),
            "测试前必须模拟断线前残留的 active craft session，否则无法锁住 reconnect lock 回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertEquals(
            0,
            CraftStore.recipes().size(),
            "断线必须清空旧 recipe list，避免新 server/session 复用上一连接 craft 表"
        );
        assertFalse(
            CraftStore.sessionState().active(),
            "断线必须把 active craft session 复位为 idle；否则重连后 CraftActionBar 会继续显示制作进行中"
        );
        assertFalse(
            CraftStore.lastOutcome().isPresent(),
            "断线必须清空 lastOutcome，避免上一连接出炉 toast 串到新 session"
        );
        assertFalse(
            CraftStore.lastUnlocked().isPresent(),
            "断线必须清空 lastUnlocked，避免上一连接解锁提示串到新 session"
        );
    }

    /**
     * plan-bughunt-dropped-loot-session-leak — DroppedItemStore.clearOnDisconnect() 此前
     * 定义了却没有被 clearClientStateOnDisconnect() 调用，切服/重连后旧 server 的地面掉落物
     * 坐标会在新 world 首个 dropped_loot_sync 抵达前被误渲染为当前 session 掉落物，G 键还会
     * 带着旧 instanceId 发 pickup 请求。本测试锁住"断线清理路径必须清空 DroppedItemStore"。
     */
    @Test
    void disconnectClearsDroppedItemStoreToPreventStaleSessionBleed() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            7001L, "main_pack", 0, 0,
            10.0, 64.0, 10.0, InventoryItem.simple("relic", "残器")
        ));
        assertEquals(
            1,
            DroppedItemStore.snapshot().size(),
            "测试前必须模拟断线前残留的地面掉落物，否则无法锁住 session leak 回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertEquals(
            0,
            DroppedItemStore.snapshot().size(),
            "断线必须清空 DroppedItemStore，否则旧 server 掉落物坐标会串到新 server world 渲染"
        );
        assertNull(
            DroppedItemStore.nearestTo(10.0, 64.0, 10.0),
            "断线后 nearestTo 必须返回 null，否则 G 键会带着旧 instanceId 向新 server 发 pickup 请求"
        );
    }

    private static CraftRecipe sampleCraftRecipe(String id, boolean unlocked) {
        return new CraftRecipe(
            id,
            CraftCategory.TOOL,
            "木柄",
            List.of(new CraftRecipe.MaterialEntry("rough_wood", 2)),
            0.0,
            100L,
            "rough_handle",
            1,
            CraftRecipe.Requirements.NONE,
            unlocked
        );
    }
}
