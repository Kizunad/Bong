package com.bong.client.hud;

import com.bong.client.movement.MovementState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementHudPlannerTest {
    @Test
    void hidesAfterAutoHideFadeWindow() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        List<HudRenderCommand> visibleAtHoverEnd = MovementHudPlanner.buildCommands(state, 800, 600, 4_000L);
        List<HudRenderCommand> visibleAtFadeLastMs = MovementHudPlanner.buildCommands(state, 800, 600, 4_499L);
        List<HudRenderCommand> hiddenAtFadeEnd = MovementHudPlanner.buildCommands(state, 800, 600, 4_500L);
        assertFalse(
            visibleAtHoverEnd.isEmpty(),
            "expected HUD commands because hover visible window includes elapsed=3000ms, actual empty"
        );
        assertFalse(
            visibleAtFadeLastMs.isEmpty(),
            "expected HUD commands because fade window includes elapsed=3499ms, actual empty"
        );
        assertTrue(
            hiddenAtFadeEnd.isEmpty(),
            "expected 0 commands because HUD fade window expired, actual size: "
                + hiddenAtFadeEnd.size() + ", commands: " + hiddenAtFadeEnd
        );
    }

    @Test
    void lowStaminaDoesNotKeepDashIndicatorVisible() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, true, 1_000L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 20_000L);

        assertTrue(
            commands.isEmpty(),
            "expected HUD commands to be empty because low stamina alone should not pin dash HUD, actual size: "
                + commands.size() + ", commands: " + commands
        );
    }

    @Test
    void rendersOnlyDashCooldownPanelWithoutStaminaBar() {
        MovementState state = new MovementState(
            0.75,
            true,
            MovementState.Action.DASHING,
            MovementState.ZoneKind.NORMAL,
            20,
            1.8,
            70,
            100,
            false,
            12L,
            "",
            1_000L,
            1_000L,
            0L
        );

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 1_100L);

        long movementRects = commands.stream().filter(MovementHudPlannerTest::isMovementRect).count();
        assertEquals(
            3,
            movementRects,
            "expected 3 movement rects because panel, cooldown track, and cooldown fill render without stamina, actual: "
                + movementRects + ", commands: " + commands
        );
        assertTrue(
            commands.stream().anyMatch(c -> c.isScaledText() && "DASH".equals(c.text())),
            "expected DASH label because movement HUD only keeps dash cooldown, commands: " + commands
        );
        assertTrue(
            commands.stream().noneMatch(c -> isMovementRect(c) && c.color() == 0xFFFFD060),
            "expected no stamina-colored rect because stamina moved to player status HUD, commands: " + commands
        );
    }

    @Test
    void dashPanelSitsBesideHotbarWithoutHorizontalOverlap() {
        MovementState state = state(MovementState.Action.DASHING, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 398, 121, 1_100L);

        HudRenderCommand panel = movementPanel(commands);
        int hotbarLeftX = hotbarLeftX(398);
        int hotbarRightX = hotbarRightX(398);

        assertTrue(
            panel.x() >= hotbarRightX || panel.x() + panel.width() <= hotbarLeftX,
            "expected movement panel beside hotbar without horizontal overlap, actual panel: "
                + panel + ", hotbarLeftX: " + hotbarLeftX + ", hotbarRightX: " + hotbarRightX
        );
    }

    @Test
    void dashPanelUsesLeftSideBeforeRightWeaponReservedSlotWhenFullReservedSideFails() {
        MovementState state = state(MovementState.Action.DASHING, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        int screenWidth = 350;
        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, screenWidth, 121, 1_100L);

        HudRenderCommand panel = movementPanel(commands);
        int hotbarLeftX = hotbarLeftX(screenWidth);

        assertTrue(
            panel.x() + panel.width() <= hotbarLeftX,
            "expected movement panel to use available left side before occupying right weapon reserved slot, actual panel: "
                + panel + ", hotbarLeftX: " + hotbarLeftX
        );
    }

    @Test
    void dashPanelMovesAboveHotbarWhenNoSideSlotFits() {
        MovementState state = state(MovementState.Action.DASHING, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        int screenWidth = 320;
        int screenHeight = 121;
        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, screenWidth, screenHeight, 1_100L);

        HudRenderCommand panel = movementPanel(commands);
        int upperY = hotbarUpperY(screenHeight);

        assertTrue(
            panel.y() + panel.height() <= upperY,
            "expected movement panel above hotbar because neither side slot fits, actual panel: "
                + panel + ", hotbarUpperY: " + upperY
            );
    }

    @Test
    void dashPanelDoesNotRenderWhenViewportCannotFitPanel() {
        MovementState state = state(MovementState.Action.DASHING, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 67, 21, 1_100L);

        assertTrue(
            commands.isEmpty(),
            "expected movement panel to skip rendering because viewport cannot fit panel plus edge margins, commands: "
                + commands
        );
    }

    @Test
    void deadZoneAddsVignetteEvenWithoutRecentAction() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.DEAD, false, 0L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 10_000L);

        assertZoneVignette(commands, 0x66000000, "dead zone");
    }

    @Test
    void negativeZoneAddsBlueVignetteEvenWithoutRecentAction() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NEGATIVE, false, 0L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 10_000L);

        assertZoneVignette(commands, 0x553A4A7A, "negative zone");
    }

    @Test
    void residueAshZoneAddsAshVignetteEvenWithoutRecentAction() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.RESIDUE_ASH, false, 0L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 10_000L);

        assertZoneVignette(commands, 0x554B3A2E, "residue ash zone");
    }

    @Test
    void rejectedActionAddsRedFlashRect() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, false, 1_000L, 1_000L)
            .withTiming(1_000L, 1_000L, 1_000L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 1_100L);

        assertTrue(
            commands.stream().anyMatch(c -> isMovementRect(c) && c.color() == 0xC0FF3030),
            "expected red rejection flash rect because action was rejected recently, commands: " + commands
        );
    }

    private static boolean isMovementRect(HudRenderCommand command) {
        return command.layer() == HudRenderLayer.MOVEMENT_HUD && command.isRect();
    }

    private static HudRenderCommand movementPanel(List<HudRenderCommand> commands) {
        return commands.stream()
            .filter(c -> isMovementRect(c)
                && c.width() == MovementHudPlanner.PANEL_WIDTH
                && c.height() == MovementHudPlanner.PANEL_HEIGHT)
            .findFirst()
            .orElseThrow(() -> new AssertionError(
                "expected movement panel rect because dash HUD is visible, commands: " + commands
            ));
    }

    private static void assertZoneVignette(List<HudRenderCommand> commands, int expectedColor, String reason) {
        List<HudRenderCommand> vignettes = commands.stream()
            .filter(HudRenderCommand::isEdgeVignette)
            .toList();
        assertEquals(
            1,
            vignettes.size(),
            "expected one " + reason + " vignette because zone feedback renders without recent action, actual: "
                + vignettes.size() + ", commands: " + commands
        );
        assertEquals(
            expectedColor,
            vignettes.get(0).color(),
            "expected " + reason + " vignette color to match MovementHudPlanner zone mapping, actual: "
                + vignettes.get(0).color()
        );
    }

    private static int hotbarLeftX(int screenWidth) {
        int hotbarWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        return (screenWidth - hotbarWidth) / 2;
    }

    private static int hotbarRightX(int screenWidth) {
        int hotbarWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        return hotbarLeftX(screenWidth) + hotbarWidth;
    }

    private static int hotbarUpperY(int screenHeight) {
        int lowerY = screenHeight - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN - QuickBarHudPlanner.SLOT_SIZE;
        return lowerY - QuickBarHudPlanner.SLOT_SIZE - QuickBarHudPlanner.UPPER_GAP;
    }

    private static MovementState state(
        MovementState.Action action,
        MovementState.ZoneKind zone,
        boolean lowStamina,
        long hudActivityAtMs,
        long rejectedAtMs
    ) {
        return new MovementState(
            0.75,
            false,
            action,
            zone,
            0,
            1.8,
            lowStamina ? 20 : 80,
            100,
            lowStamina,
            hudActivityAtMs > 0 ? 10L : null,
            rejectedAtMs > 0 ? "dash" : "",
            hudActivityAtMs,
            hudActivityAtMs,
            rejectedAtMs
        );
    }
}
