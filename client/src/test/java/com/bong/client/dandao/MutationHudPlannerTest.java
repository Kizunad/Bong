package com.bong.client.dandao;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P3 -- Tests for MutationHudPlanner.
 * Validates conditional display and correct rendering per mutation stage.
 */
class MutationHudPlannerTest {
    @AfterEach
    void cleanup() {
        MutationVisualState.reset();
    }

    @Test
    void noHudWhenStageZero() {
        // Default state: stage 0 -> no HUD
        List<HudRenderCommand> commands = MutationHudPlanner.buildCommands(960, 540);
        assertTrue(commands.isEmpty(),
            "Stage 0 should produce no HUD commands (conditional display: feedback_hud_conditional.md)");
    }

    @Test
    void stageOneShowsBasicIndicator() {
        MutationVisualState.update(1, 50.0, 0.03, List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1)
        ));

        List<HudRenderCommand> commands = MutationHudPlanner.buildCommands(960, 540);

        assertFalse(commands.isEmpty(), "Stage 1 should produce HUD commands");
        assertTrue(commands.stream().anyMatch(cmd ->
                cmd.layer() == HudRenderLayer.DANDAO_MUTATION && cmd.isText()),
            "Should have stage label text");
        assertTrue(commands.stream().anyMatch(cmd ->
                cmd.layer() == HudRenderLayer.DANDAO_MUTATION && cmd.isRect()),
            "Should have progress bar rect");
    }

    @Test
    void allStagesProduceCorrectLabel() {
        assertEquals("§a微变", MutationHudPlanner.stageLabel(1));
        assertEquals("§e显变", MutationHudPlanner.stageLabel(2));
        assertEquals("§6重变", MutationHudPlanner.stageLabel(3));
        assertEquals("§c兽化", MutationHudPlanner.stageLabel(4));
        assertEquals("§7--", MutationHudPlanner.stageLabel(0));
        assertEquals("§7--", MutationHudPlanner.stageLabel(5));
    }

    @Test
    void progressComputationBoundaries() {
        // Stage 1: threshold[1]=100, prev=0, so at toxin=50 -> progress=0.5
        assertEquals(0.5, MutationHudPlanner.computeProgress(1, 50.0), 0.01);
        // Stage 1 at boundary: toxin=100 -> progress=1.0
        assertEquals(1.0, MutationHudPlanner.computeProgress(1, 100.0), 0.01);
        // Stage 1 below min: toxin=0 -> progress=0.0
        assertEquals(0.0, MutationHudPlanner.computeProgress(1, 0.0), 0.01);
        // Stage 4 at max: progress should be 1.0
        assertEquals(1.0, MutationHudPlanner.computeProgress(4, 600.0), 0.01);
    }

    @Test
    void barColorPerStage() {
        assertEquals(0xAA7ED4A0, MutationHudPlanner.barColor(1));
        assertEquals(0xAAD4C87E, MutationHudPlanner.barColor(2));
        assertEquals(0xAAD4A07E, MutationHudPlanner.barColor(3));
        assertEquals(0xAAD47E7E, MutationHudPlanner.barColor(4));
        assertEquals(0xAA888888, MutationHudPlanner.barColor(0));
    }

    @Test
    void meridianPenaltyShownWhenPositive() {
        MutationVisualState.update(2, 150.0, 0.08, List.of());

        List<HudRenderCommand> commands = MutationHudPlanner.buildCommands(960, 540);

        assertTrue(commands.stream().anyMatch(cmd ->
                cmd.isText() && cmd.text().contains("经脉")),
            "Should show meridian penalty text when penalty > 0");
    }

    @Test
    void meridianPenaltyHiddenWhenZero() {
        MutationVisualState.update(1, 50.0, 0.0, List.of());

        List<HudRenderCommand> commands = MutationHudPlanner.buildCommands(960, 540);

        assertTrue(commands.stream().noneMatch(cmd ->
                cmd.isText() && cmd.text().contains("经脉")),
            "Should not show meridian penalty text when penalty is 0");
    }

    @Test
    void slotListRenderedWhenSlotsActive() {
        MutationVisualState.update(2, 150.0, 0.08, List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1),
            new MutationVisualState.MutationSlotEntry("ForearmScales", "Forearm", 2)
        ));

        List<HudRenderCommand> commands = MutationHudPlanner.buildCommands(960, 540);

        long slotTexts = commands.stream()
            .filter(cmd -> cmd.isText() && cmd.text().contains("◆"))
            .count();
        assertEquals(2, slotTexts, "Should render one text per active slot");
    }

    @Test
    void hasExtraArmsDetectsMultiArmSlot() {
        MutationVisualState.update(4, 600.0, 0.20, List.of(
            new MutationVisualState.MutationSlotEntry("ExtraArms", "Torso", 3)
        ));

        assertTrue(MutationHudPlanner.hasExtraArms(),
            "Should detect ExtraArms mutation for HUD conditional display");

        MutationVisualState.update(2, 150.0, 0.08, List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1)
        ));

        assertFalse(MutationHudPlanner.hasExtraArms(),
            "Should not detect ExtraArms when not present");
    }
}
