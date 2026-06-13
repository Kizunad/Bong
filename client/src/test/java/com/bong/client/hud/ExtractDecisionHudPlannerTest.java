package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.SeasonState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ExtractDecisionHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer WIDTH = text -> text == null ? 0 : text.length() * 6;
    private static final PlayerStateViewModel PLAYER = PlayerStateViewModel.create(
        "Induce",
        "offline:test",
        80.0,
        100.0,
        0.0,
        0.4,
        PlayerStateViewModel.PowerBreakdown.empty(),
        PlayerStateViewModel.SocialSnapshot.empty(),
        "spawn",
        "荒坡",
        0.35
    );

    @AfterEach
    void resetState() {
        ExtractDecisionStateStore.resetForTests();
    }

    @Test
    void extract_checklist_color_by_threshold() {
        PlayerStateViewModel lowQi = PlayerStateViewModel.create(
            "Induce",
            "offline:test",
            25.0,
            100.0,
            0.0,
            0.4,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "spawn",
            "荒坡",
            0.35
        );
        InventoryModel heavyBag = InventoryModel.builder().weight(41.0, 50.0).build();
        ExtractDecisionStateStore.Snapshot state = new ExtractDecisionStateStore.Snapshot(
            1_000L,
            70,
            false,
            0L,
            40
        );

        List<ExtractDecisionHudPlanner.DecisionRow> rows = ExtractDecisionHudPlanner.buildRows(
            lowQi,
            heavyBag,
            SeasonState.summerAt(0L),
            state,
            41L * 60L * 1000L
        );

        assertEquals(ExtractDecisionHudPlanner.Severity.CRITICAL, row(rows, "真元").severity());
        assertEquals(ExtractDecisionHudPlanner.Severity.WARNING, row(rows, "背包").severity());
        assertEquals(ExtractDecisionHudPlanner.Severity.WARNING, row(rows, "时辰").severity());
        assertEquals(ExtractDecisionHudPlanner.Severity.WARNING, row(rows, "威胁").severity());
    }

    @Test
    void checklist_no_numbers() {
        ExtractDecisionStateStore.Snapshot state = new ExtractDecisionStateStore.Snapshot(
            1_000L,
            85,
            true,
            20_000L,
            30
        );

        List<HudRenderCommand> commands = ExtractDecisionHudPlanner.buildCommands(
            PLAYER,
            InventoryModel.builder().weight(45.0, 50.0).build(),
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 10L, 100L, 0L),
            state,
            WIDTH,
            320,
            180,
            2_000L
        );

        Pattern digit = Pattern.compile("\\d");
        assertFalse(commands.isEmpty());
        assertTrue(commands.stream()
            .filter(HudRenderCommand::isText)
            .map(HudRenderCommand::text)
            .noneMatch(text -> digit.matcher(text).find()));
    }

    @Test
    void tideTurnMarksRhythmUnstable() {
        List<ExtractDecisionHudPlanner.DecisionRow> rows = ExtractDecisionHudPlanner.buildRows(
            PLAYER,
            InventoryModel.empty(),
            new SeasonState(SeasonState.Phase.WINTER_TO_SUMMER, 1L, 100L, 0L),
            ExtractDecisionStateStore.Snapshot.EMPTY,
            0L
        );

        assertEquals(ExtractDecisionHudPlanner.Severity.WARNING, row(rows, "汐转").severity());
        assertEquals("不稳", row(rows, "汐转").status());
    }

    @Test
    void emergencyWindowAddsRetreatVignette() {
        ExtractDecisionStateStore.Snapshot state = new ExtractDecisionStateStore.Snapshot(
            1_000L,
            90,
            true,
            10_000L,
            30
        );

        List<HudRenderCommand> commands = ExtractDecisionHudPlanner.buildCommands(
            PLAYER,
            InventoryModel.empty(),
            SeasonState.summerAt(0L),
            state,
            WIDTH,
            320,
            180,
            2_000L
        );

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isEdgeVignette));
        assertTrue(commands.stream().anyMatch(command -> "快撤".equals(command.text())));
    }

    @Test
    void riskSpikeOpensEmergencyWindowOnlyWhenRealmMismatched() {
        ExtractDecisionStateStore.replaceRiskScore(55, true, 1_000L);
        ExtractDecisionStateStore.replaceRiskScore(85, false, 2_000L);
        assertFalse(ExtractDecisionStateStore.snapshot().emergencyActive(2_100L));

        ExtractDecisionStateStore.replaceRiskScore(55, true, 3_000L);
        ExtractDecisionStateStore.replaceRiskScore(85, true, 4_000L);
        assertTrue(ExtractDecisionStateStore.snapshot().emergencyActive(4_100L));
    }

    @Test
    void unknownPlayerAndInventoryDoNotFakeLowQiWarning() {
        List<ExtractDecisionHudPlanner.DecisionRow> rows = ExtractDecisionHudPlanner.buildRows(
            PlayerStateViewModel.empty(),
            InventoryModel.empty(),
            SeasonState.summerAt(0L),
            new ExtractDecisionStateStore.Snapshot(0L, 75, true, 0L, 20),
            0L
        );

        assertEquals(ExtractDecisionHudPlanner.Severity.STEADY, row(rows, "真元").severity());
        assertEquals("未明", row(rows, "真元").status());
    }

    @Test
    void playerSnapshotStartsRunClockOnce() {
        ExtractDecisionStateStore.markPlayerInWorld(1_000L);
        assertEquals(1_000L, ExtractDecisionStateStore.snapshot().runStartedAtMs());

        ExtractDecisionStateStore.markPlayerInWorld(5_000L);
        assertEquals(1_000L, ExtractDecisionStateStore.snapshot().runStartedAtMs());

        ExtractDecisionStateStore.clearOnDisconnect();
        assertFalse(ExtractDecisionStateStore.snapshot().hasRunClock());
    }

    @Test
    void inactiveWithoutPlayerDataEmitsNothing() {
        List<HudRenderCommand> commands = ExtractDecisionHudPlanner.buildCommands(
            PlayerStateViewModel.empty(),
            InventoryModel.empty(),
            SeasonState.summerAt(0L),
            ExtractDecisionStateStore.Snapshot.EMPTY,
            WIDTH,
            320,
            180,
            0L
        );

        assertTrue(commands.isEmpty());
    }

    @Test
    void invalidScreenBoundsEmitNothing() {
        List<HudRenderCommand> commands = ExtractDecisionHudPlanner.buildCommands(
            PLAYER,
            InventoryModel.empty(),
            SeasonState.summerAt(0L),
            ExtractDecisionStateStore.Snapshot.EMPTY,
            WIDTH,
            0,
            180,
            0L
        );

        assertTrue(commands.isEmpty());
    }

    private static ExtractDecisionHudPlanner.DecisionRow row(
        List<ExtractDecisionHudPlanner.DecisionRow> rows,
        String label
    ) {
        return rows.stream()
            .filter(row -> label.equals(row.label()))
            .findFirst()
            .orElseThrow();
    }
}
