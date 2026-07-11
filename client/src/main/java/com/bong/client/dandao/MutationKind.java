package com.bong.client.dandao;

import net.minecraft.util.Identifier;

import java.util.Locale;

/**
 * plan-dandao-path-v1 P3 -- Client-side enum of all mutation kinds.
 *
 * <p>Maps each server {@code MutationKind} variant to its GeckoLib geo model and
 * texture. The string {@code kind} field in
 * {@link MutationVisualState.MutationSlotEntry} is matched case-insensitively
 * against {@link #name()}.
 *
 * <p><b>plan-race-system-v1 P0 review r3 (major x3 收口)</b> -- this enum previously
 * also carried a private hardcoded {@code defaultBodySlot} table duplicating the
 * server's {@code BodySlot} assignment (and it was unused dead weight: no render
 * code ever read it, since the real per-instance {@code body_slot} always arrives
 * dynamically over {@code bong:mutation_visual} in
 * {@link MutationVisualState.MutationSlotEntry#bodySlot()}). It has been removed;
 * the single source of truth for slot -&gt; body part positioning is now
 * {@link MutationSlotLayoutRegistry} (backed by the shared
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} resource, pinned
 * against the server's {@code humanoid.json mutation_slot_mapping}).
 */
public enum MutationKind {
    // Stage 1 -- subtle
    GOLDEN_IRIS("dandao_golden_iris", "dandao_iris"),
    HARDENED_NAILS("dandao_hardened_nails", "dandao_nails"),
    TOUGH_SKIN("dandao_forearm_scales", "dandao_scales"),

    // Stage 2 -- visible
    BONE_RIDGE("dandao_bone_ridge", "dandao_ridge"),
    FOREARM_SCALES("dandao_forearm_scales", "dandao_scales"),
    SPINE_SPURS("dandao_spine_spurs", "dandao_spurs"),

    // Stage 3 -- heavy
    HORNS("dandao_horns", "dandao_horns"),
    TAIL("dandao_tail", "dandao_tail"),
    BACK_CARAPACE("dandao_carapace", "dandao_carapace"),

    // Stage 4 -- bestial
    EXTRA_ARMS("dandao_extra_arms", "dandao_arms"),
    BODY_ENLARGE("dandao_beast_face", "dandao_beast"),
    BEAST_FACE("dandao_beast_face", "dandao_beast");

    private final String geoPath;
    private final String textureName;

    MutationKind(String geoPath, String textureName) {
        this.geoPath = geoPath;
        this.textureName = textureName;
    }

    public Identifier geoId() {
        return new Identifier("bong", "geo/" + geoPath + ".geo.json");
    }

    public Identifier textureId() {
        return new Identifier("bong", "textures/entity/mutation/" + textureName + ".png");
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
