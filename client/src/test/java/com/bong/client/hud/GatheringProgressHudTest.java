package com.bong.client.hud;

import com.bong.client.gathering.GatheringSessionViewModel;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GatheringProgressHudTest {
    private final HudTextHelper.WidthMeasurer measurer = text -> text == null ? 0 : text.length() * 6;

    @Test
    void activeSessionRendersProgressRing() {
        GatheringSessionViewModel session = GatheringSessionViewModel.create(
            "s1",
            20,
            40,
            "凝脉草",
            "herb",
            "fine_likely",
            "hoe_iron",
            false,
            false,
            1000L
        );

        List<HudRenderCommand> commands = GatheringProgressHud.buildCommands(session, measurer, 320, 240, 1100L);

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isRect));
        assertTrue(commands.stream().anyMatch(command -> command.isText() && command.text().contains("凝脉草")));
    }

    @Test
    void completedSessionAutoHidesAfterOneSecond() {
        GatheringSessionViewModel session = GatheringSessionViewModel.create(
            "s1",
            40,
            40,
            "凝脉草",
            "herb",
            "perfect",
            "hoe_iron",
            false,
            true,
            1000L
        );

        assertFalse(GatheringProgressHud.buildCommands(session, measurer, 320, 240, 1500L).isEmpty());
        assertTrue(GatheringProgressHud.buildCommands(session, measurer, 320, 240, 2101L).isEmpty());
    }

    @Test
    void qualityHintRendersResultLabelNearCompletion() {
        GatheringSessionViewModel session = GatheringSessionViewModel.create(
            "s1",
            36,
            40,
            "铜矿",
            "ore",
            "perfect_possible",
            "pickaxe_copper",
            false,
            false,
            1000L
        );

        List<HudRenderCommand> commands = GatheringProgressHud.buildCommands(session, measurer, 320, 240, 1100L);

        assertTrue(commands.stream().anyMatch(command -> command.isText() && command.text().contains("极品")));
    }
}
