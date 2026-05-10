package com.bong.client.combat.store;

import java.util.ArrayList;
import java.util.List;

/** Client-side HUD state for tuike false-skin layers. */
public final class FalseSkinHudStateStore {
    public record Layer(
        String tier,
        float spiritQuality,
        float damageCapacity,
        float contamLoad,
        float permanentTaintLoad
    ) {
        public Layer {
            tier = sanitizeTier(tier);
            spiritQuality = clamp(spiritQuality, 0f, 10f);
            damageCapacity = Math.max(0f, finiteOrZero(damageCapacity));
            contamLoad = clamp(contamLoad, 0f, 100f);
            permanentTaintLoad = Math.max(0f, finiteOrZero(permanentTaintLoad));
        }
    }

    public record State(
        String targetId,
        String kind,
        int layersRemaining,
        float contamCapacityPerLayer,
        float absorbedContam,
        long equippedAtTick,
        List<Layer> layers
    ) {
        public static final State NONE = new State("", "", 0, 0f, 0f, 0L, List.of());

        public State {
            targetId = targetId == null ? "" : targetId;
            kind = kind == null ? "" : kind;
            layersRemaining = Math.max(0, Math.min(3, layersRemaining));
            contamCapacityPerLayer = Math.max(0f, finiteOrZero(contamCapacityPerLayer));
            absorbedContam = Math.max(0f, finiteOrZero(absorbedContam));
            equippedAtTick = Math.max(0L, equippedAtTick);
            layers = normalizeLayers(kind, layersRemaining, contamCapacityPerLayer, absorbedContam, layers);
            layersRemaining = layers.isEmpty() ? layersRemaining : Math.min(3, layers.size());
        }

        public boolean active() {
            return layersRemaining > 0;
        }

        public float contamRatio() {
            if (contamCapacityPerLayer <= 0f) return 0f;
            return clamp(absorbedContam / contamCapacityPerLayer, 0f, 1f);
        }
    }

    private static volatile State snapshot = State.NONE;

    private FalseSkinHudStateStore() {
    }

    public static State snapshot() {
        return snapshot;
    }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }

    private static List<Layer> normalizeLayers(
        String kind,
        int layersRemaining,
        float contamCapacityPerLayer,
        float absorbedContam,
        List<Layer> rawLayers
    ) {
        if (rawLayers != null && !rawLayers.isEmpty()) {
            return List.copyOf(rawLayers.subList(0, Math.min(3, rawLayers.size())));
        }

        if (layersRemaining <= 0) {
            return List.of();
        }

        List<Layer> synthesized = new ArrayList<>(layersRemaining);
        String tier = tierForLegacyKind(kind);
        for (int i = 0; i < layersRemaining; i++) {
            float layerContam = i == layersRemaining - 1 ? absorbedContam : 0f;
            synthesized.add(new Layer(tier, 1f, contamCapacityPerLayer, layerContam, 0f));
        }
        return List.copyOf(synthesized);
    }

    private static String tierForLegacyKind(String kind) {
        return "rotten_wood_armor".equals(kind) ? "mid" : "fan";
    }

    private static String sanitizeTier(String tier) {
        if (tier == null || tier.isBlank()) return "fan";
        return switch (tier) {
            case "fan", "light", "mid", "heavy", "ancient" -> tier;
            default -> "fan";
        };
    }

    private static float clamp(float value, float min, float max) {
        return Math.max(min, Math.min(max, finiteOrZero(value)));
    }

    private static float finiteOrZero(float value) {
        return Float.isFinite(value) ? value : 0f;
    }
}
