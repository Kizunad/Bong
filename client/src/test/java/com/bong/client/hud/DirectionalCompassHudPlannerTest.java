package com.bong.client.hud;

import com.bong.client.state.ZoneState;
import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.RiftPortalView;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

class DirectionalCompassHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void compassZoneNameFlash() {
        ZoneState zone = ZoneState.create("jade", "青谷", 0.8, 1, 1_000L);

        List<HudRenderCommand> commands = DirectionalCompassHudPlanner.buildCommands(
            zone,
            ExtractState.empty(),
            HudImmersionMode.Mode.PEACE,
            HudRuntimeContext.empty(),
            WIDTH,
            320,
            180,
            1_500L
        );

        assertTrue(commands.stream().anyMatch(cmd ->
            "青谷".equals(cmd.text()) && cmd.color() == DirectionalCompassHudPlanner.FLASH_TEXT
        ));
    }

    @Test
    void nicheMarkerOnCompass() {
        HudRuntimeContext runtime = new HudRuntimeContext(
            0.0,
            0.0,
            64.0,
            0.0,
            false,
            List.of(new HudRuntimeContext.CompassMarker(
                HudRuntimeContext.CompassMarker.Kind.SPIRIT_NICHE,
                10.0,
                10.0,
                1.0
            ))
        );

        List<HudRenderCommand> commands = DirectionalCompassHudPlanner.buildCommands(
            ZoneState.create("jade", "青谷", 0.8, 1, 0L),
            ExtractState.empty(),
            HudImmersionMode.Mode.PEACE,
            runtime,
            WIDTH,
            320,
            180,
            1_000L
        );

        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isRect() && (cmd.color() & 0x00FFFFFF) == (DirectionalCompassHudPlanner.NICHE_MARKER & 0x00FFFFFF)
        ));
    }

    @Test
    void collapseMarkerUsesFrameTimestamp() {
        ExtractState extractState = new ExtractState(
            List.of(new RiftPortalView(42L, "collapse_tear", "exit", "tsy_lingxu_01", 0.0, 64.0, 10.0, 2.0, 0, null)),
            null,
            "",
            0,
            0,
            false,
            "",
            0xFFFFFFFF,
            0L,
            "tsy_lingxu_01",
            1_000L,
            100,
            0L,
            0,
            2_000L
        );

        List<HudRenderCommand> commands = DirectionalCompassHudPlanner.buildCommands(
            ZoneState.create("tsy", "坍缩渊", 0.5, 1, 0L),
            extractState,
            HudImmersionMode.Mode.PEACE,
            HudRuntimeContext.empty(),
            WIDTH,
            320,
            180,
            2_000L
        );

        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isRect() && (cmd.color() & 0x00FFFFFF) == (DirectionalCompassHudPlanner.COLLAPSE_EXIT_MARKER & 0x00FFFFFF)
        ));
    }
}
