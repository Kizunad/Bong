package com.bong.client.hud;

import com.bong.client.lingtian.state.LingtianSessionStore;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LingtianOverlayHudPlannerTest {
    @Test
    void activeLingtianSessionBuildsCrosshairOverlay() {
        LingtianSessionStore.Snapshot snapshot = new LingtianSessionStore.Snapshot(
            true,
            LingtianSessionStore.Kind.HARVEST,
            1,
            64,
            1,
            25,
            100,
            "凝脉草",
            "manual",
            0.4f,
            true
        );

        List<HudRenderCommand> commands = LingtianOverlayHudPlanner.buildCommands(snapshot, 320, 180);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("凝脉草 25%")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("染 40%")));
    }

    @Test
    void overlayCoordinatesAndProgressStayWithinPanelBounds() {
        LingtianSessionStore.Snapshot snapshot = new LingtianSessionStore.Snapshot(
            true,
            LingtianSessionStore.Kind.HARVEST,
            1,
            64,
            1,
            250,
            100,
            "凝脉草",
            "manual",
            0.4f,
            false
        );

        List<HudRenderCommand> commands = LingtianOverlayHudPlanner.buildCommands(snapshot, 80, 30);

        assertTrue(commands.stream().allMatch(cmd -> cmd.x() >= 0 && cmd.y() >= 0));
        assertEquals(
            LingtianOverlayHudPlanner.PANEL_WIDTH - 12,
            commands.stream()
                .filter(cmd -> cmd.isRect() && cmd.color() == LingtianOverlayHudPlanner.FILL)
                .findFirst()
                .orElseThrow()
                .width()
        );
    }
}
