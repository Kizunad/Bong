package com.bong.client.network;

import java.util.Objects;

/**
 * Pure data describing the client-side feedback for a mineral probe result.
 *
 * <p>The network handler creates this value without reading Minecraft client
 * state.  {@code BongNetworkHandler.applyDispatch(...)} is the only production
 * boundary that turns it into HUD and sound side effects.</p>
 */
public record MineralProbeFeedbackSpec(
    String actionbarText,
    int actionbarColor,
    SoundEffect soundEffect,
    float volume,
    float pitch
) {
    public MineralProbeFeedbackSpec {
        actionbarText = Objects.requireNonNull(actionbarText, "actionbarText");
        soundEffect = Objects.requireNonNull(soundEffect, "soundEffect");
    }

    public enum SoundEffect {
        AMETHYST_CHIME,
        NOTE_BLOCK_BASS
    }
}
