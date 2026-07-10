package com.bong.client;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.state.NarrationState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
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
        BongHudStateStore.clear();
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

    // ──────────────────────────────────────────────────────────────────────
    // plan-bughunt-hud-state-session-reset — BongHudStateStore 是生产 HUD 管线
    // 每帧读取的 static snapshot（见 BongHud.java 直接调用 BongHudStateStore.snapshot()）。
    // 断线清理清单此前没有 reset 它，导致旧 server 写入的 zoneState（区域 overlay/
    // atmosphere）与 visualEffectState（HUD tint/相机偏移/FOV）跨 session 残留，直到
    // 新服首个 zone_info 到达或旧 visual effect 自然过期才消失。
    // ──────────────────────────────────────────────────────────────────────

    @Test
    void disconnectResetsBongHudStateStoreToPreventCrossSessionZoneAndVisualEffectLeak() {
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            ZoneState.create("negative_qi_zone", "负灵域", 0.08, 6, 1_000L),
            NarrationState.create("broadcast", null, "旧服残留旁白", "narration"),
            VisualEffectState.create("near_death_vignette", 0.9, 30_000L, 0L)
        ));

        assertFalse(
            BongHudStateStore.snapshot().isEmpty(),
            "测试前置：必须先模拟旧 session 写入的非空 zoneState/visualEffectState，否则无法锁住残留回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        BongHudStateSnapshot afterDisconnect = BongHudStateStore.snapshot();
        assertTrue(
            afterDisconnect.isEmpty(),
            "断线必须把 BongHudStateStore 整体复位为 empty snapshot，否则新 session 首帧仍会用旧区域/旧特效渲染 HUD"
        );
        assertTrue(
            afterDisconnect.zoneState().isEmpty(),
            "断线必须清空 zoneState，避免新服首个 zone_info 到达前 HUD/atmosphere 继续显示上一服的负灵域"
        );
        assertTrue(
            afterDisconnect.visualEffectState().isEmpty(),
            "断线必须清空 visualEffectState，避免旧 near_death_vignette 在剩余 TTL 内继续污染新 session 的 HUD tint/相机/FOV"
        );
    }

    @Test
    void disconnectHudStateResetDoesNotBreakNormalReplaceAfterReconnect() {
        BongNetworkHandler.clearClientStateOnDisconnect();
        assertTrue(BongHudStateStore.snapshot().isEmpty(), "测试前置：断线后 store 应已复位为空");

        BongHudStateSnapshot newSessionSnapshot = BongHudStateSnapshot.create(
            ZoneState.create("qingyun_peaks", "青云残峰", 0.95, 0, 5_000L),
            NarrationState.empty(),
            VisualEffectState.none()
        );
        BongHudStateStore.replace(newSessionSnapshot);

        assertEquals(
            "qingyun_peaks",
            BongHudStateStore.snapshot().zoneState().zoneId(),
            "回归防线：断线 reset 不能变成一次性开关——新 session 收到新 zone_info 后正常 replace() 写入必须继续生效"
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
