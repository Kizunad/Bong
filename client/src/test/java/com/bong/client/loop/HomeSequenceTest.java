package com.bong.client.loop;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudRuntimeContext;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.AudioPlaybackBridge;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class HomeSequenceTest {
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

        assertTrue(state.insideHome());
        assertTrue(state.enteredThisTick());
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

        assertFalse(state.insideHome());
        assertTrue(HomeSequence.buildCommands(state, 320, 180, 1_000L).isEmpty());
    }

    @Test
    void rendersNoForcedChecklistAndSettlingHintAtHome() {
        HomeSequence.State state = HomeSequence.update(
            runtime(10.0, 10.0, marker(11.0, 11.0)),
            InventoryModel.empty(),
            4_000L
        );

        List<HudRenderCommand> commands = HomeSequence.buildCommands(state, 320, 180, 5_000L);

        assertTrue(commands.stream().anyMatch(command -> command.layer() == HudRenderLayer.HOME_SEQUENCE));
        assertTrue(commands.stream().anyMatch(command -> "回到灵龛".equals(command.text())));
        assertTrue(commands.stream().anyMatch(command -> command.text().contains("整理：背包")));
        assertTrue(commands.stream().anyMatch(command -> command.text().contains("靠墙坐下")));
    }

    @Test
    void newBadgeWindowMarksItemsGainedBeforeHomeEntry() {
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

        HomeSequence.update(runtime(10.0, 10.0, marker(10.5, 10.5)), inventory(oldItem, newItem), 2_000L);

        assertFalse(HomeSequence.newBadgeActive(oldItem, 2_100L));
        assertTrue(HomeSequence.newBadgeActive(newItem, 2_100L));
        assertFalse(HomeSequence.newBadgeActive(newItem, 32_001L));
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
