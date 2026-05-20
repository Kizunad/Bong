package com.bong.client.armor;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.stream.Collectors;

/**
 * plan-depth-loop-v1 P1: template_id -> OBJ model / texture path registry for custom armor rendering.
 *
 * <p>When an armor template_id is present here, {@code MixinPlayerEntityArmor} skips the
 * leather dye fallback and lets {@link ArmorFeatureRenderer} handle the OBJ render instead.
 */
public final class ArmorModelRegistry {
    public record ArmorModelSpec(
        String templateId,
        String slot,
        String modelPath,
        String texturePath
    ) {}

    private static final Map<String, ArmorModelSpec> REGISTRY = new LinkedHashMap<>();
    private static final Set<String> MODEL_PATHS;

    static {
        register("armor_iron_helmet", "head", "iron_helmet");
        register("armor_iron_chestplate", "chest", "iron_chestplate");
        register("armor_iron_leggings", "legs", "iron_leggings");
        register("armor_iron_boots", "feet", "iron_boots");

        MODEL_PATHS = REGISTRY.values().stream()
            .map(ArmorModelSpec::modelPath)
            .collect(Collectors.toUnmodifiableSet());
    }

    private static void register(String templateId, String slot, String modelDir) {
        REGISTRY.put(templateId, new ArmorModelSpec(
            templateId,
            slot,
            "bong:models/armor/" + modelDir + "/" + modelDir + ".obj",
            "bong:textures/armor/" + modelDir + "/0.png"
        ));
    }

    public static Optional<ArmorModelSpec> get(String templateId) {
        if (templateId == null) return Optional.empty();
        return Optional.ofNullable(REGISTRY.get(templateId.trim()));
    }

    public static Set<String> modelPaths() {
        return MODEL_PATHS;
    }

    public static int size() {
        return REGISTRY.size();
    }

    private ArmorModelRegistry() {}
}
