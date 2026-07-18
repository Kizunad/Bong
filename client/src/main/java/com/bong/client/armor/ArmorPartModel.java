package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import net.minecraft.client.model.ModelData;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.model.ModelPartBuilder;
import net.minecraft.client.model.ModelPartData;
import net.minecraft.client.model.ModelTransform;
import net.minecraft.client.model.TexturedModelData;

import java.util.EnumMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * plan-armor-model-render-v1 P0 — Bong 护甲的 vanilla {@link ModelPart} 烘焙底盘。
 *
 * <p>运行时唯一模型事实来源是 {@link #CUBE_TABLES}。Blockbench 文件只用于离线设计和预览；生成器
 * 定稿后须把同一组 cube 数值转写到本表，并由 {@code ArmorPartModelTest} 逐 key 锁住。这样实体
 * {@code FeatureRenderer} 不依赖 SML/OBJ、GeckoLib cache 或真实 vanilla {@code ItemStack}。
 *
 * <p>cube 的 x/z 已是目标玩家骨骼的局部坐标；y 保留 Bedrock 的“脚在 0、向上为正”坐标。烘焙时按
 * 槽位使用独立骨骼 pivot：头/躯干 pivot 在 Bedrock y=24，腿/脚 pivot 在 y=12。转换公式固定为
 * {@code vanilla_y_min = pivot_y - (bedrock_origin_y + size_y)}。
 */
public final class ArmorPartModel {
    public static final int TEXTURE_WIDTH = 64;
    public static final int TEXTURE_HEIGHT = 64;

    public static final float HEAD_BONE_PIVOT_Y = 24.0f;
    public static final float CHEST_BONE_PIVOT_Y = 24.0f;
    public static final float LEGS_BONE_PIVOT_Y = 12.0f;
    public static final float FEET_BONE_PIVOT_Y = 12.0f;

    public enum Mount {
        HEAD("head"),
        BODY("body"),
        LEFT_LEG("left_leg"),
        RIGHT_LEG("right_leg"),
        LEFT_FOOT("left_foot"),
        RIGHT_FOOT("right_foot");

        private final String childName;

        Mount(String childName) {
            this.childName = childName;
        }

        public String childName() {
            return childName;
        }
    }

    public record ArmorCube(
        Mount mount,
        float ox,
        float oy,
        float oz,
        float sx,
        float sy,
        float sz,
        int u,
        int v
    ) {}

    private static final Map<String, List<ArmorCube>> CUBE_TABLES = placeholderCubeTables();

    private ArmorPartModel() {}

    public static boolean supports(String modelKey) {
        return modelKey != null && CUBE_TABLES.containsKey(modelKey.trim());
    }

    public static Set<String> modelKeys() {
        return CUBE_TABLES.keySet();
    }

    static List<ArmorCube> cubes(String modelKey) {
        List<ArmorCube> cubes = CUBE_TABLES.get(modelKey);
        if (cubes == null) {
            throw new IllegalArgumentException("unknown armor model key: " + modelKey);
        }
        return cubes;
    }

    public static List<Mount> mountsForSlot(EquipSlotType slot) {
        if (slot == null) {
            return List.of();
        }
        return switch (slot) {
            case HEAD -> List.of(Mount.HEAD);
            case CHEST -> List.of(Mount.BODY);
            case LEGS -> List.of(Mount.LEFT_LEG, Mount.RIGHT_LEG);
            case FEET -> List.of(Mount.LEFT_FOOT, Mount.RIGHT_FOOT);
            default -> List.of();
        };
    }

    public static float bedrockToVanillaCuboidY(
        EquipSlotType slot,
        float bedrockOriginY,
        float sizeY
    ) {
        return bonePivotY(slot) - (bedrockOriginY + sizeY);
    }

    public static ModelPart buildModelPart(String modelKey) {
        List<ArmorCube> cubes = cubes(modelKey);
        ModelData modelData = new ModelData();
        ModelPartData root = modelData.getRoot();
        Map<Mount, ModelPartBuilder> builders = new EnumMap<>(Mount.class);

        for (ArmorCube cube : cubes) {
            EquipSlotType slot = slotForMount(cube.mount());
            float vanillaY = bedrockToVanillaCuboidY(slot, cube.oy(), cube.sy());
            ModelPartBuilder builder = builders.computeIfAbsent(cube.mount(), ignored -> ModelPartBuilder.create());
            builder.uv(cube.u(), cube.v())
                .cuboid(cube.ox(), vanillaY, cube.oz(), cube.sx(), cube.sy(), cube.sz());
        }

        for (Map.Entry<Mount, ModelPartBuilder> entry : builders.entrySet()) {
            root.addChild(entry.getKey().childName(), entry.getValue(), ModelTransform.NONE);
        }
        return TexturedModelData.of(modelData, TEXTURE_WIDTH, TEXTURE_HEIGHT).createModel();
    }

    private static float bonePivotY(EquipSlotType slot) {
        if (slot == null) {
            throw new IllegalArgumentException("armor slot must not be null");
        }
        return switch (slot) {
            case HEAD -> HEAD_BONE_PIVOT_Y;
            case CHEST -> CHEST_BONE_PIVOT_Y;
            case LEGS -> LEGS_BONE_PIVOT_Y;
            case FEET -> FEET_BONE_PIVOT_Y;
            default -> throw new IllegalArgumentException("not an armor slot: " + slot);
        };
    }

    private static EquipSlotType slotForMount(Mount mount) {
        return switch (mount) {
            case HEAD -> EquipSlotType.HEAD;
            case BODY -> EquipSlotType.CHEST;
            case LEFT_LEG, RIGHT_LEG -> EquipSlotType.LEGS;
            case LEFT_FOOT, RIGHT_FOOT -> EquipSlotType.FEET;
        };
    }

    private static Map<String, List<ArmorCube>> placeholderCubeTables() {
        Map<String, List<ArmorCube>> tables = new LinkedHashMap<>();
        tables.put("iron_helmet", List.of(
            new ArmorCube(Mount.HEAD, -4.4f, 23.8f, -4.4f, 8.8f, 8.6f, 8.8f, 0, 0)
        ));
        tables.put("iron_chestplate", List.of(
            new ArmorCube(Mount.BODY, -4.4f, 11.8f, -2.5f, 8.8f, 12.4f, 5.0f, 0, 0)
        ));
        tables.put("iron_leggings", symmetricLegPlaceholders(0));
        tables.put("iron_boots", symmetricFootPlaceholders(0));

        tables.put("bone_helmet", List.of(
            new ArmorCube(Mount.HEAD, -4.5f, 23.7f, -4.5f, 9.0f, 8.8f, 9.0f, 0, 0)
        ));
        tables.put("bone_chestplate", List.of(
            new ArmorCube(Mount.BODY, -4.5f, 11.7f, -2.6f, 9.0f, 12.6f, 5.2f, 0, 0)
        ));
        tables.put("bone_leggings", symmetricLegPlaceholders(16));
        tables.put("bone_boots", symmetricFootPlaceholders(16));
        return Map.copyOf(tables);
    }

    private static List<ArmorCube> symmetricLegPlaceholders(int textureV) {
        return List.of(
            new ArmorCube(Mount.LEFT_LEG, -2.2f, -0.2f, -2.2f, 4.4f, 12.4f, 4.4f, 0, textureV),
            new ArmorCube(Mount.RIGHT_LEG, -2.2f, -0.2f, -2.2f, 4.4f, 12.4f, 4.4f, 0, textureV)
        );
    }

    private static List<ArmorCube> symmetricFootPlaceholders(int textureV) {
        return List.of(
            new ArmorCube(Mount.LEFT_FOOT, -2.3f, -0.2f, -2.3f, 4.6f, 4.4f, 4.6f, 0, textureV),
            new ArmorCube(Mount.RIGHT_FOOT, -2.3f, -0.2f, -2.3f, 4.6f, 4.4f, 4.6f, 0, textureV)
        );
    }
}
