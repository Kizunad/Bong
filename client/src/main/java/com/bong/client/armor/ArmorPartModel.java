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

    private static final Map<String, List<ArmorCube>> CUBE_TABLES = cubeTables();

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

    private static Map<String, List<ArmorCube>> cubeTables() {
        Map<String, List<ArmorCube>> tables = new LinkedHashMap<>();
        tables.put("iron_helmet", ironHelmet());
        tables.put("iron_chestplate", ironChestplate());
        tables.put("iron_leggings", ironLeggings());
        tables.put("iron_boots", ironBoots());

        tables.put("bone_helmet", boneHelmet());
        tables.put("bone_chestplate", boneChestplate());
        tables.put("bone_leggings", boneLeggings());
        tables.put("bone_boots", boneBoots());
        return Map.copyOf(tables);
    }

    private static List<ArmorCube> ironHelmet() {
        return List.of(
            new ArmorCube(Mount.HEAD, -4.4f, 31.4f, -4.4f, 4.1f, 1.0f, 8.8f, 0, 0),
            new ArmorCube(Mount.HEAD, 0.2f, 31.4f, -4.4f, 4.2f, 1.0f, 7.8f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.4f, 29.1f, -4.7f, 8.8f, 2.3f, 0.9f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.4f, 24.4f, 3.8f, 8.8f, 7.0f, 0.9f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.7f, 25.0f, -3.7f, 0.9f, 6.4f, 7.5f, 0, 0),
            new ArmorCube(Mount.HEAD, 3.8f, 25.0f, -3.7f, 0.9f, 6.4f, 7.5f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.75f, 24.3f, -4.3f, 1.2f, 3.8f, 1.6f, 32, 0),
            new ArmorCube(Mount.HEAD, 3.55f, 24.3f, -4.3f, 1.2f, 3.8f, 1.6f, 32, 0),
            new ArmorCube(Mount.HEAD, -0.4f, 24.7f, -4.95f, 0.8f, 4.0f, 0.6f, 48, 0),
            new ArmorCube(Mount.HEAD, -0.45f, 31.9f, -3.8f, 0.9f, 1.1f, 7.6f, 32, 0),
            new ArmorCube(Mount.HEAD, -3.2f, 29.9f, -4.9f, 0.6f, 0.6f, 0.35f, 48, 0),
            new ArmorCube(Mount.HEAD, 2.6f, 29.9f, -4.9f, 0.6f, 0.6f, 0.35f, 48, 0),
            new ArmorCube(Mount.HEAD, -4.9f, 29.5f, -0.3f, 0.35f, 0.6f, 0.6f, 48, 0),
            new ArmorCube(Mount.HEAD, 4.55f, 29.5f, -0.3f, 0.35f, 0.6f, 0.6f, 48, 0),
            new ArmorCube(Mount.HEAD, -3.8f, 23.7f, 3.9f, 7.6f, 0.9f, 1.4f, 32, 0)
        );
    }

    private static List<ArmorCube> ironChestplate() {
        return List.of(
            new ArmorCube(Mount.BODY, -4.25f, 18.0f, -2.75f, 4.0f, 6.0f, 0.8f, 0, 0),
            new ArmorCube(Mount.BODY, 0.2f, 18.4f, -2.75f, 4.05f, 5.6f, 0.8f, 0, 0),
            new ArmorCube(Mount.BODY, -3.55f, 13.0f, -2.65f, 7.1f, 5.2f, 0.75f, 0, 0),
            new ArmorCube(Mount.BODY, -4.1f, 18.0f, 1.95f, 8.2f, 6.0f, 0.8f, 0, 0),
            new ArmorCube(Mount.BODY, -3.5f, 13.0f, 1.95f, 7.0f, 5.1f, 0.75f, 0, 0),
            new ArmorCube(Mount.BODY, -4.55f, 14.5f, -1.7f, 0.65f, 8.7f, 3.4f, 0, 0),
            new ArmorCube(Mount.BODY, 3.9f, 14.5f, -1.7f, 0.65f, 8.7f, 3.4f, 0, 0),
            new ArmorCube(Mount.BODY, -4.4f, 22.5f, -3.0f, 3.5f, 1.3f, 1.0f, 32, 0),
            new ArmorCube(Mount.BODY, 0.9f, 22.5f, -3.0f, 3.5f, 1.3f, 1.0f, 32, 0),
            new ArmorCube(Mount.BODY, -0.35f, 13.0f, -3.05f, 0.7f, 9.5f, 0.45f, 32, 0),
            new ArmorCube(Mount.BODY, -3.8f, 12.3f, -2.75f, 7.6f, 1.0f, 5.5f, 32, 0),
            new ArmorCube(Mount.BODY, -5.8f, 21.2f, -2.55f, 1.8f, 2.5f, 5.1f, 0, 0),
            new ArmorCube(Mount.BODY, 4.0f, 21.2f, -2.55f, 1.8f, 2.5f, 5.1f, 0, 0),
            new ArmorCube(Mount.BODY, -6.15f, 22.6f, -2.7f, 2.2f, 0.7f, 5.4f, 32, 0),
            new ArmorCube(Mount.BODY, 3.95f, 22.6f, -2.7f, 2.2f, 0.7f, 5.4f, 32, 0),
            new ArmorCube(Mount.BODY, -3.6f, 18.0f, -3.05f, 0.8f, 4.5f, 0.35f, 32, 0),
            new ArmorCube(Mount.BODY, 2.8f, 18.0f, -3.05f, 0.8f, 4.5f, 0.35f, 32, 0),
            new ArmorCube(Mount.BODY, -3.5f, 21.0f, -3.15f, 0.65f, 0.65f, 0.35f, 48, 0),
            new ArmorCube(Mount.BODY, 2.85f, 21.0f, -3.15f, 0.65f, 0.65f, 0.35f, 48, 0),
            new ArmorCube(Mount.BODY, -2.8f, 14.0f, -3.0f, 0.65f, 0.65f, 0.35f, 48, 0),
            new ArmorCube(Mount.BODY, 2.15f, 14.0f, -3.0f, 0.65f, 0.65f, 0.35f, 48, 0),
            new ArmorCube(Mount.BODY, -0.55f, 18.2f, -3.3f, 1.1f, 1.1f, 0.5f, 48, 0),
            new ArmorCube(Mount.BODY, 2.2f, 15.2f, -2.95f, 1.0f, 2.2f, 0.35f, 32, 0)
        );
    }

    private static List<ArmorCube> ironLeggings() {
        return List.of(
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 7.0f, -2.65f, 4.1f, 5.2f, 0.75f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.85f, 5.1f, -2.6f, 3.7f, 2.1f, 0.7f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.35f, 10.8f, -2.85f, 4.7f, 1.4f, 0.85f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.15f, 3.2f, -2.95f, 4.3f, 1.8f, 1.0f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.65f, 4.9f, -2.0f, 0.65f, 6.6f, 1.2f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 6.2f, 2.0f, 4.1f, 0.65f, 0.45f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -0.35f, 3.7f, -3.2f, 0.7f, 0.7f, 0.35f, 48, 0),
            new ArmorCube(Mount.LEFT_LEG, -0.3f, 11.15f, -3.05f, 0.6f, 0.6f, 0.3f, 48, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.65f, 3.4f, -2.7f, 0.75f, 1.5f, 2.0f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 7.0f, -2.65f, 4.1f, 5.2f, 0.75f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -1.85f, 5.1f, -2.6f, 3.7f, 2.1f, 0.7f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.35f, 10.8f, -2.85f, 4.7f, 1.4f, 0.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.15f, 3.2f, -2.95f, 4.3f, 1.8f, 1.0f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.3f, 4.9f, -2.0f, 0.65f, 6.6f, 1.2f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 6.2f, 2.0f, 4.1f, 0.65f, 0.45f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -0.35f, 3.7f, -3.2f, 0.7f, 0.7f, 0.35f, 48, 0),
            new ArmorCube(Mount.RIGHT_LEG, -0.3f, 11.15f, -3.05f, 0.6f, 0.6f, 0.3f, 48, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.3f, 3.4f, -2.7f, 0.75f, 1.5f, 2.0f, 32, 0)
        );
    }

    private static List<ArmorCube> ironBoots() {
        return List.of(
            new ArmorCube(Mount.LEFT_FOOT, -2.1f, 2.0f, -2.7f, 4.2f, 3.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.3f, 0.0f, -3.1f, 4.6f, 1.8f, 1.2f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, 1.65f, 0.7f, -1.9f, 0.65f, 4.3f, 1.2f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.15f, 4.0f, -2.75f, 4.3f, 0.75f, 5.5f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -0.35f, 0.7f, -3.35f, 0.7f, 0.7f, 0.35f, 48, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.1f, 0.4f, 2.0f, 4.2f, 2.3f, 0.75f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.35f, -0.25f, -3.2f, 4.7f, 0.5f, 5.5f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.1f, 2.0f, -2.7f, 4.2f, 3.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.3f, 0.0f, -3.1f, 4.6f, 1.8f, 1.2f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.3f, 0.7f, -1.9f, 0.65f, 4.3f, 1.2f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.15f, 4.0f, -2.75f, 4.3f, 0.75f, 5.5f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -0.35f, 0.7f, -3.35f, 0.7f, 0.7f, 0.35f, 48, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.1f, 0.4f, 2.0f, 4.2f, 2.3f, 0.75f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.35f, -0.25f, -3.2f, 4.7f, 0.5f, 5.5f, 32, 0)
        );
    }

    private static List<ArmorCube> boneHelmet() {
        return List.of(
            new ArmorCube(Mount.HEAD, -3.7f, 28.7f, -4.75f, 3.2f, 1.0f, 0.75f, 0, 0),
            new ArmorCube(Mount.HEAD, 0.4f, 28.9f, -4.75f, 3.0f, 0.9f, 0.75f, 32, 0),
            new ArmorCube(Mount.HEAD, -1.05f, 29.6f, -4.7f, 2.1f, 2.5f, 0.65f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.45f, 27.1f, -3.5f, 0.85f, 4.9f, 2.65f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.4f, 30.45f, -0.8f, 0.8f, 0.8f, 4.7f, 32, 0),
            new ArmorCube(Mount.HEAD, 3.6f, 27.6f, -3.6f, 0.85f, 4.3f, 2.55f, 32, 0),
            new ArmorCube(Mount.HEAD, 3.65f, 30.7f, -0.95f, 0.75f, 0.75f, 4.85f, 0, 0),
            new ArmorCube(Mount.HEAD, -3.6f, 30.4f, 3.5f, 7.1f, 0.85f, 0.75f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.55f, 24.9f, -4.25f, 1.0f, 3.8f, 1.3f, 32, 0),
            new ArmorCube(Mount.HEAD, 3.55f, 24.5f, -4.25f, 1.0f, 4.2f, 1.3f, 32, 0),
            new ArmorCube(Mount.HEAD, -0.35f, 25.0f, -4.9f, 0.7f, 3.7f, 0.55f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.15f, 23.7f, -4.35f, 0.75f, 2.2f, 0.75f, 32, 0),
            new ArmorCube(Mount.HEAD, -4.05f, 23.15f, -4.25f, 0.55f, 0.8f, 0.55f, 0, 0),
            new ArmorCube(Mount.HEAD, 3.4f, 24.0f, -4.35f, 0.75f, 1.9f, 0.75f, 32, 0),
            new ArmorCube(Mount.HEAD, -3.0f, 31.5f, -1.0f, 1.1f, 1.8f, 1.1f, 0, 0),
            new ArmorCube(Mount.HEAD, -3.35f, 33.0f, -0.85f, 0.85f, 1.6f, 0.85f, 0, 0),
            new ArmorCube(Mount.HEAD, -3.65f, 34.25f, -0.7f, 0.55f, 1.15f, 0.55f, 32, 0),
            new ArmorCube(Mount.HEAD, 2.0f, 31.5f, -1.0f, 1.0f, 1.45f, 1.0f, 0, 0),
            new ArmorCube(Mount.HEAD, 2.25f, 32.65f, -0.85f, 0.65f, 1.0f, 0.65f, 32, 0),
            new ArmorCube(Mount.HEAD, -3.6f, 27.1f, 4.0f, 7.2f, 0.55f, 0.45f, 0, 32),
            new ArmorCube(Mount.HEAD, -4.75f, 27.0f, -0.2f, 0.4f, 0.8f, 0.8f, 0, 32)
        );
    }

    private static List<ArmorCube> boneChestplate() {
        return List.of(
            new ArmorCube(Mount.BODY, -0.7f, 19.0f, -3.0f, 1.4f, 4.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.BODY, -0.55f, 13.0f, -3.0f, 1.1f, 5.7f, 0.75f, 32, 0),
            new ArmorCube(Mount.BODY, -1.0f, 18.2f, -3.25f, 2.0f, 1.3f, 0.45f, 32, 0),
            new ArmorCube(Mount.BODY, -5.3f, 21.4f, -2.7f, 1.8f, 1.8f, 1.0f, 32, 0),
            new ArmorCube(Mount.BODY, 3.5f, 21.8f, -2.7f, 2.0f, 1.6f, 1.0f, 0, 0),
            new ArmorCube(Mount.BODY, -6.0f, 22.2f, -1.3f, 1.2f, 0.9f, 3.2f, 32, 0),
            new ArmorCube(Mount.BODY, 5.1f, 22.0f, -0.8f, 1.0f, 0.8f, 2.6f, 32, 0),
            new ArmorCube(Mount.BODY, -4.1f, 22.8f, -3.0f, 3.2f, 0.7f, 0.75f, 0, 0),
            new ArmorCube(Mount.BODY, 0.9f, 22.6f, -3.0f, 3.5f, 0.75f, 0.75f, 0, 0),
            new ArmorCube(Mount.BODY, -0.6f, 18.6f, 2.05f, 1.2f, 5.2f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, -0.45f, 13.0f, 2.05f, 0.9f, 5.3f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, -4.35f, 21.0f, -3.0f, 3.9f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, -4.05f, 18.8f, -3.0f, 3.6f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, -3.65f, 16.5f, -3.0f, 3.2f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, -3.15f, 14.2f, -3.0f, 2.7f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, 0.45f, 21.2f, -3.0f, 3.8f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, 0.45f, 19.0f, -3.0f, 3.3f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, 0.45f, 16.7f, -3.0f, 2.9f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, 0.45f, 14.5f, -3.0f, 2.1f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, -4.15f, 20.6f, 2.0f, 3.7f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, -3.7f, 17.5f, 2.0f, 3.25f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, -3.25f, 14.4f, 2.0f, 2.8f, 0.65f, 0.7f, 0, 0),
            new ArmorCube(Mount.BODY, 0.45f, 20.4f, 2.0f, 3.5f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, 0.45f, 17.7f, 2.0f, 3.0f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, 0.45f, 14.7f, 2.0f, 2.35f, 0.65f, 0.7f, 32, 0),
            new ArmorCube(Mount.BODY, -3.45f, 18.0f, -3.1f, 0.5f, 4.8f, 0.3f, 0, 32),
            new ArmorCube(Mount.BODY, 2.95f, 18.6f, -3.1f, 0.5f, 3.7f, 0.3f, 0, 32),
            new ArmorCube(Mount.BODY, -4.1f, 12.55f, -2.95f, 8.2f, 0.55f, 0.45f, 0, 32),
            new ArmorCube(Mount.BODY, -4.0f, 12.55f, 2.5f, 8.0f, 0.55f, 0.45f, 0, 32),
            new ArmorCube(Mount.BODY, -4.1f, 12.55f, -2.5f, 0.45f, 0.55f, 5.0f, 0, 32),
            new ArmorCube(Mount.BODY, 3.65f, 12.55f, -2.5f, 0.45f, 0.55f, 5.0f, 0, 32),
            new ArmorCube(Mount.BODY, 2.5f, 14.15f, -3.0f, 1.35f, 0.5f, 0.65f, 32, 0),
            new ArmorCube(Mount.BODY, 2.55f, 13.95f, -3.15f, 0.35f, 1.25f, 0.3f, 0, 32),
            new ArmorCube(Mount.BODY, -0.75f, 21.0f, 2.65f, 1.5f, 1.0f, 0.45f, 32, 0),
            new ArmorCube(Mount.BODY, -0.65f, 17.4f, 2.65f, 1.3f, 0.9f, 0.45f, 0, 0),
            new ArmorCube(Mount.BODY, -0.55f, 14.1f, 2.65f, 1.1f, 0.8f, 0.45f, 32, 0),
            new ArmorCube(Mount.BODY, 3.65f, 12.25f, -3.15f, 0.85f, 1.0f, 0.55f, 0, 32)
        );
    }

    private static List<ArmorCube> boneLeggings() {
        return List.of(
            new ArmorCube(Mount.LEFT_LEG, -0.5f, 5.1f, -2.8f, 1.0f, 6.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.2f, 5.5f, -2.55f, 0.9f, 6.1f, 0.9f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.15f, 10.75f, -2.9f, 1.65f, 1.25f, 0.8f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 0.15f, 10.9f, -2.9f, 2.0f, 1.1f, 0.8f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.2f, 3.25f, -3.0f, 2.4f, 1.65f, 0.9f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.2f, 3.9f, -2.55f, 0.65f, 1.6f, 1.7f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 9.1f, -3.0f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 9.1f, 2.65f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.1f, 9.1f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, 1.75f, 9.1f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 5.85f, -3.0f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 5.85f, 2.65f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.1f, 5.85f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, 1.75f, 5.85f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, 1.0f, 3.55f, -3.1f, 0.7f, 1.0f, 0.6f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -0.5f, 5.1f, -2.8f, 1.0f, 6.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.1f, 5.5f, -2.55f, 0.9f, 5.5f, 0.9f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.15f, 10.75f, -2.9f, 1.65f, 1.25f, 0.8f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, 0.15f, 10.9f, -2.9f, 2.0f, 1.1f, 0.8f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -1.2f, 3.25f, -3.0f, 2.4f, 1.65f, 0.9f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.1f, 3.9f, -2.55f, 0.65f, 1.6f, 1.7f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 9.1f, -3.0f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 9.1f, 2.65f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.1f, 9.1f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, 1.75f, 9.1f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 5.85f, -3.0f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 5.85f, 2.65f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.1f, 5.85f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, 1.75f, 5.85f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -1.7f, 3.5f, -3.05f, 0.55f, 0.85f, 0.55f, 0, 0)
        );
    }

    private static List<ArmorCube> boneBoots() {
        return List.of(
            new ArmorCube(Mount.LEFT_FOOT, -0.55f, 1.8f, -2.8f, 1.1f, 4.2f, 0.8f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.9f, 2.0f, -2.6f, 0.7f, 3.7f, 0.65f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, 1.1f, 2.2f, -2.6f, 0.7f, 3.4f, 0.65f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.0f, 3.75f, -2.9f, 4.0f, 0.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.8f, 3.8f, 2.15f, 3.6f, 0.7f, 0.65f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -0.45f, -0.1f, -3.55f, 0.9f, 1.45f, 1.75f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, 1.1f, 0.0f, -3.4f, 0.7f, 1.25f, 1.45f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.9f, 0.15f, -3.3f, 0.6f, 1.05f, 1.3f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.7f, 0.25f, 2.0f, 3.4f, 1.8f, 0.65f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, 1.1f, 2.55f, -3.15f, 0.75f, 0.8f, 0.45f, 0, 32),
            new ArmorCube(Mount.LEFT_FOOT, -2.05f, 2.75f, -3.0f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.LEFT_FOOT, -2.05f, 2.75f, 2.65f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.LEFT_FOOT, -2.1f, 2.75f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.LEFT_FOOT, 1.75f, 2.75f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -0.55f, 1.8f, -2.8f, 1.1f, 4.2f, 0.8f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, 1.1f, 2.0f, -2.6f, 0.7f, 3.7f, 0.65f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.9f, 2.2f, -2.6f, 0.7f, 3.4f, 0.65f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.0f, 3.75f, -2.9f, 4.0f, 0.8f, 0.75f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.8f, 3.8f, 2.15f, 3.6f, 0.7f, 0.65f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -0.45f, -0.1f, -3.55f, 0.9f, 1.45f, 1.75f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.9f, 0.0f, -3.4f, 0.7f, 1.25f, 1.45f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, 1.1f, 0.15f, -3.3f, 0.6f, 1.05f, 1.3f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.7f, 0.25f, 2.0f, 3.4f, 1.8f, 0.65f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.9f, 2.55f, -3.15f, 0.75f, 0.8f, 0.45f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.05f, 2.75f, -3.0f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.05f, 2.75f, 2.65f, 4.1f, 0.45f, 0.35f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.1f, 2.75f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, 1.75f, 2.75f, -2.65f, 0.35f, 0.45f, 5.3f, 0, 32)
        );
    }

}
