package com.bong.client.entity;

import net.minecraft.entity.EntityDimensions;
import net.minecraft.util.Identifier;

import java.util.Arrays;
import java.util.List;
import java.util.Locale;

public enum BongEntityModelKind {
    SPIRIT_NICHE(
        "spirit_niche",
        "SpiritNiche.bbmodel",
        142,
        0.9f,
        1.5f,
        64,
        10,
        0.35f,
        "inactive",
        "active",
        "invaded"
    ),
    SPIRIT_EYE(
        "spirit_eye",
        "SpiritEye.bbmodel",
        143,
        1.0f,
        2.0f,
        96,
        5,
        0.0f,
        "qi_050",
        "qi_070",
        "qi_100"
    ),
    RIFT_PORTAL(
        "rift_portal",
        "RiftPortal.bbmodel",
        144,
        2.0f,
        3.0f,
        128,
        3,
        0.0f,
        "main_rift",
        "deep_rift",
        "collapse_tear"
    ),
    FORGE_STATION(
        "forge_station",
        "ForgeStation.bbmodel",
        145,
        1.0f,
        1.0f,
        64,
        10,
        0.35f,
        "idle",
        "working"
    ),
    ALCHEMY_FURNACE(
        "alchemy_furnace",
        "AlchemyFurnace.bbmodel",
        146,
        1.0f,
        1.5f,
        64,
        10,
        0.35f,
        "idle",
        "brewing"
    ),
    FORMATION_CORE(
        "formation_core",
        "FormationCore.bbmodel",
        147,
        1.0f,
        0.35f,
        64,
        10,
        0.15f,
        "inactive",
        "active",
        "exhausted"
    ),
    LINGTIAN_PLOT(
        "lingtian_plot",
        "LingtianPlot.bbmodel",
        148,
        0.9f,
        0.2f,
        64,
        10,
        0.05f,
        "wild",
        "tilled",
        "planted",
        "mature"
    ),
    DRY_CORPSE(
        "dry_corpse",
        "DryCorpse.bbmodel",
        149,
        1.0f,
        0.45f,
        64,
        10,
        0.12f,
        "intact",
        "searching",
        "looted"
    ),
    BONE_SKELETON(
        "bone_skeleton",
        "BoneSkeleton.bbmodel",
        150,
        1.0f,
        0.35f,
        64,
        10,
        0.12f,
        "intact",
        "searching",
        "looted"
    ),
    STORAGE_POUCH(
        "storage_pouch",
        "StoragePouch.bbmodel",
        151,
        0.55f,
        0.45f,
        64,
        10,
        0.12f,
        "intact",
        "searching",
        "looted"
    ),
    STONE_CASKET(
        "stone_casket",
        "StoneCasket.bbmodel",
        152,
        1.2f,
        0.65f,
        64,
        10,
        0.18f,
        "intact",
        "searching",
        "looted"
    );

    private static final String MOD_ID = "bong";

    private final String entityId;
    private final String blockbenchFileName;
    private final int expectedRawId;
    private final EntityDimensions dimensions;
    private final int trackingRange;
    private final int trackingTickInterval;
    private final float shadowRadius;
    private final List<String> textureStates;

    BongEntityModelKind(
        String entityId,
        String blockbenchFileName,
        int expectedRawId,
        float width,
        float height,
        int trackingRange,
        int trackingTickInterval,
        float shadowRadius,
        String... textureStates
    ) {
        if (textureStates == null || textureStates.length == 0) {
            throw new IllegalArgumentException("textureStates must not be empty for " + entityId);
        }
        for (String textureState : textureStates) {
            if (textureState == null || textureState.isBlank()) {
                throw new IllegalArgumentException("textureStates must not contain blank values for " + entityId);
            }
        }
        this.entityId = entityId;
        this.blockbenchFileName = blockbenchFileName;
        this.expectedRawId = expectedRawId;
        this.dimensions = EntityDimensions.fixed(width, height);
        this.trackingRange = trackingRange;
        this.trackingTickInterval = trackingTickInterval;
        this.shadowRadius = shadowRadius;
        this.textureStates = List.copyOf(Arrays.asList(textureStates));
    }

    public String entityId() {
        return entityId;
    }

    public Identifier identifier() {
        return new Identifier(MOD_ID, entityId);
    }

    public int expectedRawId() {
        return expectedRawId;
    }

    public EntityDimensions dimensions() {
        return dimensions;
    }

    public int trackingRange() {
        return trackingRange;
    }

    public int trackingTickInterval() {
        return trackingTickInterval;
    }

    public float shadowRadius() {
        return shadowRadius;
    }

    public Identifier modelResource() {
        return new Identifier(MOD_ID, "geo/" + entityId + ".geo.json");
    }

    public Identifier animationResource() {
        return new Identifier(MOD_ID, "animations/" + entityId + ".animation.json");
    }

    public String idleAnimationName() {
        return "animation.bong." + entityId + ".idle";
    }

    public Identifier textureForState(int visualState) {
        String suffix = textureStates.get(normalizeVisualState(visualState));
        return new Identifier(MOD_ID, "textures/entity/" + entityId + "_" + suffix + ".png");
    }

    public int normalizeVisualState(int visualState) {
        if (visualState < 0) {
            return 0;
        }
        int maxState = textureStates.size() - 1;
        return Math.min(visualState, maxState);
    }

    public int stateCount() {
        return textureStates.size();
    }

    public List<String> textureStates() {
        return textureStates;
    }

    public String blockbenchFileName() {
        return blockbenchFileName;
    }

    public String displayLabel() {
        return Arrays.stream(entityId.split("_"))
            .filter(part -> !part.isBlank())
            .map(part -> part.substring(0, 1).toUpperCase(Locale.ROOT) + part.substring(1))
            .reduce("", String::concat);
    }
}
