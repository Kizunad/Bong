package com.bong.client.hud;

import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.ZoneState;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class QiDensityRadarHudPlannerTest {
    @Test
    void radarHiddenBelowNingmai() {
        List<HudRenderCommand> commands = QiDensityRadarHudPlanner.buildCommands(
            player("Induce"),
            ZoneState.create("jade", "青谷", 0.8, 1, 0L),
            PerceptionEdgeState.empty(),
            HudImmersionMode.Mode.PEACE,
            HudEnvironmentVariant.NORMAL,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );

        assertTrue(commands.isEmpty());
    }

    @Test
    void radarNegativeQiInvertMarker() {
        List<HudRenderCommand> commands = QiDensityRadarHudPlanner.buildCommands(
            player("Condense"),
            ZoneState.create("negative", "负灵域", -0.5, 3, "normal", 0L),
            PerceptionEdgeState.empty(),
            HudImmersionMode.Mode.COMBAT,
            HudEnvironmentVariant.NEGATIVE_QI,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );

        assertFalse(commands.isEmpty());
        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.layer() == HudRenderLayer.QI_RADAR
                && cmd.isRect()
                && (cmd.color() & 0x00FFFFFF) == (QiDensityRadarHudPlanner.NEGATIVE_QI & 0x00FFFFFF)
        ));
    }

    @Test
    void tsyRadarAddsFalseMarker() {
        List<HudRenderCommand> normal = QiDensityRadarHudPlanner.buildCommands(
            player("Condense"),
            ZoneState.create("jade", "青谷", 0.5, 1, 0L),
            PerceptionEdgeState.empty(),
            HudImmersionMode.Mode.COMBAT,
            HudEnvironmentVariant.NORMAL,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );
        List<HudRenderCommand> tsy = QiDensityRadarHudPlanner.buildCommands(
            player("Condense"),
            ZoneState.create("tsy", "坍缩渊", 0.5, 1, 0L),
            PerceptionEdgeState.empty(),
            HudImmersionMode.Mode.COMBAT,
            HudEnvironmentVariant.TSY,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );

        assertTrue(tsy.size() > normal.size());
    }

    private static PlayerStateViewModel player(String realm) {
        return PlayerStateViewModel.create(
            realm,
            "offline:test",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "jade",
            "青谷",
            0.5
        );
    }
}
