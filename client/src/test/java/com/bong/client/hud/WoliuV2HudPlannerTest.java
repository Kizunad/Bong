package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WoliuV2HudPlannerTest {
    @Test
    void activeVortexShowsChargeCooldownBackfireAndTurbulence() {
        VortexStateStore.State state = new VortexStateStore.State(
            true,
            5f,
            0.8f,
            0.9f,
            80L,
            2,
            "woliu.heart",
            0.65f,
            8_000L,
            "severed",
            30f,
            0.75f,
            10_000L
        );

        List<HudRenderCommand> commands = WoliuV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(
            commands.stream().anyMatch(cmd ->
                cmd.layer() == HudRenderLayer.VORTEX_TURBULENCE && cmd.isRect() && cmd.x() > 700
            ),
            "expected right-side status panel rect because woliu HUD should render near the screen edge, actual rect x values="
                + commands.stream().filter(HudRenderCommand::isRect).map(HudRenderCommand::x).toList()
        );
        assertTextPresent(commands, "涡流");
        assertTextPresent(commands, "涡心");
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.VORTEX_BACKFIRE && cmd.isEdgeVignette()),
            "expected backfire edge vignette because backfireLevel is present, actual command count=" + commands.size()
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.VORTEX_TURBULENCE && cmd.isScreenTint()),
            "expected turbulence screen tint because active turbulence is visible, actual command count=" + commands.size()
        );
    }

    @Test
    void emptyStateDoesNotEmitVortexHud() {
        List<HudRenderCommand> commands = WoliuV2HudPlanner.buildCommands(VortexStateStore.State.NONE, 960, 540, 1_000L);

        assertTrue(
            commands.isEmpty(),
            "expected empty command list because NONE is an idle woliu state, actual command count="
                + commands.size() + ", texts=" + texts(commands)
        );
    }

    @Test
    void cooldownOverlayClampsExtremeRemainingSeconds() {
        VortexStateStore.State state = new VortexStateStore.State(
            false,
            0f,
            0f,
            0f,
            0L,
            0,
            "",
            0f,
            Long.MAX_VALUE,
            "",
            0f,
            0f,
            0L
        );

        List<HudRenderCommand> commands = WoliuV2HudPlanner.buildCommands(state, 960, 540, 0L);

        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.layer() == HudRenderLayer.VORTEX_TURBULENCE
                && cmd.isText()
                && "冷却 2147483647s".equals(cmd.text())
        ), "expected clamped cooldown text because Long.MAX_VALUE should clamp to Integer.MAX_VALUE seconds, actual texts="
            + texts(commands));
    }

    @Test
    void inactiveResidualTurbulenceDoesNotKeepWoliuHudAlive() {
        VortexStateStore.State state = new VortexStateStore.State(
            false,
            6f,
            0f,
            0f,
            0L,
            0,
            "woliu.vortex_resonance",
            0f,
            0L,
            "",
            6f,
            0.8f,
            10_000L
        );

        List<HudRenderCommand> commands = WoliuV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(
            commands.isEmpty(),
            "expected empty command list because inactive residual turbulence should not keep the HUD alive, actual command count="
                + commands.size() + ", texts=" + texts(commands)
        );
    }

    @Test
    void statusPanelShowsInactiveBackfireWithWarningColor() {
        VortexStateStore.State state = new VortexStateStore.State(
            false,
            0f,
            0f,
            0f,
            0L,
            0,
            "",
            0f,
            0L,
            "severed",
            0f,
            0f,
            0L
        );

        List<HudRenderCommand> commands = WoliuV2StatusPanelHud.buildCommands(state, 960, 540, 1_000L);

        assertTextPresent(commands, "待机");
        HudRenderCommand backfireLine = findText(commands, "拦截 0  反噬 severed");
        assertEquals(
            0xFFFFB268,
            backfireLine.color(),
            "expected warning color because backfireLevel is present even when no skill is active, actual color="
                + Integer.toHexString(backfireLine.color())
        );
        assertThrows(
            UnsupportedOperationException.class,
            () -> commands.add(HudRenderCommand.text(HudRenderLayer.VORTEX_TURBULENCE, "mutation", 0, 0, 0)),
            "expected immutable command list because HUD planners return copyOf snapshots"
        );
    }

    @Test
    void statusPanelTreatsNullSkillAndBackfireAsBlankIdleState() {
        VortexStateStore.State state = new VortexStateStore.State(
            true,
            0f,
            0f,
            0f,
            0L,
            0,
            null,
            0f,
            0L,
            null,
            0f,
            0f,
            0L
        );

        List<HudRenderCommand> commands = WoliuV2StatusPanelHud.buildCommands(state, 960, 540, 1_000L);

        assertTrue(
            commands.isEmpty(),
            "expected no panel because null activeSkillId/backfireLevel canonicalize to blank idle state, actual command count="
                + commands.size()
        );
    }

    @Test
    void statusPanelRendersActiveSkillCooldownProgressAndTurbulenceText() {
        VortexStateStore.State state = new VortexStateStore.State(
            true,
            12.25f,
            0f,
            0f,
            0L,
            3,
            "woliu.vortex_resonance",
            2f,
            2_001L,
            "",
            30f,
            2f,
            2_000L
        );

        List<HudRenderCommand> commands = WoliuV2StatusPanelHud.buildCommands(state, 960, 540, 1_000L);

        assertTextPresent(commands, "涡流");
        assertTextPresent(commands, "施放");
        assertTextPresent(commands, "涡流共振");
        assertTextPresent(commands, "冷却 2s");
        assertTextPresent(commands, "半径 30.0  强度 100%");
        assertTextPresent(commands, "拦截 3");
        assertTextPresent(commands, "紊流 1s");
        assertTrue(
            commands.stream().anyMatch(cmd ->
                cmd.layer() == HudRenderLayer.VORTEX_TURBULENCE
                    && cmd.isRect()
                    && cmd.width() == 174
                    && cmd.height() == 4
                    && cmd.color() == 0xFF62D6E8
            ),
            "expected full-width charge fill because chargeProgress clamps to 1.0, actual commands=" + commands.size()
        );
    }

    private static void assertTextPresent(List<HudRenderCommand> commands, String expectedText) {
        findText(commands, expectedText);
    }

    private static HudRenderCommand findText(List<HudRenderCommand> commands, String expectedText) {
        return commands.stream()
            .filter(cmd -> cmd.layer() == HudRenderLayer.VORTEX_TURBULENCE)
            .filter(HudRenderCommand::isText)
            .filter(cmd -> expectedText.equals(cmd.text()))
            .findFirst()
            .orElseThrow(() -> new AssertionError(
                "expected text command because status panel should render `" + expectedText + "`, actual texts="
                    + commands.stream()
                        .filter(HudRenderCommand::isText)
                        .map(HudRenderCommand::text)
                        .toList()
            ));
    }

    private static List<String> texts(List<HudRenderCommand> commands) {
        return commands.stream()
            .filter(HudRenderCommand::isText)
            .map(HudRenderCommand::text)
            .toList();
    }
}
