package com.bong.client.social;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.AudioPlaybackBridge;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicLong;

public final class NicheIntrusionAlertHandler {
    private static final int WARNING_COLOR = 0xFFFFAA55;
    private static final int SOCIAL_COLOR = 0xFFA0C0FF;
    private static final String FATIGUE_RECIPE_ID = "niche_guardian_fatigue";
    private static final String BROKEN_RECIPE_ID = "niche_guardian_broken";
    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(10_000L);
    private static AudioPlaybackBridge audioBridge = SoundRecipePlayer.instance();

    private NicheIntrusionAlertHandler() {
    }

    static void setAudioBridgeForTests(AudioPlaybackBridge bridge) {
        audioBridge = bridge == null ? SoundRecipePlayer.instance() : bridge;
    }

    static void resetAudioBridgeForTests() {
        audioBridge = SoundRecipePlayer.instance();
    }

    public static void recordIntrusion(String intruderId, List<Long> itemsTaken, double taintDelta) {
        NicheGuardianStore.recordIntrusion(new NicheGuardianStore.NicheIntrusionAlert(
            itemsTaken,
            intruderId,
            taintDelta,
            System.currentTimeMillis()
        ));
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            UnifiedEvent.Priority.P1_IMPORTANT,
            "niche_intrusion:" + fallback(intruderId),
            "灵龛入侵：" + fallback(intruderId) + " 取走 " + (itemsTaken == null ? 0 : itemsTaken.size()) + " 件",
            WARNING_COLOR,
            System.currentTimeMillis()
        );
    }

    public static void recordGuardianFatigue(String guardianKind, int chargesRemaining) {
        NicheGuardianStore.recordFatigue(guardianKind, chargesRemaining);
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            UnifiedEvent.Priority.P2_NORMAL,
            "niche_guardian_fatigue:" + fallback(guardianKind),
            "守家载体损耗：" + fallback(guardianKind) + " 剩余 " + Math.max(0, chargesRemaining) + " 次",
            SOCIAL_COLOR,
            System.currentTimeMillis()
        );
        playGuardianReactionAudio(false);
    }

    public static void recordGuardianBroken(String guardianKind, String intruderId) {
        NicheGuardianStore.recordBroken(guardianKind, intruderId);
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            UnifiedEvent.Priority.P1_IMPORTANT,
            "niche_guardian_broken:" + fallback(guardianKind),
            "守家载体破损：" + fallback(guardianKind),
            WARNING_COLOR,
            System.currentTimeMillis()
        );
        playGuardianReactionAudio(true);
    }

    private static void playGuardianReactionAudio(boolean broken) {
        String recipeId = broken ? BROKEN_RECIPE_ID : FATIGUE_RECIPE_ID;
        audioBridge.play(new AudioEventPayload.PlaySoundRecipe(
            recipeId,
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            guardianReactionRecipe(recipeId, broken)
        ));
    }

    private static AudioRecipe guardianReactionRecipe(String recipeId, boolean broken) {
        List<AudioLayer> layers = broken
            ? List.of(
                new AudioLayer(new Identifier("minecraft", "block.stone.break"), 0.7f, 0.7f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.wither.shoot"), 0.3f, 1.4f, 2)
            )
            : List.of(new AudioLayer(
                new Identifier("minecraft", "block.grindstone.use"),
                0.5f,
                0.8f,
                0
            ));
        return new AudioRecipe(
            recipeId,
            layers,
            Optional.empty(),
            broken ? 78 : 72,
            AudioAttenuation.WORLD_3D,
            AudioCategory.BLOCKS,
            AudioBus.ENVIRONMENT
        );
    }

    private static String fallback(String value) {
        return value == null || value.isBlank() ? "unknown" : value.trim();
    }
}
