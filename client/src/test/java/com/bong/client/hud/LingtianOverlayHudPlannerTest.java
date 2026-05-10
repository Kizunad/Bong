package com.bong.client.hud;

import com.bong.client.lingtian.state.LingtianSessionStore;
import org.junit.jupiter.api.Test;

import java.util.List;

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
}
