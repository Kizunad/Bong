package com.bong.client.hud;

import com.bong.client.combat.store.TribulationBroadcastStore;
import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class TribulationBroadcastHudPlannerTest {
    @AfterEach void tearDown() {
        TribulationBroadcastStore.resetForTests();
        TribulationStateStore.resetForTests();
        PlayerStateStore.resetForTests();
    }

    @Test void hiddenWhenInactive() {
        assertTrue(TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L).isEmpty());
    }

    @Test void drawsStageAndActor() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 12, -34, 10_000L, false, 120
        ));
        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(
            800, 600, 1_000L, new TribulationBroadcastHudPlanner.ViewerPosition(-188, -34)
        );
        assertFalse(cmds.isEmpty());
        boolean hasWarn = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("甲"));
        assertTrue(hasWarn);
        boolean hasPositionAndDistance = cmds.stream().anyMatch(c -> c.isText()
            && c.text().contains("坐标 (12, -34)")
            && c.text().contains("方位 东")
            && c.text().contains("距离 120 格"));
        assertTrue(hasPositionAndDistance);
    }

    @Test void directionLabelsUseEightWayCompass() {
        TribulationBroadcastHudPlanner.ViewerPosition origin = new TribulationBroadcastHudPlanner.ViewerPosition(0, 0);

        assertEquals("东", TribulationBroadcastHudPlanner.directionLabel(origin, 10, 0));
        assertEquals("东南", TribulationBroadcastHudPlanner.directionLabel(origin, 10, 10));
        assertEquals("南", TribulationBroadcastHudPlanner.directionLabel(origin, 0, 10));
        assertEquals("西北", TribulationBroadcastHudPlanner.directionLabel(origin, -10, -10));
        assertEquals("脚下", TribulationBroadcastHudPlanner.directionLabel(origin, 0, 0));
        assertEquals("", TribulationBroadcastHudPlanner.directionLabel(null, 10, 0));
    }

    @Test void drawsLockedStage() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "locked", 0, 0, 10_000L, false, 0
        ));
        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);
        boolean hasLocked = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("劫锁已成"));
        assertTrue(hasLocked);
    }

    @Test void hiddenWhenExpired() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 1_000L, false, 0
        ));
        assertTrue(TribulationBroadcastHudPlanner.buildCommands(800, 600, 2_000L).isEmpty());
    }

    @Test void spectateHintShownWhenWithin50() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 10_000L, true, 30.0
        ));
        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);
        boolean hasSpectate = cmds.stream().anyMatch(c -> c.isText()
            && c.text().contains("观战")
            && c.text().contains("100 格内会承雷"));
        assertTrue(hasSpectate);
    }

    @Test void drawsWaveProgressFromTribulationStateStore() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "striking", 0, 0, 10_000L, false, 0
        ));
        TribulationStateStore.replace(new TribulationStateStore.State(
            true, "offline:Azure", "Azure", "du_xu", "wave", 0, 0,
            3, 5, 100, 200, 300, false, true, List.of("offline:Azure"), ""
        ));

        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);

        boolean hasProgress = cmds.stream().anyMatch(c -> c.isText()
            && c.text().contains("劫波 3/5")
            && c.text().contains("名额已满"));
        assertTrue(hasProgress);
    }

    @Test void progressLabelCoversHeartDemonPhase() {
        TribulationStateStore.State state = new TribulationStateStore.State(
            true, "offline:Azure", "Azure", "du_xu", "heart_demon", 0, 0,
            4, 5, 100, 200, 300, false, false, List.of("offline:Azure"), ""
        );

        assertEquals("心魔劫 4/5", TribulationBroadcastHudPlanner.progressLabel(state));
    }

    @Test void marksLocalPrimaryTribulator() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "Azure", "striking", 0, 0, 10_000L, false, 0
        ));
        TribulationStateStore.replace(new TribulationStateStore.State(
            true, "offline:Azure", "Azure", "du_xu", "wave", 0, 0,
            2, 5, 100, 200, 300, false, false, List.of("offline:Azure"), ""
        ));
        PlayerStateStore.replace(playerState("offline:Azure"));

        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);

        assertTrue(cmds.stream().anyMatch(c -> c.isText() && c.text().contains("渡劫者本人")));
    }

    @Test void marksLocalInterceptorAndSpectator() {
        TribulationStateStore.State state = new TribulationStateStore.State(
            true, "offline:Azure", "Azure", "du_xu", "wave", 0, 0,
            2, 5, 100, 200, 300, false, false,
            List.of("offline:Azure", "offline:Beryl"), ""
        );

        assertEquals("截胡者", TribulationBroadcastHudPlanner.viewerRoleLabel(state, "offline:Beryl"));
        assertEquals("观战者", TribulationBroadcastHudPlanner.viewerRoleLabel(state, "offline:Cedar"));
        assertEquals("观战者", TribulationBroadcastHudPlanner.viewerRoleLabel(state, ""));
    }

    private static PlayerStateViewModel playerState(String playerId) {
        return PlayerStateViewModel.create(
            "Spirit", playerId, 80.0, 100.0, 0.0, 0.5,
            PlayerStateViewModel.PowerBreakdown.empty(), PlayerStateViewModel.SocialSnapshot.empty(),
            "green_cloud_peak", "青云峰", 0.8
        );
    }
}
