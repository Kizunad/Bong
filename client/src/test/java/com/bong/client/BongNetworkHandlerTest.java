package com.bong.client;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongNetworkHandlerTest {
    @AfterEach
    void resetUnknownTypeLogCache() {
        BongNetworkHandler.resetUnknownTypeLogTimesForTests();
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
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
