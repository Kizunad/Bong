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
        135,
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
        136,
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
        137,
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
        138,
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
        139,
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
        140,
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
        141,
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
        142,
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
        143,
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
        144,
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
        145,
        1.2f,
        0.65f,
        64,
        10,
        0.18f,
        "intact",
        "searching",
        "looted"
    ),
    // plan-supply-coffin-v1：三档物资棺。当前仅 intact 单 state；P2 视觉 polish
    // PR 接入 opening 裂纹叠层贴图后再扩 state count。raw_id 146-148 紧跟
    // STONE_CASKET(145) 之后，与 server 端 entity_model.rs COFFIN_*_ENTITY_KIND
    // 1:1 对应（plan-supply-coffin-v1 P1.1）。
    COFFIN_COMMON(
        "coffin_common",
        "CoffinCommon.bbmodel",
        146,
        1.0f,
        0.6f,
        64,
        10,
        0.15f,
        "intact"
    ),
    COFFIN_RARE(
        "coffin_rare",
        "CoffinRare.bbmodel",
        147,
        1.0f,
        0.7f,
        64,
        10,
        0.18f,
        "intact"
    ),
    COFFIN_PRECIOUS(
        "coffin_precious",
        "CoffinPrecious.bbmodel",
        148,
        1.2f,
        0.8f,
        64,
        10,
        0.22f,
        "intact"
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
