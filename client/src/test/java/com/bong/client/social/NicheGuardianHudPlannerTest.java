package com.bong.client.social;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F5 fix — {@link NicheGuardianStore} previously only received writes (from
 * {@code NicheIntrusionAlertHandler}) and was never rendered by any HUD planner.
 * Mirrors {@code com.bong.client.npc.NpcInteractionLogHudPlannerTest}'s gating coverage.
 */
class NicheGuardianHudPlannerTest {
    @AfterEach
    void reset() {
        NicheGuardianStore.resetForTests();
    }

    @Test
    void emptyStoreProducesNoCommands() {
        List<HudRenderCommand> commands = NicheGuardianHudPlanner.buildCommands(320, 180);

        assertTrue(
            commands.isEmpty(),
            "expected empty command list when NicheGuardianStore has no guardian status and no intrusion alert "
                + "(HUD conditional display: never-activated feature must be fully hidden, not shown empty/placeholder), "
                + "actual size: " + commands.size()
        );
    }

    @Test
    void guardianStatusAloneProducesCommands() {
        NicheGuardianStore.recordFatigue("puppet", 3);

        List<HudRenderCommand> commands = NicheGuardianHudPlanner.buildCommands(320, 180);

        assertFalse(commands.isEmpty(), "expected non-empty commands once a guardian status exists, actual: empty");
        assertTrue(
            commands.stream().anyMatch(c -> c.text().contains("puppet x3")),
            "expected a text command containing 'puppet x3' (charges remaining), actual commands: " + commands
        );
    }

    @Test
    void intrusionAlertAloneProducesCommands() {
        NicheGuardianStore.recordIntrusion(new NicheGuardianStore.NicheIntrusionAlert(
            List.of(42L), "char:raider", 0.2, 1_000L
        ));

        List<HudRenderCommand> commands = NicheGuardianHudPlanner.buildCommands(320, 180);

        assertFalse(commands.isEmpty(), "expected non-empty commands once an intrusion alert exists (no guardian needed)");
        assertTrue(commands.stream().anyMatch(c -> c.text().contains("char:raider")));
    }

    @Test
    void allCommandsUseNicheGuardianLayerExclusively() {
        NicheGuardianStore.recordFatigue("puppet", 1);

        List<HudRenderCommand> commands = NicheGuardianHudPlanner.buildCommands(320, 180);

        for (HudRenderCommand command : commands) {
            assertEquals(
                HudRenderLayer.NICHE_GUARDIAN,
                command.layer(),
                "expected every command emitted by NicheGuardianHudPlanner to use HudRenderLayer.NICHE_GUARDIAN, actual: "
                    + command.layer()
            );
        }
    }

    @Test
    void zeroScreenWidthProducesNoCommandsEvenWithData() {
        NicheGuardianStore.recordFatigue("puppet", 1);

        assertTrue(NicheGuardianHudPlanner.buildCommands(0, 180).isEmpty());
    }

    @Test
    void zeroScreenHeightProducesNoCommandsEvenWithData() {
        NicheGuardianStore.recordFatigue("puppet", 1);

        assertTrue(NicheGuardianHudPlanner.buildCommands(320, 0).isEmpty());
    }

    @Test
    void brokenGuardianStatusIsFlaggedInOutput() {
        NicheGuardianStore.recordBroken("puppet", "char:looter");

        List<HudRenderCommand> commands = NicheGuardianHudPlanner.buildCommands(320, 180);

        assertTrue(
            commands.stream().anyMatch(c -> c.text().contains("broken")),
            "expected 'broken' marker text once the guardian is recorded as broken, actual commands: " + commands
        );
        // recordBroken also appends an intrusion alert internally (NicheGuardianStore contract) —
        // the panel should show both the broken guardian row and the resulting intrusion row.
        assertTrue(commands.stream().anyMatch(c -> c.text().contains("龛侵")));
    }

    @Test
    void titleRowIsAlwaysPresentWhenPanelRenders() {
        NicheGuardianStore.recordFatigue("puppet", 5);

        List<HudRenderCommand> commands = NicheGuardianHudPlanner.buildCommands(320, 180);

        assertTrue(commands.stream().anyMatch(c -> "灵龛守护".equals(c.text())));
    }
}
