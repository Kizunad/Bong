package com.bong.client;

import bong.Common;
import bong.Envelope;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.lifecycle.ClientStoreScopeManifest;
import com.bong.client.lifecycle.JavaLifecycleSourceInspector;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.SeasonState;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongNetworkHandlerTest {
    private static final Set<String> DISCONNECT_ENTRY_ALLOWED_INVOCATIONS = Set.of(
        "SessionScopedStoreRegistry.clearAllOnDisconnect",
        "runAdjunctDisconnectTeardown"
    );
    private static final Set<String> DISCONNECT_ADJUNCT_ALLOWED_INVOCATIONS = Set.of(
        "runDisconnectCleanups",
        "EnvironmentEffectController.clearOnDisconnect",
        "BongShaderState.clearOnDisconnect",
        "CastFovController.clearOnDisconnect",
        "CombatJuiceSystem.clearOnDisconnect",
        "CombatHudBootstrap.clearOnDisconnect",
        "MovementKeybindings.clearOnDisconnect",
        "BotanyHudBootstrap.clearOnDisconnect",
        "TechniquesListPanel.clearOnDisconnect",
        "WeaponTreasurePanel.clearOnDisconnect",
        "HomeSequence.clearOnDisconnect",
        "InventoryMoveRejectedHandler.clearOnDisconnect",
        "PillBuffHudPlanner.clearOnDisconnect",
        "MorphCastVignetteState.clearOnDisconnect",
        "SeasonVisualController.clearOnDisconnect",
        "ScreenTransitionController.clearOnDisconnect",
        "WorldVfxDemoBootstrap.clearOnDisconnect",
        "DeadDropBreakPlayer.clearOnDisconnect",
        "NpcFootstepAudioController.clearOnDisconnect",
        "BongAnimationRegistry.clearOnDisconnect",
        "NpcDialogueBubbleRenderer.clear",
        "com.bong.client.audio.MusicStateMachine.clearOnDisconnect",
        "SoundRecipePlayer.instance",
        "SoundRecipePlayer.instance().clearOnDisconnect",
        "BongAnimationPlayer.clearOnDisconnect",
        "AnimationLayerManager.clearOnDisconnect",
        "LowerBodyGaitController.clearOnDisconnect",
        "BongPunchCombo.clearOnDisconnect",
        "MutationVisualState.reset",
        "SpiderDisguiseHandler.clearOnDisconnect",
        "RatQiTierHandler.clearOnDisconnect",
        "DaoZhanDisguiseHandler.clearOnDisconnect",
        "com.bong.client.era.EraAmbianceState.reset",
        "BongToast.clearOnDisconnect"
    );

    @AfterEach
    void resetUnknownTypeLogCache() {
        BongNetworkHandler.resetUnknownTypeLogTimesForTests();
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        BongHudStateStore.clear();
        DroppedItemStore.resetForTests();
        BongToast.resetForTests();
        IdentityPanelStateStore.resetForTest();
        FalseSkinHudStateStore.resetForTests();
        DuguV2HudStateStore.resetForTests();
        SearchHudStateStore.resetForTests();
        PlayerStateStore.resetForTests();
        SeasonStateStore.resetForTests();
    }

    @Test
    void seasonStoreClearOnDisconnect_restoresSummerBaseline() {
        SeasonStateStore.replace(new SeasonState(SeasonState.Phase.WINTER, 42L, 1_000L, 2L));

        SeasonStateStore.clearOnDisconnect();

        assertEquals(SeasonState.summerAt(0L), SeasonStateStore.snapshot(),
            "断线必须移除旧会话的 season payload，恢复无服务端状态的夏季基线");
    }

    @Test
    void realPlayerStateProtoDispatchUpdatesStoresThroughPrivateProductionApplyDispatch() {
        SeasonStateStore.replace(new SeasonState(SeasonState.Phase.SUMMER, 7L, 1000L, 0L));
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setPlayerState(seasonAuditPlayerState()
                .setSeasonState(Envelope.SeasonState.newBuilder()
                    .setSeason(Envelope.Season.SEASON_WINTER)
                    .setTickIntoPhase(42)
                    .setPhaseTotalTicks(1000)
                    .setYearIndex(2)))
            .build();

        ServerDataRouter.RouteResult route = routeRealPlayerState(envelope, "有效 WINTER");

        invokePrivateProductionApplyDispatch(route.dispatch(), "有效 WINTER");

        assertCurrentClientPlayerStateProjection(
            PlayerStateStore.snapshot(),
            "有效 WINTER 最终 PlayerStateStore"
        );
        SeasonState stored = SeasonStateStore.snapshot();
        assertEquals(SeasonState.Phase.WINTER, stored.phase(),
            "private applyDispatch 必须把 router 产出的 WINTER 写进 store；"
                + "否则 HUD/atmosphere/particle 仍会读取旧 SUMMER");
        assertEquals(42L, stored.tickIntoPhase(), "有效 season 的 tick_into_phase 必须完整落库");
        assertEquals(1000L, stored.phaseTotalTicks(), "有效 season 的 phase_total_ticks 必须完整落库");
        assertEquals(2L, stored.yearIndex(), "有效 season 的 year_index 必须完整落库");
    }

    @Test
    void missingSeasonStatePreservesEveryExistingSeasonStoreField() {
        SeasonState sentinel = new SeasonState(SeasonState.Phase.WINTER, 31L, 777L, 5L);
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setPlayerState(seasonAuditPlayerState())
            .build();

        assertInvalidSeasonPreservesStore(envelope, sentinel, "missing season_state");
    }

    @Test
    void unspecifiedSeasonPreservesEveryExistingSeasonStoreField() {
        SeasonState sentinel = new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 42L, 888L, 6L);
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setPlayerState(seasonAuditPlayerState()
                .setSeasonState(Envelope.SeasonState.newBuilder()
                    .setSeasonValue(0)
                    .setTickIntoPhase(10)
                    .setPhaseTotalTicks(1000)
                    .setYearIndex(3)))
            .build();

        assertInvalidSeasonPreservesStore(envelope, sentinel, "SEASON_UNSPECIFIED numeric 0");
    }

    @Test
    void unknownNumericSeasonPreservesEveryExistingSeasonStoreField() {
        SeasonState sentinel = new SeasonState(SeasonState.Phase.WINTER_TO_SUMMER, 53L, 999L, 7L);
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setPlayerState(seasonAuditPlayerState()
                .setSeasonState(Envelope.SeasonState.newBuilder()
                    .setSeasonValue(99)
                    .setTickIntoPhase(10)
                    .setPhaseTotalTicks(1000)
                    .setYearIndex(3)))
            .build();

        assertInvalidSeasonPreservesStore(envelope, sentinel, "unknown season numeric 99");
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


    @Test
    void disconnectClearsSearchHudStateToPreventCrossSessionLeak() {
        SearchHudStateStore.markStarted("旧服石匣", 100);
        assertEquals(
            SearchHudState.Phase.SEARCHING,
            SearchHudStateStore.snapshot().phase(),
            "测试前必须模拟旧 session 仍在搜刮，否则无法锁住 reconnect 残留回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertEquals(
            SearchHudState.Phase.IDLE,
            SearchHudStateStore.snapshot().phase(),
            "统一断线清理必须调用 SearchHudStateStore.clearOnDisconnect()；否则新 session 首帧会继续显示旧搜刮 HUD"
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
     * resetForTest()（测试专用），生产态清理清单里完全没有它。断线重连后若玩家在
     * 新快照到达之前打开 {@code IdentityPanelScreen}，面板会用旧快照 init 出按钮（回调固化旧
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

    /**
     * plan-bughunt-client-false-skin-cross-session-v1 — FalseSkinHudStateStore 此前只有
     * resetForTests()（测试专用），生产态清理清单里完全没有它。server 的 false_skin_state
     * 只在 Changed/RemovedComponents 时增量发包，断线切 session 不会有任何 removed 事件，
     * 新 session 若角色本身没有伪皮也不会有 payload 覆盖旧快照，导致 FalseSkinStackHud（伪皮
     * 层数块）和 ContamLoadHud（污染负载条）无限跨 session 残留。本测试锁住"断线清理路径
     * 必须清空 FalseSkinHudStateStore"。
     */
    @Test
    void disconnectClearsFalseSkinHudStateStoreToPreventCrossSessionResidualHud() {
        FalseSkinHudStateStore.replace(new FalseSkinHudStateStore.State(
            "player-1", "rotten_wood_armor", 2, 50f, 12.5f, 100L, List.of()));

        assertTrue(
            FalseSkinHudStateStore.snapshot().active(),
            "测试前必须模拟断线前残留的 active 伪皮快照，否则无法锁住跨 session 残留回归"
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertEquals(
            FalseSkinHudStateStore.State.NONE,
            FalseSkinHudStateStore.snapshot(),
            "断线必须把 FalseSkinHudStateStore 整体复位为 State.NONE，否则新 session 在未收到"
                + "任何 false_skin_state 前，FalseSkinStackHud/ContamLoadHud 会继续渲染上一局的"
                + "伪皮层数和污染负载"
        );
        assertFalse(
            FalseSkinHudStateStore.snapshot().active(),
            "断线后 active() 必须为 false，否则 FalseSkinStackHud/ContamLoadHud 的渲染门槛条件仍会通过"
        );
    }

    @Test
    void disconnectClearingFalseSkinHudStateStoreDoesNotBlockNewSessionSnapshotAfterReconnect() {
        FalseSkinHudStateStore.replace(new FalseSkinHudStateStore.State(
            "player-1", "rotten_wood_armor", 2, 50f, 12.5f, 100L, List.of()));

        BongNetworkHandler.clearClientStateOnDisconnect();
        assertEquals(
            FalseSkinHudStateStore.State.NONE,
            FalseSkinHudStateStore.snapshot(),
            "测试前置：断线后 store 应已复位为 State.NONE"
        );

        FalseSkinHudStateStore.State newSessionState = new FalseSkinHudStateStore.State(
            "player-2", "spirit_wood_scroll", 1, 40f, 0f, 900L, List.of());
        FalseSkinHudStateStore.replace(newSessionState);

        assertEquals(
            newSessionState,
            FalseSkinHudStateStore.snapshot(),
            "回归防线：断线清理不能变成一次性开关——新 session 收到新 false_skin_state 后"
                + "正常 replace() 写入必须继续生效"
        );
    }

    /**
     * plan-bughunt-dugu-v2-hud-disconnect-bleed-v1 — DuguV2HudStateStore 此前只有
     * resetForTests()（测试专用），生产态断线清理清单里完全没有它。server 的
     * dugu_v2_skill_cast / dugu_v2_self_cure / dugu_v2_shroud_active /
     * permanent_qi_max_decay_applied bridge 只在毒蛊 v2 事件发生时推增量/状态，没有
     * join/disconnect reset payload；revealRisk 没有 expiry 字段、selfRevealed 是 sticky
     * merge，新 session 若没再触发毒蛊 v2 事件也不会有 payload 覆盖旧快照，导致上一局的
     * "暴露 xx%" "自蕴 xx% 已露" 或遮蔽 tint 无限期跨 session 残留到下一局。本测试锁住
     * "断线清理路径必须清空 DuguV2HudStateStore"。
     */
    @Test
    void disconnectClearsDuguV2HudStateStoreToPreventCrossSessionResidualHud() {
        DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
            true, 0.8f, "剧毒攻心", 0.65f, 72.5f, true, true, 999_000L, 5.5f, 40f, 999_500L));

        assertTrue(
            DuguV2HudStateStore.snapshot().selfRevealed(),
            "测试前必须模拟断线前残留的 sticky selfRevealed=true 快照，否则无法锁住跨 session 残留回归；"
                + "实际 selfRevealed=" + DuguV2HudStateStore.snapshot().selfRevealed()
        );
        assertTrue(
            DuguV2HudStateStore.snapshot().revealRisk() > 0f,
            "测试前必须模拟断线前残留的 revealRisk > 0 快照（该字段无 expiry，是跨 session 残留的核心症状）；"
                + "实际 revealRisk=" + DuguV2HudStateStore.snapshot().revealRisk()
        );

        BongNetworkHandler.clearClientStateOnDisconnect();

        assertEquals(
            DuguV2HudStateStore.State.NONE,
            DuguV2HudStateStore.snapshot(),
            "断线必须把 DuguV2HudStateStore 整体复位为 State.NONE，否则新 session 在未收到任何"
                + "毒蛊 v2 payload 前，DuguV2HudPlanner 会继续渲染上一局的暴露/自蕴/遮蔽 HUD"
        );
        assertFalse(
            DuguV2HudStateStore.snapshot().selfRevealed(),
            "断线后 sticky selfRevealed 必须回 false，否则下一局没中毒的角色也会显示已自曝；"
                + "实际 selfRevealed=" + DuguV2HudStateStore.snapshot().selfRevealed()
        );
        assertEquals(
            0f,
            DuguV2HudStateStore.snapshot().revealRisk(),
            "断线后 revealRisk 必须归零，否则 DuguV2HudPlanner 的 revealRisk > 0 渲染门槛条件仍会通过"
        );
    }

    @Test
    void disconnectClearingDuguV2HudStateStoreDoesNotBlockNewSessionSnapshotAfterReconnect() {
        DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
            true, 0.8f, "剧毒攻心", 0.65f, 72.5f, true, true, 999_000L, 5.5f, 40f, 999_500L));

        BongNetworkHandler.clearClientStateOnDisconnect();
        assertEquals(
            DuguV2HudStateStore.State.NONE,
            DuguV2HudStateStore.snapshot(),
            "测试前置：断线后 store 应已复位为 State.NONE"
        );

        DuguV2HudStateStore.State newSessionState = new DuguV2HudStateStore.State(
            true, 0.4f, "新局中毒提示", 0.2f, 15f, false, false, 0L, 0f, 0f, 0L);
        DuguV2HudStateStore.replace(newSessionState);

        assertEquals(
            newSessionState,
            DuguV2HudStateStore.snapshot(),
            "回归防线：断线清理不能变成一次性开关——新 session 收到新 dugu_v2_* payload 后"
                + "正常 replace() 写入必须继续生效"
        );
    }

    @Test
    void disconnectAdjunctRuntimeFailureDoesNotSkipLaterCleanup() {
        List<String> calls = new java.util.ArrayList<>();

        BongNetworkHandler.runDisconnectCleanups(
            () -> calls.add("before"),
            () -> {
                calls.add("failing");
                throw new IllegalStateException("recoverable cleanup failure");
            },
            () -> calls.add("after")
        );

        assertEquals(
            List.of("before", "failing", "after"),
            calls,
            "单个 adjunct RuntimeException 必须被隔离，后续 animation/audio/HUD 清理仍要执行"
        );
    }

    @Test
    void disconnectAdjunctErrorStillPropagatesWithoutRunningLaterCleanup() {
        List<String> calls = new java.util.ArrayList<>();

        AssertionError error = assertThrows(
            AssertionError.class,
            () -> BongNetworkHandler.runDisconnectCleanups(
                () -> calls.add("before"),
                () -> {
                    calls.add("fatal");
                    throw new AssertionError("fatal cleanup");
                },
                () -> calls.add("after")
            )
        );

        assertEquals("fatal cleanup", error.getMessage(), "Error 必须原样透传");
        assertEquals(List.of("before", "fatal"), calls, "Error 不得伪装成可恢复 RuntimeException");
    }

    // ──────────────────────────────────────────────────────────────────────
    // plan-bughunt-dugu-v2-hud-disconnect-bleed-v1 — 断线注册接线断言。
    // 上面的 helper 级用例只驱动 clearClientStateOnDisconnect() 本体；若有人把
    // ClientPlayConnectionEvents.DISCONNECT 注册块删掉、或把 helper 里的
    // DuguV2HudStateStore.clearOnDisconnect() 调用移走，helper 级测试依然全绿，
    // 生产态断线清理却已断链。register() 挂的 Fabric DISCONNECT 回调需要活的
    // Minecraft client 实例，单测无法直接触发；镜像
    // TiandaoPresencePayloadHandlerTest 的 source-scan 模式锁住这条接线。
    // ──────────────────────────────────────────────────────────────────────

    @Test
    void bongNetworkHandlerRegistersDisconnectWiringThroughTheSessionStoreRegistry() throws Exception {
        java.nio.file.Path testClasses = java.nio.file.Path.of("").toAbsolutePath().normalize();
        java.nio.file.Path clientRoot;
        if (java.nio.file.Files.isDirectory(testClasses.resolve("src"))) {
            clientRoot = testClasses;
        } else if (java.nio.file.Files.isDirectory(testClasses.resolve("client").resolve("src"))) {
            clientRoot = testClasses.resolve("client");
        } else {
            clientRoot = testClasses;
        }
        java.nio.file.Path handlerSrc = clientRoot.resolve(
            "src/main/java/com/bong/client/BongNetworkHandler.java"
        );
        assertTrue(
            java.nio.file.Files.exists(handlerSrc),
            "BongNetworkHandler.java 必须存在于 " + handlerSrc.toAbsolutePath()
                + "，否则无法核验统一 Store 断线清理接线；实际 exists=false"
        );
        String src = java.nio.file.Files.readString(handlerSrc);

        int disconnectBlockStart = src.indexOf("ClientPlayConnectionEvents.DISCONNECT.register(");
        assertTrue(
            disconnectBlockStart >= 0,
            "期望 BongNetworkHandler 中存在 ClientPlayConnectionEvents.DISCONNECT.register(...) 注册块"
                + "（断线清理的生产态入口），实际：源码中未找到"
        );
        int disconnectBlockEnd = src.indexOf("ClientPlayConnectionEvents.JOIN.register(", disconnectBlockStart);
        assertTrue(
            disconnectBlockEnd > disconnectBlockStart,
            "期望 DISCONNECT.register(...) 之后存在 JOIN.register(...) 块用于圈定断线注册块范围，"
                + "实际 disconnectBlockEnd=" + disconnectBlockEnd
        );
        String disconnectBlock = src.substring(disconnectBlockStart, disconnectBlockEnd);

        assertTrue(
            disconnectBlock.contains("clearClientStateOnDisconnect"),
            "期望 DISCONNECT 注册块路由到 BongNetworkHandler.clearClientStateOnDisconnect()；"
                + "否则 registry 和非 Store hook 都不会在真实断线时执行"
        );

        String clearHelper = methodSource(
            src,
            "static void clearClientStateOnDisconnect()"
        );
        String adjunctHelper = methodSource(
            src,
            "private static void runAdjunctDisconnectTeardown()"
        );

        String registryCall = "SessionScopedStoreRegistry.clearAllOnDisconnect()";
        assertEquals(
            clearHelper.indexOf(registryCall),
            clearHelper.lastIndexOf(registryCall),
            "统一 helper 必须恰好调用一次 session Store registry，避免漏清或重复清理"
        );
        assertTrue(
            clearHelper.contains(registryCall),
            "统一 helper 必须调用 SessionScopedStoreRegistry.clearAllOnDisconnect()"
        );

        List<String> nonStoreHooks = List.of(
            "() -> NpcDialogueBubbleRenderer.clear()",
            "() -> com.bong.client.audio.MusicStateMachine.clearOnDisconnect()",
            "() -> SoundRecipePlayer.instance().clearOnDisconnect()",
            "() -> BongAnimationPlayer.clearOnDisconnect()",
            "() -> AnimationLayerManager.clearOnDisconnect()",
            "() -> BongPunchCombo.clearOnDisconnect()",
            "() -> MutationVisualState.reset()",
            "() -> SpiderDisguiseHandler.clearOnDisconnect()",
            "() -> RatQiTierHandler.clearOnDisconnect()",
            "() -> DaoZhanDisguiseHandler.clearOnDisconnect()",
            "() -> com.bong.client.era.EraAmbianceState.reset()",
            "() -> BongToast.clearOnDisconnect()"
        );
        int previousIndex = -1;
        for (String hook : nonStoreHooks) {
            int hookIndex = adjunctHelper.indexOf(hook);
            assertTrue(
                hookIndex > previousIndex,
                "非 Store hook 必须保留且维持既有相对顺序；未按序找到 " + hook
            );
            assertEquals(
                hookIndex,
                adjunctHelper.lastIndexOf(hook),
                "非 Store hook 必须恰好调用一次，避免重复副作用：" + hook
            );
            previousIndex = hookIndex;
        }

        StoreTokenLexicon lexicon = registryManagedStoreTokens(clientRoot.resolve("src/main/java"));
        JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
            src,
            "BongNetworkHandler",
            "clearClientStateOnDisconnect",
            lexicon.fqcns(),
            DISCONNECT_ENTRY_ALLOWED_INVOCATIONS,
            Set.of()
        );
        JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
            src,
            "BongNetworkHandler",
            "runAdjunctDisconnectTeardown",
            lexicon.fqcns(),
            DISCONNECT_ADJUNCT_ALLOWED_INVOCATIONS,
            Set.of()
        );
    }

    private record StoreTokenLexicon(Set<String> fqcns) {}

    private static StoreTokenLexicon registryManagedStoreTokens(
        java.nio.file.Path productionSourceRoot
    ) throws Exception {
        Set<String> fqcns = new TreeSet<>(ClientStoreScopeManifest.registryManagedSessionStores());
        for (String fqcn : fqcns) {
            java.nio.file.Path source = productionSourceRoot.resolve(fqcn.replace('.', '/') + ".java");
            assertTrue(java.nio.file.Files.exists(source), "allowlist 门禁必须能读取 registry-managed Store：" + fqcn);
        }
        return new StoreTokenLexicon(Set.copyOf(fqcns));
    }

    private static StoreTokenLexicon fixtureStoreTokens() {
        return new StoreTokenLexicon(Set.of("com.bong.client.hud.LootContainerStateStore"));
    }

    @Test
    void storeReferenceGuardRejectsEveryDirectStoreReferenceShape() {
        StoreTokenLexicon lexicon = fixtureStoreTokens();
        List<String> forbiddenFixtures = List.of(
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        LootContainerStateStore.clearOnDisconnect();
                    }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        com.bong.client.hud.LootContainerStateStore.clearOnDisconnect();
                    }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        Runnable cleaner = LootContainerStateStore::clearOnDisconnect;
                    }
                }
                """,
            """
                import com.bong.client.hud.LootContainerStateStore;
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        Class<?> storeType = LootContainerStateStore.class;
                    }
                }
                """,
            """
                import static com.bong.client.hud.LootContainerStateStore.clearOnDisconnect;
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() { clearOnDisconnect(); }
                }
                """,
            """
                import static com.bong.client.hud.LootContainerStateStore.*;
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() { clearOnDisconnect(); }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() { Helper.clearOnDisconnect(); }
                }
                final class Helper {
                    static void clearOnDisconnect() { }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() { Helper.clear(); }
                }
                final class Helper {
                    static void clear() { LootContainerStateStore.clearOnDisconnect(); }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        Runnable cleaner = Helper::clearOnDisconnect;
                    }
                }
                final class Helper {
                    static void clearOnDisconnect() { LootContainerStateStore.clearOnDisconnect(); }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() { new Helper(); }
                }
                final class Helper {
                    Helper() { LootContainerStateStore.clearOnDisconnect(); }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        Runnable cleaner = Helper::clear;
                    }
                }
                final class Helper {
                    static void clear() { LootContainerStateStore.clearOnDisconnect(); }
                }
                """
        );

        for (String fixture : forbiddenFixtures) {
            assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
                    fixture,
                    "BongNetworkHandler",
                    "clearClientStateOnDisconnect",
                    lexicon.fqcns(),
                    Set.of(),
                    Set.of()
                )
            );
        }
    }

    @Test
    void adjunctStoreReferenceGuardRejectsStoreMethodReference() {
        String fixture = """
            final class BongNetworkHandler {
                private static void runAdjunctDisconnectTeardown() {
                    Runnable cleaner = LootContainerStateStore::clearOnDisconnect;
                }
            }
            """;
        StoreTokenLexicon lexicon = fixtureStoreTokens();

        assertThrows(
            AssertionError.class,
            () -> JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
                fixture,
                "BongNetworkHandler",
                "runAdjunctDisconnectTeardown",
                lexicon.fqcns(),
                Set.of(),
                Set.of()
            )
        );
    }

    @Test
    void storeReferenceGuardAllowsSanctionedRegistryAndNonStoreTeardown() {
        String fixture = """
            final class BongNetworkHandler {
                static void clearClientStateOnDisconnect() {
                    SessionScopedStoreRegistry.clearAllOnDisconnect();
                    runAdjunctDisconnectTeardown();
                }
            }
            """;
        StoreTokenLexicon lexicon = fixtureStoreTokens();

        JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
            fixture,
            "BongNetworkHandler",
            "clearClientStateOnDisconnect",
            lexicon.fqcns(),
            DISCONNECT_ENTRY_ALLOWED_INVOCATIONS,
            Set.of()
        );
    }

    @Test
    void disconnectAllowlistRejectsRenamedHelperCallAndMethodReference() {
        StoreTokenLexicon lexicon = fixtureStoreTokens();
        for (String fixture : List.of(
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() { Helper.teardown(); }
                }
                final class Helper {
                    static void teardown() { LootContainerStateStore.clearOnDisconnect(); }
                }
                """,
            """
                final class BongNetworkHandler {
                    static void clearClientStateOnDisconnect() {
                        Runnable teardown = Helper::teardown;
                    }
                }
                final class Helper {
                    static void teardown() { LootContainerStateStore.clearOnDisconnect(); }
                }
                """
        )) {
            assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
                    fixture,
                    "BongNetworkHandler",
                    "clearClientStateOnDisconnect",
                    lexicon.fqcns(),
                    DISCONNECT_ENTRY_ALLOWED_INVOCATIONS,
                    Set.of()
                )
            );
        }
    }

    @Test
    void storeImportUsedOnlyOutsideAuditedDisconnectMethodIsAllowed() {
        String fixture = """
            import com.bong.client.hud.LootContainerStateStore;
            final class BongNetworkHandler {
                static void clearClientStateOnDisconnect() {
                    SessionScopedStoreRegistry.clearAllOnDisconnect();
                    runAdjunctDisconnectTeardown();
                }
                private static void runAdjunctDisconnectTeardown() { }
                static void applyBusinessPayload() {
                    LootContainerStateStore.replace(null);
                }
            }
            """;
        StoreTokenLexicon lexicon = fixtureStoreTokens();

        JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
            fixture,
            "BongNetworkHandler",
            "clearClientStateOnDisconnect",
            lexicon.fqcns(),
            DISCONNECT_ENTRY_ALLOWED_INVOCATIONS,
            Set.of()
        );
        JavaLifecycleSourceInspector.assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
            fixture,
            "BongNetworkHandler",
            "runAdjunctDisconnectTeardown",
            lexicon.fqcns(),
            Set.of(),
            Set.of()
        );
    }

    private static Envelope.PlayerState.Builder seasonAuditPlayerState() {
        return Envelope.PlayerState.newBuilder()
            .setPlayer("offline:SeasonAudit")
            .setRealm(Common.Realm.REALM_CONDENSE)
            .setSpiritQi(50.0)
            .setSpiritQiMax(100.0)
            .setKarma(-0.37)
            .setCompositePower(0.35)
            .setZone("zone-1")
            .setBreakdown(Envelope.PlayerPowerBreakdown.newBuilder()
                .setCombat(0.21)
                .setWealth(0.42)
                .setSocial(0.63)
                .setKarma(0.74)
                .setTerritory(0.84));
    }

    private static ServerDataRouter.RouteResult routeRealPlayerState(
        Envelope.ServerDataEnvelope envelope,
        String scenario
    ) {
        ProtoServerDataBridge.BridgeResult bridge = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(
            bridge.isSuccess(),
            scenario + " 的真实 player_state proto bytes 应 bridge 成功：" + bridge.errorMessage()
        );
        JsonObject bridgeJson = JsonParser.parseString(bridge.legacyJson()).getAsJsonObject();
        assertEquals(
            -0.37,
            bridgeJson.get("karma").getAsDouble(),
            0.0001,
            scenario + " bridge 必须保留带符号的顶层 karma"
        );
        assertEquals(
            0.74,
            bridgeJson.getAsJsonObject("breakdown").get("karma").getAsDouble(),
            0.0001,
            scenario + " bridge 必须独立保留 wire breakdown.karma，不能与顶层 karma 混接"
        );

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault().route(
            bridge.legacyJson(),
            bridge.legacyJson().getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(
            route.isHandled(),
            scenario + " 不应吞掉其余合法 player_state 字段：" + route.logMessage()
        );
        PlayerStateViewModel playerState = route.dispatch()
            .playerStateViewModel()
            .orElseThrow(() -> new AssertionError(
                scenario + " 必须保留合法 player_state dispatch，不能因 season 无效而整包丢弃"
            ));
        assertCurrentClientPlayerStateProjection(
            playerState,
            scenario + " dispatch"
        );
        return route;
    }

    private static void assertInvalidSeasonPreservesStore(
        Envelope.ServerDataEnvelope envelope,
        SeasonState sentinel,
        String scenario
    ) {
        SeasonStateStore.replace(sentinel);
        ServerDataRouter.RouteResult route = routeRealPlayerState(envelope, scenario);
        assertTrue(
            route.dispatch().seasonState().isEmpty(),
            scenario + " 必须只让 season 分支安全 no-op，不能产生默认季节"
        );

        invokePrivateProductionApplyDispatch(route.dispatch(), scenario);

        assertCurrentClientPlayerStateProjection(
            PlayerStateStore.snapshot(),
            scenario + " 最终 PlayerStateStore"
        );
        SeasonState stored = SeasonStateStore.snapshot();
        assertEquals(sentinel.phase(), stored.phase(), scenario + " 不得覆盖既有 phase");
        assertEquals(
            sentinel.tickIntoPhase(),
            stored.tickIntoPhase(),
            scenario + " 不得覆盖既有 tickIntoPhase"
        );
        assertEquals(
            sentinel.phaseTotalTicks(),
            stored.phaseTotalTicks(),
            scenario + " 不得覆盖既有 phaseTotalTicks"
        );
        assertEquals(sentinel.yearIndex(), stored.yearIndex(), scenario + " 不得覆盖既有 yearIndex");
    }

    private static void assertCurrentClientPlayerStateProjection(
        PlayerStateViewModel playerState,
        String scenario
    ) {
        assertEquals("offline:SeasonAudit", playerState.playerId(), scenario + " 必须保留 player id");
        assertEquals("Condense", playerState.realm(), scenario + " 必须保留 realm");
        assertEquals(50.0, playerState.spiritQiCurrent(), 0.0001, scenario + " 必须保留 spirit qi");
        assertEquals(100.0, playerState.spiritQiMax(), 0.0001, scenario + " 必须保留 spirit qi max");
        assertEquals(-0.37, playerState.karma(), 0.0001, scenario + " 必须保留带符号的顶层 karma");
        assertEquals(0.35, playerState.compositePower(), 0.0001, scenario + " 必须保留 composite power");
        assertEquals("zone-1", playerState.zoneId(), scenario + " 必须保留 zone");
        assertEquals(0.21, playerState.breakdown().combat(), 0.0001, scenario + " 必须保留 combat breakdown");
        assertEquals(0.42, playerState.breakdown().wealth(), 0.0001, scenario + " 必须保留 wealth breakdown");
        assertEquals(0.63, playerState.breakdown().social(), 0.0001, scenario + " 必须保留 social breakdown");
        assertEquals(0.84, playerState.breakdown().territory(), 0.0001, scenario + " 必须保留 territory breakdown");
    }

    private static void invokePrivateProductionApplyDispatch(ServerDataDispatch dispatch, String scenario) {
        try {
            Method applyDispatch = BongNetworkHandler.class.getDeclaredMethod(
                "applyDispatch",
                MinecraftClient.class,
                ServerDataDispatch.class,
                String.class
            );
            assertTrue(
                Modifier.isPrivate(applyDispatch.getModifiers()),
                "applyDispatch 必须保持 private 生产边界，测试只通过反射进入"
            );
            applyDispatch.setAccessible(true);
            applyDispatch.invoke(null, allocateHeadlessClientWithoutPlayer(), dispatch, "player_state");
        } catch (InvocationTargetException exception) {
            throw new AssertionError(
                scenario + " 调用 private applyDispatch 时不应在 player == null 的合法 headless 边界抛错",
                exception.getCause()
            );
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError(scenario + " 无法反射调用 private applyDispatch", exception);
        }
    }

    private static String methodSource(String source, String declaration) {
        int start = source.indexOf(declaration);
        assertTrue(start >= 0, "必须存在 production lifecycle helper：" + declaration);
        int bodyStart = source.indexOf('{', start);
        assertTrue(bodyStart >= 0, "production lifecycle helper 必须有方法体：" + declaration);
        int depth = 0;
        for (int index = bodyStart; index < source.length(); index++) {
            char current = source.charAt(index);
            if (current == '{') {
                depth++;
            } else if (current == '}' && --depth == 0) {
                return source.substring(start, index + 1);
            }
        }
        throw new AssertionError("无法圈定 production lifecycle helper：" + declaration);
    }

    private static MinecraftClient allocateHeadlessClientWithoutPlayer() {
        try {
            Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
            Field singleton = unsafeClass.getDeclaredField("theUnsafe");
            singleton.setAccessible(true);
            Object unsafe = singleton.get(null);
            Method allocateInstance = unsafeClass.getMethod("allocateInstance", Class.class);
            MinecraftClient client = (MinecraftClient) allocateInstance.invoke(unsafe, MinecraftClient.class);
            assertNull(client.player, "无构造 headless client 必须没有 player，才能只执行纯状态 dispatch 分支");
            return client;
        } catch (InvocationTargetException exception) {
            throw new AssertionError("无法分配 non-null headless MinecraftClient", exception.getCause());
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError("无法分配 non-null headless MinecraftClient", exception);
        }
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
