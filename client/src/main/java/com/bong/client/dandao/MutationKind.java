package com.bong.client.dandao;

import net.minecraft.util.Identifier;

import java.util.Locale;

/**
 * plan-dandao-path-v1 P3 -- Client-side enum of all mutation kinds.
 *
 * <p>Maps each server {@code MutationKind} variant to its GeckoLib
 * geo model, texture, and body slot. The string {@code kind} field in
 * {@link MutationVisualState.MutationSlotEntry} is matched case-insensitively
 * against {@link #name()}.
 */
public enum MutationKind {
    // Stage 1 -- subtle
    GOLDEN_IRIS("dandao_golden_iris", "dandao_iris", "Head"),
    HARDENED_NAILS("dandao_hardened_nails", "dandao_nails", "Forearm"),
    TOUGH_SKIN("dandao_forearm_scales", "dandao_scales", "Forearm"),

    // Stage 2 -- visible
    BONE_RIDGE("dandao_bone_ridge", "dandao_ridge", "Head"),
    FOREARM_SCALES("dandao_forearm_scales", "dandao_scales", "Forearm"),
    SPINE_SPURS("dandao_spine_spurs", "dandao_spurs", "Back"),

    // Stage 3 -- heavy
    HORNS("dandao_horns", "dandao_horns", "Head"),
    TAIL("dandao_tail", "dandao_tail", "Lower"),
    BACK_CARAPACE("dandao_carapace", "dandao_carapace", "Back"),

    // Stage 4 -- bestial
    EXTRA_ARMS("dandao_extra_arms", "dandao_arms", "Torso"),
    BODY_ENLARGE("dandao_beast_face", "dandao_beast", "Torso"),
    BEAST_FACE("dandao_beast_face", "dandao_beast", "Head");

    private final String geoPath;
    private final String textureName;
    private final String defaultBodySlot;

    MutationKind(String geoPath, String textureName, String defaultBodySlot) {
        this.geoPath = geoPath;
        this.textureName = textureName;
        this.defaultBodySlot = defaultBodySlot;
    }

    public Identifier geoId() {
        return new Identifier("bong", "geo/" + geoPath + ".geo.json");
    }

    public Identifier textureId() {
        return new Identifier("bong", "textures/entity/mutation/" + textureName + ".png");
    }

    public String defaultBodySlot() {
        return defaultBodySlot;
    }

    /**
     * Resolves server-side kind string (e.g. "GoldenIris") to the enum constant.
     * Returns {@code null} if unrecognized.
     */
    public static MutationKind fromServerName(String serverName) {
        if (serverName == null || serverName.isBlank()) {
            return null;
        }
        // Convert CamelCase to UPPER_SNAKE: "GoldenIris" -> "GOLDEN_IRIS"
        String snake = serverName
            .replaceAll("([a-z])([A-Z])", "$1_$2")
            .toUpperCase(Locale.ROOT);
        try {
            return MutationKind.valueOf(snake);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }
}
