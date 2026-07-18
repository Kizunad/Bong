package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import net.minecraft.util.Identifier;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * plan-armor-model-render-v1 P0: template_id -> ModelPart cube table / texture registry.
 *
 * <p>When an armor template_id is present here, {@code MixinPlayerEntityArmor} skips the
 * leather dye fallback once {@link ArmorFeatureRenderer} is ready and lets its ModelPart path render instead.
 */
public final class ArmorModelRegistry {
    public record ArmorModelSpec(
        String templateId,
        EquipSlotType slot,
        String modelKey,
        String texturePath
    ) {
        public Identifier textureId() {
            return new Identifier(texturePath);
        }
    }

    private static final Map<String, ArmorModelSpec> REGISTRY = new LinkedHashMap<>();

    static {
        register("armor_iron_helmet", EquipSlotType.HEAD, "iron_helmet");
        register("armor_iron_chestplate", EquipSlotType.CHEST, "iron_chestplate");
        register("armor_iron_leggings", EquipSlotType.LEGS, "iron_leggings");
        register("armor_iron_boots", EquipSlotType.FEET, "iron_boots");

        register("armor_bone_helmet", EquipSlotType.HEAD, "bone_helmet");
        register("armor_bone_chestplate", EquipSlotType.CHEST, "bone_chestplate");
        register("armor_bone_leggings", EquipSlotType.LEGS, "bone_leggings");
        register("armor_bone_boots", EquipSlotType.FEET, "bone_boots");
    }

    private static void register(String templateId, EquipSlotType slot, String modelKey) {
        REGISTRY.put(templateId, new ArmorModelSpec(
            templateId,
            slot,
            modelKey,
            "bong:textures/armor/" + modelKey + "/0.png"
        ));
    }

    public static Optional<ArmorModelSpec> get(String templateId) {
        if (templateId == null) return Optional.empty();
        return Optional.ofNullable(REGISTRY.get(templateId.trim()));
    }

    public static List<ArmorModelSpec> all() {
        return List.copyOf(REGISTRY.values());
    }

    public static int size() {
        return REGISTRY.size();
    }

    private ArmorModelRegistry() {}
}
