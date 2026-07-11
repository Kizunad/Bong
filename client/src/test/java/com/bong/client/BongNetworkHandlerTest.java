package com.bong.client;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.state.NarrationState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
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
        BongHudStateStore.clear();
        DroppedItemStore.resetForTests();
        BongToast.resetForTests();
        IdentityPanelStateStore.resetForTest();
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

    @Test
    void disconnectClearsBongToastToPreventCrossSessionLeak() {
        BongToast.show(NarrationState.create("broadcast", null, "雷劫将至", "system_warning"), 1_000L);

        assertFalse(
            BongToast.current(1_001L).isEmpty(),
            "测试前必须模拟断线前一个尚未过期的活跃 toast，否则无法锁住跨 session 泄漏回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertTrue(
            BongToast.current(1_002L).isEmpty(),
            "断线必须立即清空 BongToast.activeToast；否则 reconnect 到新 server 后的首批 HUD 帧" +
                "会继续渲染上一 server 未过期的 warning/era/event toast，误导玩家判断当前局势"
        );
    }

    @Test
    void disconnectClearingBongToastDoesNotBlockNewToastAfterReconnect() {
        BongToast.show(NarrationState.create("broadcast", null, "旧服警示", "system_warning"), 0L);

        BongNetworkHandler.clearClientStateOnDisconnect();
        BongToast.show(NarrationState.create("broadcast", null, "新服提示", "era_decree"), 100L);

        BongToast toast = BongToast.current(101L);
        assertFalse(toast.isEmpty(), "断线清场后 reconnect 收到的新 toast 必须能正常显示，不能被清场逻辑永久锁死");
        assertEquals("时代法旨：新服提示", toast.text().getString());
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

    /**
     * plan-bughunt-client-identity-panel-stale-session-v1 — IdentityPanelStateStore 此前只有
     * resetForTest()（测试专用），生产态清理清单里完全没有它。断线重连后 HUD 角标
     * （{@code IdentityHudCornerLabel}）会短暂展示上一 session 的 active identity，且若玩家在
     * 这段窗口打开 {@code IdentityPanelScreen}，面板会用旧快照 init 出按钮（回调固化旧
     * identityId），后续新 session 的 fresh payload 只能改文字改不了按钮，形成 split-brain UI。
     * 本测试锁住"断线清理路径必须清空 IdentityPanelStateStore"。
     */
    @Test
    void disconnectClearsIdentityPanelStateStoreToPreventStaleSessionIdentityLeak() {
        IdentityPanelStateStore.replace(new IdentityPanelState(
            3, 400L, 0L,
            List.of(new IdentityPanelEntry(3, "断线前身份", 20, false, List.of()))));
        assertFalse(
            IdentityPanelStateStore.snapshot().identities().isEmpty(),
            "测试前必须模拟断线前残留的非空身份快照，否则无法锁住跨 session 身份泄漏回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertEquals(
            IdentityPanelState.empty(),
            IdentityPanelStateStore.snapshot(),
            "断线必须把 IdentityPanelStateStore 整体复位为 empty()，否则新 session 首个 "
                + "identity_panel_state 到达前，HUD 角标和刚打开的身份面板会继续展示上一局身份数据"
        );
    }

    @Test
    void disconnectClearingIdentityPanelStateStoreDoesNotBlockNewSessionSnapshotAfterReconnect() {
        IdentityPanelStateStore.replace(new IdentityPanelState(
            3, 400L, 0L,
            List.of(new IdentityPanelEntry(3, "断线前身份", 20, false, List.of()))));

        BongNetworkHandler.clearClientStateOnDisconnect();
        assertEquals(
            IdentityPanelState.empty(),
            IdentityPanelStateStore.snapshot(),
            "测试前置：断线后 store 应已复位为空"
        );

        IdentityPanelState newSessionState = new IdentityPanelState(
            7, 900L, 0L, List.of(new IdentityPanelEntry(7, "新局身份", 0, false, List.of())));
        IdentityPanelStateStore.replace(newSessionState);

        assertEquals(
            newSessionState,
            IdentityPanelStateStore.snapshot(),
            "回归防线：断线清理不能变成一次性开关——新 session 收到新 identity_panel_state 后"
                + "正常 replace() 写入必须继续生效"
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
