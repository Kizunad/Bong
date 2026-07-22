package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.hud.AlchemyProgressHudPlanner;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AlchemyScreenSessionPresentationTest {
    private static final BlockPos FURNACE_POS = new BlockPos(12, 64, -8);

    private final List<String> sentPayloads = new ArrayList<>();

    @BeforeEach
    void setUp() {
        AlchemySessionStore.resetForTests();
        AlchemyFurnaceStore.resetForTests();
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8))
        );
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        AlchemySessionStore.resetForTests();
        AlchemyFurnaceStore.resetForTests();
    }

    @Test
    void openedScreenRefreshesTerminalGuidanceAndKeepsCentralHudHidden() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        screen.attachSessionListenerForTests();
        AlchemySessionPresentationPlanner.Presentation awaiting =
            screen.sessionPresentationForTests();
        assertFalse(awaiting.idle(),
            "furnace 已声明 has_session=true 时不得暂时伪装成未起炉");
        assertTrue(awaiting.statusText().contains("等待同步"),
            "session packet 尚未到达时应显示等待同步，而不是丢失炉内占用态");

        AlchemySessionStore.replace(finishedSession());

        AlchemySessionPresentationPlanner.Presentation view = screen.sessionPresentationForTests();
        assertNotNull(view, "已打开 screen 必须收到 AlchemySessionStore 网络更新");
        assertTrue(view.terminal(),
            "has_session=true + active=false + 完整 guidance 必须进入 terminal 呈现");
        assertFalse(view.active());
        assertTrue(view.statusText().contains("已完成"));
        assertTrue(view.statusText().contains("已结束"),
            "服务端 status_label 必须保留在结束态文案中");
        assertEquals("§f177 / 240t", view.progressText());
        assertEquals("§e0.73 / 0.61", view.temperatureText());
        assertEquals("§713.5 / 18.0", view.qiText());
        assertTrue(view.detailLines().contains("§a✓ §7t12 (+3) §fci_she_hao×2 + ling_shui×1"));
        assertTrue(view.detailLines().contains("§c× §7t101 (+9) §fdan_sha×3"));
        assertTrue(view.detailLines().contains("§e○ §7t190 (+4) §f（无投料）"));
        assertTrue(view.detailLines().contains("§7干预：§7AdjustTemp(0.73)"));
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L).stream()
                .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD),
            "terminal snapshot 为 inactive，会保留炉内 guidance，但中央 HUD 必须继续隐藏");

        screen.detachSessionListenerForTests();
    }

    @Test
    void openedScreenRefreshesEveryActiveSnapshotIncludingStageAndInterventionChanges() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        List<AlchemySessionPresentationPlanner.Presentation> refreshes = new ArrayList<>();
        screen.setSessionPresentationObserverForTests(refreshes::add);
        screen.attachSessionListenerForTests();
        int refreshesAfterAttach = refreshes.size();

        AlchemySessionStore.replace(activeSessionAtStart());

        assertEquals(refreshesAfterAttach + 1, refreshes.size(),
            "首次异步 active snapshot 必须让已打开 screen 恰好刷新一次");
        AlchemySessionPresentationPlanner.Presentation initial = screen.sessionPresentationForTests();
        assertTrue(initial.active());
        assertEquals("§f20 / 100t", initial.progressText());
        assertEquals("§e0.50 / 0.55", initial.temperatureText());
        assertEquals("§72.0 / 5.0", initial.qiText());
        assertEquals(
            List.of("§7干预", "§7AdjustTemp(0.50)"),
            initial.detailLines(),
            "首次 active snapshot 的 intervention guidance 必须直接进入已打开 screen"
        );
        assertEquals(List.of(0), initial.flashingStageSlots(),
            "首次 active snapshot 必须让当前窗口 stage 进入真实 slot 闪烁规划");

        AlchemySessionStore.replace(activeSessionAfterFeedAndIntervention());

        assertEquals(refreshesAfterAttach + 2, refreshes.size(),
            "后续 active stage/intervention snapshot 必须再次恰好刷新一次");
        AlchemySessionPresentationPlanner.Presentation updated = screen.sessionPresentationForTests();
        assertTrue(updated.active());
        assertEquals("§f46 / 100t", updated.progressText());
        assertEquals("§e0.57 / 0.55", updated.temperatureText());
        assertEquals("§73.5 / 5.0", updated.qiText());
        assertEquals(
            List.of("§7干预", "§7FeedStage(0)", "§7InjectQi(3.5)"),
            updated.detailLines(),
            "active-to-active 更新不得冻结旧 intervention guidance"
        );
        assertEquals(List.of(1), updated.flashingStageSlots(),
            "后续 active snapshot 必须清掉已完成 stage，并闪烁新窗口对应 slot");
        assertEquals(2, AlchemySessionStore.snapshot().stages().size());
        assertTrue(AlchemySessionStore.snapshot().stages().get(0).completed(),
            "后续 active snapshot 的 stage 完成态必须保留给 screen 的 stage-flash 消费链");
        assertEquals(40, AlchemySessionStore.snapshot().stages().get(1).atTick());
        assertEquals(8, AlchemySessionStore.snapshot().stages().get(1).window());
        assertEquals("dan_sha×3", AlchemySessionStore.snapshot().stages().get(1).summary());
        assertFalse(initial.equals(updated),
            "active-to-active 更新必须替换 presentation，不能保留首次网络快照");

        screen.detachSessionListenerForTests();
    }

    @Test
    void retrySuccessEmptyFurnaceAndSessionClearCompletedPresentation() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        screen.attachSessionListenerForTests();
        AlchemySessionStore.replace(finishedSession());
        assertTrue(screen.sessionPresentationForTests().terminal());

        AlchemyFurnaceStore.replace(furnace(false));
        AlchemySessionStore.replace(AlchemySessionStore.Snapshot.empty());

        AlchemySessionPresentationPlanner.Presentation cleared = screen.sessionPresentationForTests();
        assertTrue(cleared.idle(),
            "重试发奖成功后的 empty furnace/session 必须清除旧完成态");
        assertEquals("§8未起炉", cleared.statusText());
        assertEquals("§70 / 0t", cleared.progressText());
        assertEquals("", cleared.temperatureText());
        assertEquals("", cleared.qiText());
        assertEquals(List.of("§7干预"), cleared.detailLines(),
            "empty session 不得残留 stage 或 intervention guidance");

        screen.detachSessionListenerForTests();
    }

    @Test
    void repeatedAttachIsIdempotentAndRemovedScreenReceivesNoLaterUpdates() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        int[] refreshCount = {0};
        screen.setSessionPresentationObserverForTests(ignored -> refreshCount[0]++);
        screen.attachSessionListenerForTests();
        screen.attachSessionListenerForTests();
        screen.attachSessionListenerForTests();
        assertEquals(1, AlchemySessionStore.listenerCountForTests(),
            "重复 init/build/resize 等价 attach 必须只保留一个 session listener");
        int refreshesBeforeUpdate = refreshCount[0];

        AlchemySessionStore.replace(finishedSession());
        assertEquals(refreshesBeforeUpdate + 1, refreshCount[0],
            "单次 store update 必须恰好刷新一次，不能因重复 attach 倍增");
        AlchemySessionPresentationPlanner.Presentation retained = screen.sessionPresentationForTests();
        assertTrue(retained.terminal(),
            "幂等 listener 仍必须把 store 更新送到已打开 screen");

        screen.detachSessionListenerForTests();
        assertEquals(0, AlchemySessionStore.listenerCountForTests(),
            "removed/detach 必须从 store 解绑 listener");
        screen.attachSessionListenerForTests();
        assertEquals(0, AlchemySessionStore.listenerCountForTests(),
            "removed 后不得 reattach zombie listener");
        int refreshesAfterRemoval = refreshCount[0];
        AlchemySessionStore.replace(activeSession());

        assertEquals(refreshesAfterRemoval, refreshCount[0],
            "removed screen 后续 store update 的刷新次数必须为零");
        assertEquals(retained, screen.sessionPresentationForTests(),
            "removed 后不得收到后续 store 更新");
    }

    @Test
    void terminalSessionStillSendsRealTakeBackAction() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemySessionStore.replace(finishedSession());
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);

        assertTrue(screen.takeBackForTests(), "T take-back action 必须继续由 AlchemyScreen 消费");
        assertEquals(
            List.of("{\"type\":\"alchemy_take_back\",\"v\":1,\"furnace_pos\":[12,64,-8],\"slot_idx\":0}"),
            sentPayloads,
            "terminal session 的 T 必须走真实 ClientRequestSender alchemy_take_back seam"
        );
    }

    private static AlchemyFurnaceStore.Snapshot furnace(boolean hasSession) {
        return new AlchemyFurnaceStore.Snapshot(
            FURNACE_POS, 2, 88.0f, 100.0f, "Azure", hasSession
        );
    }

    private static AlchemySessionStore.Snapshot finishedSession() {
        return new AlchemySessionStore.Snapshot(
            "finished_contract_recipe",
            false,
            177,
            240,
            0.73f,
            0.61f,
            0.07f,
            13.5,
            18.0,
            "已结束",
            List.of(
                new AlchemySessionStore.StageHint(
                    12, 3, "ci_she_hao×2 + ling_shui×1", true, false),
                new AlchemySessionStore.StageHint(101, 9, "dan_sha×3", false, true),
                new AlchemySessionStore.StageHint(190, 4, "", false, false)
            ),
            List.of("§7AdjustTemp(0.73)", "§7InjectQi(13.5)")
        );
    }

    private static AlchemySessionStore.Snapshot activeSession() {
        return activeSessionAtStart();
    }

    private static AlchemySessionStore.Snapshot activeSessionAtStart() {
        return new AlchemySessionStore.Snapshot(
            "active_contract_recipe",
            true,
            20,
            100,
            0.50f,
            0.55f,
            0.08f,
            2.0,
            5.0,
            "炼制中",
            List.of(
                new AlchemySessionStore.StageHint(18, 5, "ci_she_hao×2", false, false),
                new AlchemySessionStore.StageHint(40, 8, "dan_sha×3", false, false)
            ),
            List.of("§7AdjustTemp(0.50)")
        );
    }

    private static AlchemySessionStore.Snapshot activeSessionAfterFeedAndIntervention() {
        return new AlchemySessionStore.Snapshot(
            "active_contract_recipe",
            true,
            46,
            100,
            0.57f,
            0.55f,
            0.08f,
            3.5,
            5.0,
            "炼制中",
            List.of(
                new AlchemySessionStore.StageHint(18, 5, "ci_she_hao×2", true, false),
                new AlchemySessionStore.StageHint(40, 8, "dan_sha×3", false, false)
            ),
            List.of("§7FeedStage(0)", "§7InjectQi(3.5)")
        );
    }
}
