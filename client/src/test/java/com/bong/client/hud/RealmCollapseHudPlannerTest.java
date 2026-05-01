package com.bong.client.hud;

import com.bong.client.state.RealmCollapseHudState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class RealmCollapseHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void activeCollapseBuildsCountdownPanel() {
        RealmCollapseHudState state = RealmCollapseHudState.create(
            "blood_valley",
            "域崩撤离窗口已开启",
            1_000L,
            1_200
        );

        List<HudRenderCommand> commands = RealmCollapseHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 1_000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.REALM_COLLAPSE && cmd.isScreenTint()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("域崩撤离")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("blood valley")));
        assertTrue(commands.stream().anyMatch(HudRenderCommand::isRect));
    }

    @Test
    void expiredCollapseBuildsNoCommands() {
        RealmCollapseHudState state = RealmCollapseHudState.create(
            "blood_valley",
            "域崩撤离窗口已开启",
            1_000L,
            20
        );

        List<HudRenderCommand> commands = RealmCollapseHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 2_001L);

        assertTrue(commands.isEmpty());
    }
}
