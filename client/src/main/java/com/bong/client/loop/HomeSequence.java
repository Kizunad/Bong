package com.bong.client.loop;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudRuntimeContext;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.AudioPlaybackBridge;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;

public final class HomeSequence {
    public static final double HOME_RADIUS_BLOCKS = 5.0;
    public static final long SETTLE_ANIMATION_MS = 3_000L;
    public static final long NEW_BADGE_MS = 30_000L;

    private static final int PANEL_WIDTH = 168;
    private static final int PANEL_HEIGHT = 58;
    private static final int HOME_TEXT = 0xFFE8F1D8;
    private static final int HOME_DIM = 0xFFB8C8A8;
    private static final String HOME_AUDIO_RECIPE_ID = "home_safe_crackle";
    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(20_000L);

    private static State state = State.away();
    private static Set<String> knownInventoryKeys = new HashSet<>();
    private static final Map<String, Long> newItemUntilMs = new HashMap<>();
    private static AudioPlaybackBridge audioBridge = SoundRecipePlayer.instance();

    private HomeSequence() {
    }

    public static State update(HudRuntimeContext runtimeContext, InventoryModel inventory, long nowMillis) {
        HudRuntimeContext runtime = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
        long now = Math.max(0L, nowMillis);
        HomeMarker nearest = nearestHomeMarker(runtime);
        boolean insideHome = nearest != null && nearest.distanceBlocks() <= HOME_RADIUS_BLOCKS;
        boolean enteredThisTick = insideHome && !state.insideHome();

        observeInventory(inventory, enteredThisTick, now);
        if (enteredThisTick) {
            playHomeAudio();
        }

        state = insideHome
            ? new State(true, enteredThisTick, state.insideHome() ? state.enteredAtMs() : now, nearest.distanceBlocks())
            : State.away();
        return state;
    }

    public static List<HudRenderCommand> buildCommands(State snapshot, int screenWidth, int screenHeight, long nowMillis) {
        State safeState = snapshot == null ? State.away() : snapshot;
        if (!safeState.insideHome() || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }

        int x = Math.max(8, (screenWidth - PANEL_WIDTH) / 2);
        int y = Math.max(28, screenHeight - PANEL_HEIGHT - 36);
        List<HudRenderCommand> commands = new ArrayList<>();
        commands.add(HudRenderCommand.screenTint(HudRenderLayer.HOME_SEQUENCE, 0x180B1410));
        commands.add(HudRenderCommand.rect(HudRenderLayer.HOME_SEQUENCE, x, y, PANEL_WIDTH, PANEL_HEIGHT, 0xA00F1812));
        commands.add(HudRenderCommand.rect(HudRenderLayer.HOME_SEQUENCE, x, y, PANEL_WIDTH, 1, 0x806F8A66));
        commands.add(HudRenderCommand.text(HudRenderLayer.HOME_SEQUENCE, "回到灵龛", x + 8, y + 7, HOME_TEXT));
        commands.add(HudRenderCommand.text(HudRenderLayer.HOME_SEQUENCE, "灵龛——无灵气", x + 8, y + 20, HOME_DIM));
        commands.add(HudRenderCommand.text(HudRenderLayer.HOME_SEQUENCE, "整理：背包 · 骨币 · 伤势 · 下一趟", x + 8, y + 33, HOME_DIM));
        if (settleAnimationActive(safeState, nowMillis)) {
            commands.add(HudRenderCommand.text(HudRenderLayer.HOME_SEQUENCE, "靠墙坐下，慢慢把气息压稳", x + 8, y + 46, 0xFFD7E8C8));
        } else if (hasActiveNewBadge(nowMillis)) {
            commands.add(HudRenderCommand.text(HudRenderLayer.HOME_SEQUENCE, "背包里有 NEW 收获", x + 8, y + 46, 0xFFFFE08A));
        }
        return commands;
    }

    public static boolean newBadgeActive(InventoryItem item, long nowMillis) {
        if (item == null || item.isEmpty()) {
            return false;
        }
        Long until = newItemUntilMs.get(itemKey(item));
        if (until == null) {
            return false;
        }
        if (until <= Math.max(0L, nowMillis)) {
            newItemUntilMs.remove(itemKey(item));
            return false;
        }
        return true;
    }

    static void setAudioBridgeForTests(AudioPlaybackBridge bridge) {
        audioBridge = bridge == null ? SoundRecipePlayer.instance() : bridge;
    }

    static void resetAudioBridgeForTests() {
        audioBridge = SoundRecipePlayer.instance();
    }

    public static void resetForTests() {
        state = State.away();
        knownInventoryKeys = new HashSet<>();
        newItemUntilMs.clear();
        resetAudioBridgeForTests();
    }

    static boolean settleAnimationActive(State snapshot, long nowMillis) {
        State safeState = snapshot == null ? State.away() : snapshot;
        return safeState.insideHome()
            && Math.max(0L, nowMillis) - safeState.enteredAtMs() < SETTLE_ANIMATION_MS;
    }

    static boolean hasActiveNewBadge(long nowMillis) {
        long now = Math.max(0L, nowMillis);
        newItemUntilMs.entrySet().removeIf(entry -> entry.getValue() <= now);
        return !newItemUntilMs.isEmpty();
    }

    private static HomeMarker nearestHomeMarker(HudRuntimeContext runtime) {
        HomeMarker best = null;
        for (HudRuntimeContext.CompassMarker marker : runtime.compassMarkers()) {
            if (marker.kind() != HudRuntimeContext.CompassMarker.Kind.SPIRIT_NICHE) {
                continue;
            }
            double dx = marker.worldX() - runtime.playerX();
            double dz = marker.worldZ() - runtime.playerZ();
            double distance = Math.sqrt(dx * dx + dz * dz);
            if (!Double.isFinite(distance)) {
                continue;
            }
            if (best == null || distance < best.distanceBlocks()) {
                best = new HomeMarker(distance);
            }
        }
        return best;
    }

    private static void observeInventory(InventoryModel inventory, boolean enteringHome, long nowMillis) {
        InventoryModel safeInventory = inventory == null ? InventoryModel.empty() : inventory;
        Set<String> current = inventoryKeys(safeInventory);
        if (enteringHome) {
            for (String key : current) {
                if (!knownInventoryKeys.contains(key)) {
                    newItemUntilMs.put(key, nowMillis + NEW_BADGE_MS);
                }
            }
        }
        knownInventoryKeys = current;
        newItemUntilMs.keySet().removeIf(key -> !current.contains(key));
    }

    private static Set<String> inventoryKeys(InventoryModel inventory) {
        Set<String> keys = new HashSet<>();
        for (InventoryModel.GridEntry entry : inventory.gridItems()) {
            keys.add(itemKey(entry.item()));
        }
        for (InventoryItem item : inventory.hotbar()) {
            if (item != null && !item.isEmpty()) {
                keys.add(itemKey(item));
            }
        }
        inventory.equipped().values().stream()
            .filter(item -> item != null && !item.isEmpty())
            .map(HomeSequence::itemKey)
            .forEach(keys::add);
        return keys;
    }

    private static String itemKey(InventoryItem item) {
        long instanceId = item.instanceId();
        if (instanceId > 0L) {
            return "i:" + instanceId;
        }
        return "fallback:" + item.itemId() + ":" + item.displayName() + ":" + item.stackCount();
    }

    private static void playHomeAudio() {
        audioBridge.play(new AudioEventPayload.PlaySoundRecipe(
            HOME_AUDIO_RECIPE_ID,
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            0.72f,
            0.0f,
            homeAudioRecipe()
        ));
    }

    private static AudioRecipe homeAudioRecipe() {
        return new AudioRecipe(
            HOME_AUDIO_RECIPE_ID,
            List.of(
                new AudioLayer(new Identifier("minecraft", "block.campfire.crackle"), 0.35f, 0.85f, 0),
                new AudioLayer(new Identifier("minecraft", "block.amethyst_block.chime"), 0.22f, 0.72f, 3)
            ),
            Optional.empty(),
            52,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.PLAYERS,
            AudioBus.ENVIRONMENT
        );
    }

    public record State(boolean insideHome, boolean enteredThisTick, long enteredAtMs, double distanceBlocks) {
        public State {
            enteredAtMs = Math.max(0L, enteredAtMs);
            if (!Double.isFinite(distanceBlocks)) {
                distanceBlocks = Double.POSITIVE_INFINITY;
            }
        }

        static State away() {
            return new State(false, false, 0L, Double.POSITIVE_INFINITY);
        }
    }

    private record HomeMarker(double distanceBlocks) {
    }
}
