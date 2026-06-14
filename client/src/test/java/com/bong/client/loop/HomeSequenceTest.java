package com.bong.client.loop;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudRuntimeContext;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.AudioPlaybackBridge;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class HomeSequenceTest {
    @BeforeEach
    void prepare() {
        HomeSequence.resetForTests();
    }

    @AfterEach
    void reset() {
        HomeSequence.resetForTests();
    }

    @Test
    void entersHomeWithinFiveBlocksAndPlaysSafeCue() {
        RecordingAudioBridge audio = new RecordingAudioBridge();
        HomeSequence.setAudioBridgeForTests(audio);

        HomeSequence.State state = HomeSequence.update(
            runtime(10.0, 10.0, marker(13.0, 14.0)),
            InventoryModel.empty(),
            1_000L
        );

        assertTrue(state.insideHome(), "expected insideHome=true because marker is within 5 blocks, actual false");
        assertTrue(state.enteredThisTick(), "expected enteredThisTick=true because previous state was away, actual false");
        assertEquals(1, audio.played.size());
        assertEquals("home_safe_crackle", audio.played.get(0).recipeId());
    }

    @Test
    void ignoresSpiritNicheOutsideFiveBlocks() {
        HomeSequence.State state = HomeSequence.update(
            runtime(10.0, 10.0, marker(16.0, 10.0)),
            InventoryModel.empty(),
            1_000L
        );

        assertFalse(state.insideHome(), "expected insideHome=false because marker is 6 blocks away, actual true");
        assertTrue(
            HomeSequence.buildCommands(state, 320, 180, 1_000L).isEmpty(),
            "expected no home commands because player is outside home radius, actual commands rendered"
        );
    }

    @Test
    void treatsExactlyFiveBlocksAsInsideAndBeyondFiveAsOutside() {
        HomeSequence.State boundary = HomeSequence.update(
            runtime(10.0, 10.0, marker(13.0, 14.0)),
            InventoryModel.empty(),
            1_000L
        );

        assertTrue(boundary.insideHome(), "expected insideHome=true because distance is exactly 5.0, actual false");

        HomeSequence.State beyond = HomeSequence.update(
            runtime(10.0, 10.0, marker(13.01, 14.0)),
            InventoryModel.empty(),
            2_000L
        );

        assertFalse(beyond.insideHome(), "expected insideHome=false because distance is greater than 5.0, actual true");
    }

    @Test
    void handlesNullRuntimeAndInventoryAsAwayEmptyState() {
        HomeSequence.State state = HomeSequence.update(null, null, -100L);

        assertFalse(state.insideHome(), "expected insideHome=false because null runtime has no markers, actual true");
        assertFalse(state.enteredThisTick(), "expected enteredThisTick=false because null runtime cannot enter home, actual true");
    }

    @Test
    void rendersNoForcedChecklistAndSettlingHintAtHome() {
        HomeSequence.State state = HomeSequence.update(
            runtime(10.0, 10.0, marker(11.0, 11.0)),
            InventoryModel.empty(),
            4_000L
        );

        List<HudRenderCommand> commands = HomeSequence.buildCommands(state, 320, 180, 5_000L);

        assertTrue(
            commands.stream().anyMatch(command -> command.layer() == HudRenderLayer.HOME_SEQUENCE),
            "expected HOME_SEQUENCE layer because player is inside home radius, actual layer missing"
        );
        assertTrue(
            commands.stream().anyMatch(command -> "回到灵龛".equals(command.text())),
            "expected home title because home panel should render, actual title missing"
        );
        assertTrue(
            commands.stream().anyMatch(command -> command.text().contains("整理：背包")),
            "expected organize hint because P3 requires run-end sorting guidance, actual hint missing"
        );
        assertTrue(
            commands.stream().anyMatch(command -> command.text().contains("靠墙坐下")),
            "expected settling hint because animation window is active, actual hint missing"
        );
    }

    @Test
    void buildCommandsRejectsInvalidScreenDimensions() {
        HomeSequence.State state = HomeSequence.update(
            runtime(10.0, 10.0, marker(11.0, 11.0)),
            InventoryModel.empty(),
            4_000L
        );

        assertTrue(
            HomeSequence.buildCommands(state, 0, 180, 5_000L).isEmpty(),
            "expected no commands because screen width is zero, actual commands rendered"
        );
        assertTrue(
            HomeSequence.buildCommands(state, 320, -1, 5_000L).isEmpty(),
            "expected no commands because screen height is negative, actual commands rendered"
        );
    }

    @Test
    void newBadgeWindowMarksItemsGainedDuringAwayRun() {
        InventoryItem oldItem = InventoryItem.createFull(
            11L,
            "dry_root",
            "枯根",
            1,
            1,
            0.1,
            "common",
            "",
            1,
            1.0,
            1.0
        );
        InventoryItem newItem = InventoryItem.createFull(
            12L,
            "red_pith_grass",
            "赤髓草",
            1,
            1,
            0.1,
            "common",
            "",
            1,
            1.0,
            1.0
        );
        HomeSequence.update(runtime(40.0, 40.0, marker(10.0, 10.0)), inventory(oldItem), 1_000L);
        HomeSequence.update(runtime(35.0, 35.0, marker(10.0, 10.0)), inventory(oldItem, newItem), 1_500L);

        HomeSequence.update(runtime(10.0, 10.0, marker(10.5, 10.5)), inventory(oldItem, newItem), 2_000L);

        assertFalse(HomeSequence.newBadgeActive(oldItem, 2_100L), "expected old item without NEW because it was in run baseline, actual active");
        assertTrue(HomeSequence.newBadgeActive(newItem, 2_100L), "expected new item with NEW because it was gained during away run, actual inactive");
        assertTrue(
            HomeSequence.newBadgeActive(newItem, 31_999L),
            "expected NEW active before 30s expiry boundary, actual inactive"
        );
        assertFalse(
            HomeSequence.newBadgeActive(newItem, 32_000L),
            "expected NEW inactive exactly at 30s expiry boundary, actual active"
        );
    }

    @Test
    void homeEntryAudioOnlyReplaysAfterLeavingAndReturning() {
        RecordingAudioBridge audio = new RecordingAudioBridge();
        HomeSequence.setAudioBridgeForTests(audio);

        HomeSequence.State first = HomeSequence.update(
            runtime(10.0, 10.0, marker(11.0, 11.0)),
            InventoryModel.empty(),
            1_000L
        );
        HomeSequence.State stillInside = HomeSequence.update(
            runtime(10.5, 10.5, marker(11.0, 11.0)),
            InventoryModel.empty(),
            2_000L
        );
        HomeSequence.State away = HomeSequence.update(
            runtime(30.0, 30.0, marker(11.0, 11.0)),
            InventoryModel.empty(),
            3_000L
        );
        HomeSequence.State returned = HomeSequence.update(
            runtime(10.0, 10.0, marker(11.0, 11.0)),
            InventoryModel.empty(),
            4_000L
        );

        assertTrue(first.enteredThisTick(), "expected first home update to enter because previous state was away, actual false");
        assertFalse(stillInside.enteredThisTick(), "expected inside→inside to avoid re-entry because player never left, actual true");
        assertFalse(away.insideHome(), "expected away state after moving outside radius, actual inside");
        assertTrue(returned.enteredThisTick(), "expected inside→away→inside to re-enter because player returned, actual false");
        assertEquals(2, audio.played.size());
    }

    private static HudRuntimeContext runtime(double playerX, double playerZ, HudRuntimeContext.CompassMarker marker) {
        return new HudRuntimeContext(0.0, playerX, 64.0, playerZ, false, List.of(marker));
    }

    private static HudRuntimeContext.CompassMarker marker(double x, double z) {
        return new HudRuntimeContext.CompassMarker(
            HudRuntimeContext.CompassMarker.Kind.SPIRIT_NICHE,
            x,
            z,
            1.0
        );
    }

    private static InventoryModel inventory(InventoryItem... items) {
        InventoryModel.Builder builder = InventoryModel.builder();
        int col = 0;
        for (InventoryItem item : items) {
            builder.gridItem(item, InventoryModel.BODY_POCKET_CONTAINER_ID, 0, col++);
        }
        return builder.build();
    }

    private static final class RecordingAudioBridge implements AudioPlaybackBridge {
        final List<AudioEventPayload.PlaySoundRecipe> played = new ArrayList<>();

        @Override
        public boolean play(AudioEventPayload.PlaySoundRecipe payload) {
            played.add(payload);
            return true;
        }

        @Override
        public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
            return true;
        }
    }
}
