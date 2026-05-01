package com.bong.client.ui;

import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CultivationScreenTest {
    @AfterEach
    void resetStateStore() {
        PlayerStateStore.resetForTests();
    }

    @Test
    void placeholderModeShowsUnsyncedContentWithoutFabricatedData() {
        CultivationScreen.RenderContent content = CultivationScreen.describe(PlayerStateViewModel.empty());

        assertTrue(content.placeholder());
        assertEquals(List.of(
            "当前尚未同步修仙数据",
            "请等待 server 下发 player_state。"
        ), content.lines());
    }

    @Test
    void syncedModeFormatsStructuredContentInStableOrder() {
        CultivationScreen.RenderContent content = CultivationScreen.describe(PlayerStateViewModel.create(
            "Induce",
            "offline:Azure",
            78.0,
            100.0,
            0.20,
            0.35,
            PlayerStateViewModel.PowerBreakdown.create(0.20, 0.40, 0.65, 0.10),
            PlayerStateViewModel.SocialSnapshot.create(7, 12, List.of("背盟者", "三叛之人"), "defend", 0, 10, 1),
            "green_cloud_peak",
            "青云峰",
            0.78
        ));

        assertFalse(content.placeholder());
        assertEquals("境界: Induce", content.lines().get(0));
        assertEquals("真元: ████████░░ 78/100", content.lines().get(1));
        assertEquals("因果 (karma): +0.20", content.lines().get(2));
        assertEquals("善恶刻度: [═════●══════] 善 ←→ 恶", content.lines().get(3));
        assertEquals("综合实力: 0.35", content.lines().get(4));
        assertEquals(List.of(
            "战斗: 0.20",
            "财富: 0.40",
            "社交: 0.65",
            "领地: 0.10"
        ), content.lines().subList(5, 9));
        assertEquals("声名: fame +7 / notoriety +12", content.lines().get(9));
        assertEquals("声名标签: 背盟者 / 三叛之人", content.lines().get(10));
        assertEquals("派系挂靠: defend rank 0 / loyalty +10 / betrayals 1", content.lines().get(11));
        assertEquals("当前区域: 青云峰", content.lines().get(12));
        assertEquals("灵气浓度: ████████░░ 78%", content.lines().get(13));
    }

    @Test
    void bootstrapCreatesRealScreenFromCurrentSnapshot() {
        CultivationScreen unsyncedScreen = CultivationScreenBootstrap.createScreenForCurrentState();
        assertTrue(unsyncedScreen.playerState().isEmpty());

        PlayerStateStore.replace(PlayerStateViewModel.create(
            "Condense",
            "offline:Azure",
            60.0,
            100.0,
            -0.25,
            0.55,
            PlayerStateViewModel.PowerBreakdown.create(0.70, 0.15, 0.25, 0.05),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "azure_peak",
            "苍岚峰",
            0.66
        ));

        CultivationScreen syncedScreen = CultivationScreenBootstrap.createScreenForCurrentState();
        assertEquals("Condense", syncedScreen.playerState().realm());
        assertFalse(syncedScreen.playerState().isEmpty());
    }

    @Test
    void disconnectResetClearsStaleSnapshotBeforeNextScreenBuild() {
        PlayerStateStore.replace(PlayerStateViewModel.create(
            "Spirit",
            "offline:Azure",
            88.0,
            100.0,
            0.45,
            0.82,
            PlayerStateViewModel.PowerBreakdown.create(0.90, 0.30, 0.50, 0.20),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "violet_valley",
            "紫霞谷",
            0.91
        ));

        CultivationScreen staleScreen = CultivationScreenBootstrap.createScreenForCurrentState();
        assertFalse(staleScreen.playerState().isEmpty());

        CultivationScreenBootstrap.clearPlayerStateSnapshot();

        CultivationScreen clearedScreen = CultivationScreenBootstrap.createScreenForCurrentState();
        assertTrue(clearedScreen.playerState().isEmpty());
        assertTrue(CultivationScreen.describe(clearedScreen.playerState()).placeholder());
    }

    @Test
    void keypressGateOnlyOpensWhenDifferentScreenIsActive() {
        assertTrue(CultivationScreenBootstrap.shouldOpenCultivationScreen(null));
        assertFalse(CultivationScreenBootstrap.shouldOpenCultivationScreen(
            new CultivationScreen(PlayerStateViewModel.empty())
        ));
    }
}
