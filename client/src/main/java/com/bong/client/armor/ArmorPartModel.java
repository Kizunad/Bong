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

        tables.put("copper_helmet", copperHelmet());
        tables.put("copper_chestplate", copperChestplate());
        tables.put("copper_leggings", copperLeggings());
        tables.put("copper_boots", copperBoots());

        tables.put("hide_helmet", hideHelmet());
        tables.put("hide_chestplate", hideChestplate());
        tables.put("hide_leggings", hideLeggings());
        tables.put("hide_boots", hideBoots());

        tables.put("scroll_wrap_helmet", scrollWrapHelmet());
        tables.put("scroll_wrap_chestplate", scrollWrapChestplate());
        tables.put("scroll_wrap_leggings", scrollWrapLeggings());
        tables.put("scroll_wrap_boots", scrollWrapBoots());
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

    private static List<ArmorCube> copperHelmet() {
        return List.of(
            new ArmorCube(Mount.HEAD, -4.30f, 31.40f, -4.30f, 8.60f, 1.20f, 8.60f, 0, 0),
            new ArmorCube(Mount.HEAD, -3.60f, 32.50f, -3.60f, 7.20f, 1.00f, 7.20f, 0, 0),
            new ArmorCube(Mount.HEAD, -2.60f, 33.40f, -2.60f, 5.20f, 0.80f, 5.20f, 0, 0),
            new ArmorCube(Mount.HEAD, -0.60f, 32.20f, -4.60f, 1.20f, 2.20f, 9.00f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.60f, 28.50f, -5.10f, 1.20f, 3.80f, 0.80f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.70f, 34.00f, -0.90f, 1.40f, 1.80f, 1.80f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.50f, 35.60f, -0.40f, 1.00f, 1.60f, 1.20f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.50f, 36.40f, 0.40f, 1.00f, 0.80f, 1.20f, 32, 32),
            new ArmorCube(Mount.HEAD, -4.55f, 28.50f, -4.75f, 9.10f, 3.20f, 0.90f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.55f, 25.00f, 3.85f, 9.10f, 6.80f, 0.90f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.75f, 25.00f, -3.80f, 0.90f, 6.80f, 7.60f, 0, 0),
            new ArmorCube(Mount.HEAD, 3.85f, 25.00f, -3.80f, 0.90f, 6.80f, 7.60f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.80f, 28.80f, -4.95f, 9.60f, 1.80f, 0.85f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.45f, 25.40f, -5.25f, 0.90f, 3.60f, 0.60f, 32, 32),
            new ArmorCube(Mount.HEAD, -4.95f, 23.40f, -4.20f, 1.10f, 5.40f, 2.60f, 0, 0),
            new ArmorCube(Mount.HEAD, 3.85f, 23.40f, -4.20f, 1.10f, 5.40f, 2.60f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.70f, 26.20f, 4.00f, 9.40f, 1.80f, 0.85f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.90f, 24.40f, 4.20f, 9.80f, 1.90f, 0.90f, 0, 0),
            new ArmorCube(Mount.HEAD, -5.15f, 22.50f, 4.45f, 10.30f, 2.00f, 0.95f, 32, 0),
            new ArmorCube(Mount.HEAD, -5.40f, 20.60f, 4.70f, 10.80f, 2.00f, 1.00f, 0, 0),
            new ArmorCube(Mount.HEAD, -5.65f, 18.70f, 4.95f, 11.30f, 2.00f, 1.05f, 32, 0),
            new ArmorCube(Mount.HEAD, -3.20f, 29.40f, -5.15f, 0.60f, 0.60f, 0.35f, 32, 32),
            new ArmorCube(Mount.HEAD, 2.60f, 29.40f, -5.15f, 0.60f, 0.60f, 0.35f, 32, 32),
            new ArmorCube(Mount.HEAD, -5.05f, 26.00f, -3.00f, 0.35f, 0.60f, 0.60f, 32, 32),
            new ArmorCube(Mount.HEAD, 4.70f, 26.00f, -3.00f, 0.35f, 0.60f, 0.60f, 32, 32)
        );
    }

    private static List<ArmorCube> copperChestplate() {
        return List.of(
            new ArmorCube(Mount.BODY, -4.30f, 20.80f, -2.75f, 8.60f, 2.80f, 0.80f, 0, 0),
            new ArmorCube(Mount.BODY, -4.20f, 17.80f, -2.85f, 8.40f, 3.20f, 0.85f, 0, 0),
            new ArmorCube(Mount.BODY, -4.00f, 14.80f, -2.85f, 8.00f, 3.20f, 0.85f, 32, 0),
            new ArmorCube(Mount.BODY, -3.80f, 12.00f, -2.75f, 7.60f, 3.00f, 0.80f, 0, 0),
            new ArmorCube(Mount.BODY, -2.20f, 17.40f, -3.20f, 4.40f, 4.40f, 0.65f, 32, 32),
            new ArmorCube(Mount.BODY, -1.60f, 18.00f, -3.55f, 3.20f, 3.20f, 0.50f, 32, 32),
            new ArmorCube(Mount.BODY, -0.80f, 18.80f, -3.90f, 1.60f, 1.60f, 0.45f, 32, 32),
            new ArmorCube(Mount.BODY, -0.50f, 21.80f, -3.20f, 1.00f, 0.80f, 0.50f, 32, 32),
            new ArmorCube(Mount.BODY, -4.20f, 20.40f, 1.95f, 8.40f, 3.20f, 0.80f, 0, 0),
            new ArmorCube(Mount.BODY, -4.10f, 17.20f, 2.00f, 8.20f, 3.40f, 0.80f, 0, 0),
            new ArmorCube(Mount.BODY, -3.90f, 14.20f, 2.05f, 7.80f, 3.20f, 0.80f, 32, 0),
            new ArmorCube(Mount.BODY, -3.80f, 12.00f, 1.95f, 7.60f, 2.40f, 0.75f, 0, 0),
            new ArmorCube(Mount.BODY, -4.55f, 13.00f, -1.80f, 0.75f, 10.00f, 3.60f, 0, 32),
            new ArmorCube(Mount.BODY, 3.80f, 13.00f, -1.80f, 0.75f, 10.00f, 3.60f, 0, 32),
            new ArmorCube(Mount.BODY, -4.40f, 22.80f, -2.90f, 3.60f, 1.20f, 0.90f, 0, 32),
            new ArmorCube(Mount.BODY, 0.80f, 22.80f, -2.90f, 3.60f, 1.20f, 0.90f, 0, 32),
            new ArmorCube(Mount.BODY, -6.20f, 22.40f, -2.70f, 2.40f, 1.80f, 5.40f, 0, 0),
            new ArmorCube(Mount.BODY, -6.55f, 20.80f, -2.60f, 2.35f, 1.80f, 5.20f, 0, 0),
            new ArmorCube(Mount.BODY, -6.85f, 19.00f, -2.50f, 2.20f, 1.90f, 5.00f, 32, 0),
            new ArmorCube(Mount.BODY, -7.10f, 17.20f, -2.40f, 2.10f, 1.90f, 4.80f, 32, 0),
            new ArmorCube(Mount.BODY, -4.90f, 21.00f, -2.85f, 0.90f, 3.20f, 0.40f, 0, 32),
            new ArmorCube(Mount.BODY, 3.80f, 22.40f, -2.70f, 2.40f, 1.80f, 5.40f, 0, 0),
            new ArmorCube(Mount.BODY, 4.20f, 20.80f, -2.60f, 2.35f, 1.80f, 5.20f, 0, 0),
            new ArmorCube(Mount.BODY, 4.65f, 19.00f, -2.50f, 2.20f, 1.90f, 5.00f, 32, 0),
            new ArmorCube(Mount.BODY, 5.00f, 17.20f, -2.40f, 2.10f, 1.90f, 4.80f, 32, 0),
            new ArmorCube(Mount.BODY, 4.00f, 21.00f, -2.85f, 0.90f, 3.20f, 0.40f, 0, 32),
            new ArmorCube(Mount.BODY, -4.20f, 11.40f, -2.90f, 8.40f, 1.30f, 5.80f, 0, 32),
            new ArmorCube(Mount.BODY, -1.10f, 10.60f, -3.30f, 2.20f, 2.00f, 0.70f, 0, 32),
            new ArmorCube(Mount.BODY, -0.90f, 8.80f, -3.15f, 0.65f, 2.00f, 0.40f, 0, 32),
            new ArmorCube(Mount.BODY, 0.25f, 8.80f, -3.15f, 0.65f, 2.00f, 0.40f, 0, 32),
            new ArmorCube(Mount.BODY, -3.50f, 21.60f, -2.95f, 0.60f, 0.60f, 0.35f, 32, 32),
            new ArmorCube(Mount.BODY, 2.90f, 21.60f, -2.95f, 0.60f, 0.60f, 0.35f, 32, 32),
            new ArmorCube(Mount.BODY, -3.10f, 13.40f, -2.95f, 0.60f, 0.60f, 0.35f, 32, 32),
            new ArmorCube(Mount.BODY, 2.50f, 13.40f, -2.95f, 0.60f, 0.60f, 0.35f, 32, 32)
        );
    }

    private static List<ArmorCube> copperLeggings() {
        return List.of(
            new ArmorCube(Mount.LEFT_LEG, -2.20f, 9.60f, -2.80f, 4.40f, 2.60f, 0.85f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.25f, 7.30f, -2.90f, 4.50f, 2.50f, 0.85f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.30f, 5.00f, -3.00f, 4.60f, 2.50f, 0.85f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.35f, 2.80f, -3.10f, 4.70f, 2.40f, 0.90f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.65f, 9.20f, -2.40f, 0.80f, 2.80f, 4.80f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.75f, 6.60f, -2.40f, 0.80f, 2.80f, 4.80f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.85f, 4.00f, -2.40f, 0.85f, 2.80f, 4.80f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.95f, 1.40f, -2.40f, 0.90f, 2.80f, 4.80f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.10f, 7.40f, 2.00f, 4.20f, 4.80f, 0.80f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.20f, 3.40f, 2.10f, 4.40f, 4.20f, 0.85f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.10f, 3.00f, -2.90f, 4.20f, 2.00f, 0.95f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.80f, 3.20f, -3.25f, 3.60f, 2.40f, 0.70f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -0.90f, 3.70f, -3.60f, 1.80f, 1.40f, 0.45f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 9.00f, 1.95f, 4.10f, 0.80f, 0.45f, 0, 32),
            new ArmorCube(Mount.LEFT_LEG, -2.05f, 4.20f, 1.95f, 4.10f, 0.80f, 0.45f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.20f, 9.60f, -2.80f, 4.40f, 2.60f, 0.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.25f, 7.30f, -2.90f, 4.50f, 2.50f, 0.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.30f, 5.00f, -3.00f, 4.60f, 2.50f, 0.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.35f, 2.80f, -3.10f, 4.70f, 2.40f, 0.90f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.45f, 9.20f, -2.40f, 0.80f, 2.80f, 4.80f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.55f, 6.60f, -2.40f, 0.80f, 2.80f, 4.80f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.65f, 4.00f, -2.40f, 0.85f, 2.80f, 4.80f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.75f, 1.40f, -2.40f, 0.90f, 2.80f, 4.80f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.10f, 7.40f, 2.00f, 4.20f, 4.80f, 0.80f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.20f, 3.40f, 2.10f, 4.40f, 4.20f, 0.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.10f, 3.00f, -2.90f, 4.20f, 2.00f, 0.95f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -1.80f, 3.20f, -3.25f, 3.60f, 2.40f, 0.70f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -0.90f, 3.70f, -3.60f, 1.80f, 1.40f, 0.45f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 9.00f, 1.95f, 4.10f, 0.80f, 0.45f, 0, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.05f, 4.20f, 1.95f, 4.10f, 0.80f, 0.45f, 0, 32)
        );
    }

    private static List<ArmorCube> copperBoots() {
        return List.of(
            new ArmorCube(Mount.LEFT_FOOT, -2.10f, 3.80f, -2.75f, 4.20f, 2.40f, 0.85f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.05f, 1.60f, -2.80f, 4.10f, 2.40f, 0.85f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -0.50f, 1.60f, -3.05f, 1.00f, 4.40f, 0.35f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -2.25f, 0.00f, -3.30f, 4.50f, 2.00f, 1.40f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.05f, 0.00f, -3.65f, 4.10f, 1.20f, 0.50f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -0.35f, 0.70f, -3.85f, 0.70f, 0.70f, 0.35f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -2.15f, 3.60f, -2.75f, 4.30f, 0.80f, 5.50f, 0, 32),
            new ArmorCube(Mount.LEFT_FOOT, 1.75f, 3.50f, -0.60f, 0.45f, 1.00f, 1.20f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -2.10f, 0.40f, 1.95f, 4.20f, 2.60f, 0.85f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.30f, -0.25f, -3.50f, 4.60f, 0.50f, 5.80f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.10f, 3.80f, -2.75f, 4.20f, 2.40f, 0.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.05f, 1.60f, -2.80f, 4.10f, 2.40f, 0.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -0.50f, 1.60f, -3.05f, 1.00f, 4.40f, 0.35f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.25f, 0.00f, -3.30f, 4.50f, 2.00f, 1.40f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.05f, 0.00f, -3.65f, 4.10f, 1.20f, 0.50f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -0.35f, 0.70f, -3.85f, 0.70f, 0.70f, 0.35f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.15f, 3.60f, -2.75f, 4.30f, 0.80f, 5.50f, 0, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.55f, 3.50f, -0.60f, 0.45f, 1.00f, 1.20f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.10f, 0.40f, 1.95f, 4.20f, 2.60f, 0.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.30f, -0.25f, -3.50f, 4.60f, 0.50f, 5.80f, 0, 32)
        );
    }

    private static List<ArmorCube> hideHelmet() {
        return List.of(
            new ArmorCube(Mount.HEAD, -4.5f, 32.0f, -5.6f, 9.0f, 1.6f, 5.7f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.7f, 31.55f, 0.1f, 9.4f, 1.6f, 4.55f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.7f, 31.8f, -5.75f, 9.4f, 1.3f, 0.9f, 32, 0),
            new ArmorCube(Mount.HEAD, -5.4f, 30.3f, -5.2f, 10.8f, 2.15f, 1.4f, 32, 0),
            new ArmorCube(Mount.HEAD, -5.4f, 30.3f, -3.8f, 1.4f, 2.15f, 2.9f, 32, 0),
            new ArmorCube(Mount.HEAD, 4.0f, 30.3f, -3.8f, 1.4f, 2.15f, 2.9f, 32, 0),
            new ArmorCube(Mount.HEAD, -5.25f, 24.3f, -4.3f, 1.2f, 7.3f, 5.9f, 0, 32),
            new ArmorCube(Mount.HEAD, 4.05f, 24.3f, -4.3f, 1.2f, 7.3f, 5.9f, 0, 32),
            new ArmorCube(Mount.HEAD, -5.05f, 23.15f, -3.3f, 0.95f, 1.2f, 3.4f, 0, 32),
            new ArmorCube(Mount.HEAD, 4.1f, 23.15f, -3.3f, 0.95f, 1.2f, 3.4f, 0, 32),
            new ArmorCube(Mount.HEAD, -5.05f, 23.9f, 1.6f, 1.0f, 7.65f, 2.5f, 0, 32),
            new ArmorCube(Mount.HEAD, 4.05f, 23.9f, 1.6f, 1.0f, 7.65f, 2.5f, 0, 32),
            new ArmorCube(Mount.HEAD, -4.6f, 27.3f, 3.95f, 9.2f, 4.35f, 1.1f, 0, 32),
            new ArmorCube(Mount.HEAD, -4.25f, 23.0f, 4.05f, 8.5f, 4.3f, 0.95f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.0f, 22.35f, 4.1f, 8.0f, 0.7f, 0.85f, 32, 0),
            new ArmorCube(Mount.HEAD, -5.5f, 31.15f, -5.3f, 11.0f, 0.3f, 0.3f, 32, 32),
            new ArmorCube(Mount.HEAD, -5.5f, 31.15f, -3.95f, 0.3f, 0.3f, 7.95f, 32, 32),
            new ArmorCube(Mount.HEAD, 5.2f, 31.15f, -3.95f, 0.3f, 0.3f, 7.95f, 32, 32),
            new ArmorCube(Mount.HEAD, -4.55f, 31.15f, 5.05f, 9.1f, 0.3f, 0.3f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.6f, 30.7f, 5.3f, 1.2f, 1.0f, 0.35f, 32, 32),
            new ArmorCube(Mount.HEAD, -0.2f, 29.3f, 5.35f, 0.35f, 1.4f, 0.3f, 32, 32),
            new ArmorCube(Mount.HEAD, -2.85f, 30.75f, -5.45f, 1.0f, 1.0f, 0.35f, 32, 32),
            new ArmorCube(Mount.HEAD, 1.85f, 30.75f, -5.45f, 1.0f, 1.0f, 0.35f, 32, 32),
            new ArmorCube(Mount.HEAD, -5.6f, 29.15f, -4.9f, 0.3f, 1.9f, 0.3f, 32, 32),
            new ArmorCube(Mount.HEAD, 5.3f, 29.5f, -4.9f, 0.3f, 1.5f, 0.3f, 32, 32)
        );
    }

    private static List<ArmorCube> hideChestplate() {
        return List.of(
            new ArmorCube(Mount.BODY, -4.35f, 12.7f, -2.75f, 8.7f, 10.1f, 0.8f, 0, 0),
            new ArmorCube(Mount.BODY, -4.35f, 12.7f, 1.95f, 8.7f, 11.3f, 0.8f, 0, 0),
            new ArmorCube(Mount.BODY, -4.5f, 12.3f, -2.35f, 0.6f, 11.1f, 4.7f, 0, 0),
            new ArmorCube(Mount.BODY, 3.9f, 12.3f, -2.35f, 0.6f, 11.1f, 4.7f, 0, 0),
            new ArmorCube(Mount.BODY, -4.4f, 22.8f, -2.8f, 2.1f, 1.12f, 5.48f, 0, 0),
            new ArmorCube(Mount.BODY, 2.3f, 22.8f, -2.8f, 2.1f, 1.12f, 5.48f, 0, 0),
            new ArmorCube(Mount.BODY, -4.45f, 12.05f, -2.92f, 8.9f, 0.9f, 0.94f, 32, 0),
            new ArmorCube(Mount.BODY, -4.45f, 12.05f, 1.98f, 8.9f, 0.9f, 0.94f, 32, 0),
            new ArmorCube(Mount.BODY, -5.800000000000001f, 23.8f, -2.65f, 1.85f, 0.75f, 5.3f, 0, 0),
            new ArmorCube(Mount.BODY, -8.15f, 23.8f, -2.6f, 2.35f, 0.35f, 5.2f, 0, 0),
            new ArmorCube(Mount.BODY, -5.7f, 21.1f, -2.6f, 1.55f, 2.75f, 5.2f, 0, 0),
            new ArmorCube(Mount.BODY, -7.0f, 20.55f, -2.55f, 1.3f, 3.3f, 5.1f, 0, 0),
            new ArmorCube(Mount.BODY, -8.2f, 20.95f, -2.4f, 1.2f, 2.9f, 4.8f, 0, 0),
            new ArmorCube(Mount.BODY, -8.05f, 19.95f, -2.45f, 3.85f, 1.3f, 1.1f, 32, 0),
            new ArmorCube(Mount.BODY, -7.35f, 19.8f, -0.55f, 2.8f, 1.45f, 1.1f, 32, 0),
            new ArmorCube(Mount.BODY, -7.85f, 20.1f, 1.35f, 3.65f, 1.15f, 1.1f, 32, 0),
            new ArmorCube(Mount.BODY, 3.95f, 23.8f, -2.65f, 1.85f, 0.75f, 5.3f, 0, 0),
            new ArmorCube(Mount.BODY, 5.8f, 23.8f, -2.6f, 2.35f, 0.35f, 5.2f, 0, 0),
            new ArmorCube(Mount.BODY, 4.15f, 21.1f, -2.6f, 1.55f, 2.75f, 5.2f, 0, 0),
            new ArmorCube(Mount.BODY, 5.7f, 20.55f, -2.55f, 1.3f, 3.3f, 5.1f, 0, 0),
            new ArmorCube(Mount.BODY, 7.0f, 20.95f, -2.4f, 1.2f, 2.9f, 4.8f, 0, 0),
            new ArmorCube(Mount.BODY, 4.2f, 19.95f, -2.45f, 3.85f, 1.3f, 1.1f, 32, 0),
            new ArmorCube(Mount.BODY, 4.55f, 19.8f, -0.55f, 2.8f, 1.45f, 1.1f, 32, 0),
            new ArmorCube(Mount.BODY, 4.2f, 20.1f, 1.35f, 3.65f, 1.15f, 1.1f, 32, 0),
            new ArmorCube(Mount.BODY, -0.6f, 18.4f, -2.9f, 3.5f, 3.4f, 0.25f, 34, 18),
            new ArmorCube(Mount.BODY, -2.6f, 14.3f, -2.9f, 3.0f, 3.3f, 0.25f, 46, 18),
            new ArmorCube(Mount.BODY, -0.35f, 21.55f, -3.05f, 1.5f, 0.28f, 0.3f, 32, 0),
            new ArmorCube(Mount.BODY, 1.05f, 18.3f, -3.05f, 1.6f, 0.28f, 0.3f, 32, 0),
            new ArmorCube(Mount.BODY, 2.75f, 19.1f, -3.05f, 0.28f, 1.7f, 0.3f, 32, 0),
            new ArmorCube(Mount.BODY, -2.05f, 17.45f, -3.05f, 1.4f, 0.26f, 0.3f, 32, 32),
            new ArmorCube(Mount.BODY, -1.6f, 14.2f, -3.05f, 1.5f, 0.26f, 0.3f, 32, 32),
            new ArmorCube(Mount.BODY, -2.7f, 15.0f, -3.05f, 0.26f, 1.55f, 0.3f, 32, 32),
            new ArmorCube(Mount.BODY, -4.95f, 13.7f, -2.92f, 0.55f, 0.52f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, 4.4f, 13.9f, -2.92f, 0.55f, 0.58f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, -4.95f, 15.15f, -2.92f, 0.55f, 0.5800000000000001f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, 4.4f, 15.0f, -2.92f, 0.55f, 0.5299999999999999f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, -4.95f, 16.4f, -2.92f, 0.55f, 0.64f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, 4.4f, 16.55f, -2.92f, 0.55f, 0.48f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, -4.95f, 17.85f, -2.92f, 0.55f, 0.52f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, 4.4f, 17.7f, -2.92f, 0.55f, 0.58f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, -4.95f, 19.05f, -2.92f, 0.55f, 0.5800000000000001f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, 4.4f, 19.2f, -2.92f, 0.55f, 0.5299999999999999f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, -4.95f, 20.35f, -2.92f, 0.55f, 0.64f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, 4.4f, 20.2f, -2.92f, 0.55f, 0.48f, 5.84f, 32, 32),
            new ArmorCube(Mount.BODY, -4.6f, 12.5f, -3.0f, 9.2f, 0.7f, 0.6f, 32, 32),
            new ArmorCube(Mount.BODY, -4.6f, 12.5f, 2.4f, 9.2f, 0.7f, 0.6f, 32, 32),
            new ArmorCube(Mount.BODY, -5.0f, 12.5f, -2.4f, 0.55f, 0.7f, 4.8f, 32, 32),
            new ArmorCube(Mount.BODY, 4.45f, 12.5f, -2.4f, 0.55f, 0.7f, 4.8f, 32, 32),
            new ArmorCube(Mount.BODY, -0.95f, 12.1f, -3.25f, 1.9f, 1.35f, 0.7f, 32, 32),
            new ArmorCube(Mount.BODY, -0.62f, 10.6f, -3.3f, 0.46f, 1.65f, 0.42f, 32, 32),
            new ArmorCube(Mount.BODY, -0.72f, 9.15f, -3.5f, 0.42f, 1.6f, 0.4f, 32, 32),
            new ArmorCube(Mount.BODY, 0.24f, 10.5f, -3.3f, 0.46f, 1.7f, 0.42f, 32, 32),
            new ArmorCube(Mount.BODY, -0.7f, 12.25f, 3.0f, 1.4f, 1.05f, 0.55f, 32, 32)
        );
    }

    private static List<ArmorCube> hideLeggings() {
        return List.of(
            new ArmorCube(Mount.LEFT_LEG, -1.9f, 10.35f, -2.55f, 4.45f, 1.9f, 0.8f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.9f, 10.35f, 1.75f, 4.45f, 1.9f, 0.8f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 2.0f, 10.4f, -2.5f, 0.6f, 1.8f, 5.0f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.85f, 10.0f, -2.72f, 4.5f, 0.65f, 0.85f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.85f, 10.0f, 1.87f, 4.5f, 0.65f, 0.85f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, 2.35f, 10.62f, -2.7f, 0.5f, 0.44f, 5.4f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, 2.35f, 11.3f, -2.7f, 0.5f, 0.4f, 5.4f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, 2.35f, 11.85f, -2.7f, 0.5f, 0.44f, 5.4f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -1.65f, 3.3f, -2.65f, 3.95f, 6.85f, 0.85f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.7f, 3.45f, -2.55f, 0.65f, 6.6f, 1.85f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.68f, 3.45f, -2.55f, 0.63f, 6.6f, 1.85f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.72f, 3.02f, -2.74f, 4.12f, 0.6f, 1.0f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.6f, 9.72f, -2.76f, 3.85f, 0.42f, 0.24f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, 2.02f, 3.95f, -2.78f, 0.38f, 5.75f, 0.26f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -1.3f, 4.4f, -3.22f, 3.25f, 1.7f, 0.75f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -0.7f, 6.1f, -3.14f, 2.05f, 0.55f, 0.65f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -0.7f, 3.85f, -3.14f, 2.05f, 0.55f, 0.65f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.98f, 4.7f, 1.98f, 0.34f, 5.4f, 0.34f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.9f, 5.3f, 1.98f, 0.34f, 4.8f, 0.34f, 32, 0),
            new ArmorCube(Mount.LEFT_LEG, 1.05f, 8.5f, 2.04f, 1.4f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -0.35f, 8.0f, 2.04f, 1.6f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -1.6f, 7.5f, 2.04f, 1.45f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -1.6f, 6.85f, 2.04f, 1.45f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -0.35f, 6.3f, 2.04f, 1.6f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, 1.05f, 5.75f, 2.04f, 1.4f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.LEFT_LEG, -1.8f, 6.98f, 2.0f, 0.85f, 0.68f, 0.52f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.5500000000000003f, 10.35f, -2.55f, 4.45f, 1.9f, 0.8f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.5500000000000003f, 10.35f, 1.75f, 4.45f, 1.9f, 0.8f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.6f, 10.4f, -2.5f, 0.6f, 1.8f, 5.0f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.65f, 10.0f, -2.72f, 4.5f, 0.65f, 0.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.65f, 10.0f, 1.87f, 4.5f, 0.65f, 0.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.85f, 10.62f, -2.7f, 0.5f, 0.44f, 5.4f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.85f, 11.3f, -2.7f, 0.5f, 0.4f, 5.4f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.85f, 11.85f, -2.7f, 0.5f, 0.44f, 5.4f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.3000000000000003f, 3.3f, -2.65f, 3.95f, 6.85f, 0.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.35f, 3.45f, -2.55f, 0.65f, 6.6f, 1.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, 1.0499999999999998f, 3.45f, -2.55f, 0.63f, 6.6f, 1.85f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.4000000000000004f, 3.02f, -2.74f, 4.12f, 0.6f, 1.0f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.25f, 9.72f, -2.76f, 3.85f, 0.42f, 0.24f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.4f, 3.95f, -2.78f, 0.38f, 5.75f, 0.26f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -1.95f, 4.4f, -3.22f, 3.25f, 1.7f, 0.75f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -1.3499999999999999f, 6.1f, -3.14f, 2.05f, 0.55f, 0.65f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -1.3499999999999999f, 3.85f, -3.14f, 2.05f, 0.55f, 0.65f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.32f, 4.7f, 2.05f, 0.34f, 5.4f, 0.34f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, 1.5599999999999998f, 5.3f, 2.05f, 0.34f, 4.8f, 0.34f, 32, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.45f, 8.5f, 2.11f, 1.4f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -1.25f, 8.0f, 2.11f, 1.6f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, 0.15000000000000013f, 7.5f, 2.11f, 1.45f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, 0.15000000000000013f, 6.85f, 2.11f, 1.45f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -1.25f, 6.3f, 2.11f, 1.6f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, -2.45f, 5.75f, 2.11f, 1.4f, 0.36f, 0.36f, 32, 32),
            new ArmorCube(Mount.RIGHT_LEG, 0.9500000000000001f, 6.98f, 2.07f, 0.85f, 0.68f, 0.52f, 32, 32)
        );
    }

    private static List<ArmorCube> hideBoots() {
        return List.of(
            new ArmorCube(Mount.LEFT_FOOT, -1.88f, 2.35f, -2.35f, 4.23f, 3.0f, 4.7f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.83f, 5.3f, -2.3f, 4.11f, 0.72f, 1.02f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.79f, 5.26f, 1.2f, 4.02f, 0.45f, 1.12f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, 1.3f, 5.29f, -1.24f, 1.02f, 0.62f, 2.4f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.86f, 5.23f, -1.24f, 0.62f, 0.42f, 2.4f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.87f, 0.75f, -2.5f, 4.19f, 1.7f, 4.9f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.85f, 0.79f, -3.78f, 3.88f, 1.46f, 1.34f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.58f, 1.25f, -4.45f, 3.5f, 1.05f, 0.5f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, 0.03f, 2.14f, -4.35f, 0.4f, 0.26f, 1.95f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.88f, 0.36f, -4.42f, 4.42f, 0.46f, 7.06f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.9f, -0.25f, -4.52f, 4.58f, 0.63f, 7.28f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.84f, 0.84f, 1.98f, 4.13f, 0.72f, 0.7f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.55f, 0.5f, -4.5f, 3.42f, 0.85f, 0.74f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, 2.32f, 1.25f, -1.15f, 0.3f, 1.35f, 1.85f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.6f, 1.35f, 2.42f, 2.4f, 1.25f, 0.28f, 32, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.83f, 2.6f, -2.52f, 4.26f, 0.5f, 0.44f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -1.82f, 2.62f, 2.08f, 4.24f, 0.5f, 0.44f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, 2.32f, 2.6f, -2.1f, 0.44f, 0.5f, 4.2f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -1.81f, 2.6f, -2.1f, 0.42f, 0.5f, 4.2f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -0.05f, 2.72f, -3.0f, 0.7f, 1.0f, 0.58f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, 0.6f, 3.15f, -2.95f, 1.5f, 0.62f, 0.46f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -1.7f, 3.22f, -2.95f, 1.62f, 0.56f, 0.46f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, 0.75f, 1.5f, -2.97f, 0.38f, 1.18f, 0.42f, 32, 32),
            new ArmorCube(Mount.LEFT_FOOT, -0.92f, 1.78f, -2.97f, 0.38f, 0.94f, 0.42f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.3500000000000005f, 2.35f, -2.35f, 4.23f, 3.0f, 4.7f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.2800000000000002f, 5.3f, -2.3f, 4.11f, 0.72f, 1.02f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.2299999999999995f, 5.26f, 1.2f, 4.02f, 0.45f, 1.12f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.3200000000000003f, 5.29f, -1.24f, 1.02f, 0.62f, 2.4f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, 1.2400000000000002f, 5.23f, -1.24f, 0.62f, 0.42f, 2.4f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.3200000000000003f, 0.75f, -2.5f, 4.19f, 1.7f, 4.9f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.03f, 0.79f, -3.78f, 3.88f, 1.46f, 1.34f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.92f, 1.25f, -4.45f, 3.5f, 1.05f, 0.5f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -0.43000000000000005f, 2.14f, -4.35f, 0.4f, 0.26f, 1.95f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.54f, 0.36f, -4.42f, 4.42f, 0.46f, 7.06f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.68f, -0.25f, -4.52f, 4.58f, 0.63f, 7.28f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.29f, 0.84f, 1.98f, 4.13f, 0.72f, 0.7f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.8699999999999999f, 0.5f, -4.5f, 3.42f, 0.85f, 0.74f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.6199999999999997f, 1.25f, -1.15f, 0.3f, 1.35f, 1.85f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -0.7999999999999998f, 1.35f, 2.42f, 2.4f, 1.25f, 0.28f, 32, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.4299999999999997f, 2.6f, -2.52f, 4.26f, 0.5f, 0.44f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.42f, 2.62f, 2.08f, 4.24f, 0.5f, 0.44f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.76f, 2.6f, -2.1f, 0.44f, 0.5f, 4.2f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, 1.3900000000000001f, 2.6f, -2.1f, 0.42f, 0.5f, 4.2f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -0.6499999999999999f, 2.72f, -3.0f, 0.7f, 1.0f, 0.58f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -2.1f, 3.15f, -2.95f, 1.5f, 0.62f, 0.46f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, 0.07999999999999985f, 3.22f, -2.95f, 1.62f, 0.56f, 0.46f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, -1.13f, 1.5f, -2.97f, 0.38f, 1.18f, 0.42f, 32, 32),
            new ArmorCube(Mount.RIGHT_FOOT, 0.54f, 1.78f, -2.97f, 0.38f, 0.94f, 0.42f, 32, 32)
        );
    }

    private static List<ArmorCube> scrollWrapHelmet() {
        return List.of(
            new ArmorCube(Mount.HEAD, -4.10f, 31.05f, -4.10f, 8.20f, 1.20f, 4.14f, 24, 28),
            new ArmorCube(Mount.HEAD, -4.10f, 31.05f, 0.04f, 8.20f, 1.20f, 4.06f, 24, 28),
            new ArmorCube(Mount.HEAD, -4.18f, 26.20f, -4.18f, 8.36f, 4.90f, 4.14f, 24, 28),
            new ArmorCube(Mount.HEAD, -4.18f, 26.20f, -0.04f, 8.36f, 4.90f, 4.22f, 24, 28),
            new ArmorCube(Mount.HEAD, -3.50f, 32.25f, -3.80f, 7.00f, 0.45f, 3.84f, 0, 0),
            new ArmorCube(Mount.HEAD, -3.50f, 32.25f, 0.04f, 7.00f, 0.45f, 3.76f, 0, 0),
            new ArmorCube(Mount.HEAD, -2.20f, 32.38f, -4.10f, 4.40f, 0.38f, 3.80f, 24, 0),
            new ArmorCube(Mount.HEAD, -2.20f, 32.38f, 0.30f, 4.40f, 0.38f, 3.80f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.20f, 31.42f, -4.35f, 8.40f, 0.94f, 1.80f, 24, 0),
            new ArmorCube(Mount.HEAD, -4.20f, 31.42f, 2.55f, 8.40f, 0.94f, 1.80f, 0, 0),
            new ArmorCube(Mount.HEAD, 2.55f, 31.36f, -4.20f, 1.80f, 0.94f, 4.30f, 0, 0),
            new ArmorCube(Mount.HEAD, 2.55f, 31.36f, 0.10f, 1.80f, 0.94f, 4.10f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.35f, 31.36f, -4.20f, 1.80f, 0.94f, 4.30f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.35f, 31.36f, 0.10f, 1.80f, 0.94f, 4.10f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.45f, 28.62f, -4.55f, 8.90f, 0.85f, 0.60f, 0, 28),
            new ArmorCube(Mount.HEAD, -4.45f, 28.62f, 3.95f, 8.90f, 0.85f, 0.60f, 0, 28),
            new ArmorCube(Mount.HEAD, 3.95f, 28.58f, -4.45f, 0.60f, 0.85f, 8.90f, 0, 28),
            new ArmorCube(Mount.HEAD, -4.55f, 28.58f, -4.45f, 0.60f, 0.85f, 8.90f, 0, 28),
            new ArmorCube(Mount.HEAD, -5.05f, 28.20f, -1.20f, 0.95f, 1.40f, 1.80f, 0, 28),
            new ArmorCube(Mount.HEAD, -4.85f, 24.20f, -1.40f, 0.45f, 4.10f, 0.45f, 0, 28),
            new ArmorCube(Mount.HEAD, -4.85f, 24.80f, -0.40f, 0.45f, 3.50f, 0.45f, 0, 28),
            new ArmorCube(Mount.HEAD, -5.15f, 25.40f, -0.90f, 0.85f, 0.85f, 0.85f, 0, 28),
            new ArmorCube(Mount.HEAD, -1.80f, 22.80f, -4.68f, 3.60f, 6.20f, 0.35f, 24, 0),
            new ArmorCube(Mount.HEAD, 1.85f, 23.60f, -4.62f, 2.40f, 5.20f, 0.35f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.25f, 23.60f, -4.62f, 2.40f, 5.20f, 0.35f, 0, 0),
            new ArmorCube(Mount.HEAD, 4.15f, 22.60f, -3.80f, 0.35f, 6.20f, 2.60f, 24, 0),
            new ArmorCube(Mount.HEAD, 4.15f, 22.20f, -0.80f, 0.35f, 6.60f, 4.40f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.50f, 22.60f, -3.80f, 0.35f, 6.20f, 2.60f, 0, 0),
            new ArmorCube(Mount.HEAD, -4.50f, 22.20f, -0.80f, 0.35f, 6.60f, 4.40f, 24, 0),
            new ArmorCube(Mount.HEAD, -4.10f, 23.40f, 4.12f, 8.20f, 5.60f, 0.38f, 24, 0),
            new ArmorCube(Mount.HEAD, -3.60f, 21.60f, 4.38f, 7.20f, 6.80f, 0.38f, 0, 0),
            new ArmorCube(Mount.HEAD, 1.20f, 20.20f, 4.60f, 2.20f, 5.20f, 0.35f, 0, 0),
            new ArmorCube(Mount.HEAD, -3.40f, 20.20f, 4.60f, 2.20f, 5.20f, 0.35f, 24, 0)
        );
    }

    private static List<ArmorCube> scrollWrapChestplate() {
        return List.of(
            new ArmorCube(Mount.BODY, -4.10f, 20.00f, -2.62f, 8.20f, 3.60f, 0.65f, 24, 0),
            new ArmorCube(Mount.BODY, -4.20f, 17.20f, -2.76f, 8.40f, 3.40f, 0.72f, 0, 0),
            new ArmorCube(Mount.BODY, -4.15f, 14.40f, -2.70f, 8.30f, 3.40f, 0.70f, 24, 0),
            new ArmorCube(Mount.BODY, -4.08f, 12.00f, -2.60f, 8.16f, 3.00f, 0.65f, 0, 0),
            new ArmorCube(Mount.BODY, -4.10f, 20.00f, 1.97f, 8.20f, 3.60f, 0.65f, 0, 0),
            new ArmorCube(Mount.BODY, -4.20f, 17.20f, 2.04f, 8.40f, 3.40f, 0.72f, 24, 0),
            new ArmorCube(Mount.BODY, -4.15f, 14.40f, 2.00f, 8.30f, 3.40f, 0.70f, 0, 0),
            new ArmorCube(Mount.BODY, -4.08f, 12.00f, 1.95f, 8.16f, 3.00f, 0.65f, 24, 0),
            new ArmorCube(Mount.BODY, 3.88f, 12.24f, -2.15f, 0.45f, 11.16f, 4.30f, 24, 28),
            new ArmorCube(Mount.BODY, -4.33f, 12.24f, -2.15f, 0.45f, 11.16f, 4.30f, 24, 28),
            new ArmorCube(Mount.BODY, -3.60f, 23.00f, -2.85f, 7.20f, 0.75f, 0.50f, 0, 28),
            new ArmorCube(Mount.BODY, -0.90f, 22.40f, -3.15f, 1.80f, 1.80f, 0.65f, 0, 28),
            new ArmorCube(Mount.BODY, 2.10f, 12.40f, -2.88f, 0.45f, 10.40f, 0.35f, 0, 28),
            new ArmorCube(Mount.BODY, -2.55f, 12.40f, -2.88f, 0.45f, 10.40f, 0.35f, 0, 28),
            new ArmorCube(Mount.BODY, 2.10f, 12.40f, 2.53f, 0.45f, 10.40f, 0.35f, 0, 28),
            new ArmorCube(Mount.BODY, -2.55f, 12.40f, 2.53f, 0.45f, 10.40f, 0.35f, 0, 28),
            new ArmorCube(Mount.BODY, 3.96f, 18.50f, -2.28f, 0.35f, 0.65f, 4.56f, 0, 28),
            new ArmorCube(Mount.BODY, 3.96f, 14.50f, -2.28f, 0.35f, 0.65f, 4.56f, 0, 28),
            new ArmorCube(Mount.BODY, -4.31f, 18.50f, -2.28f, 0.35f, 0.65f, 4.56f, 0, 28),
            new ArmorCube(Mount.BODY, -4.31f, 14.50f, -2.28f, 0.35f, 0.65f, 4.56f, 0, 28),
            new ArmorCube(Mount.BODY, -4.35f, 12.15f, -2.78f, 8.70f, 0.85f, 5.56f, 0, 28),
            new ArmorCube(Mount.BODY, -2.40f, 9.40f, -2.72f, 4.80f, 2.90f, 0.38f, 24, 0),
            new ArmorCube(Mount.BODY, -2.40f, 9.40f, 2.34f, 4.80f, 2.90f, 0.38f, 0, 0),
            new ArmorCube(Mount.BODY, 3.85f, 22.80f, -2.40f, 4.40f, 1.40f, 4.80f, 24, 28),
            new ArmorCube(Mount.BODY, 4.00f, 23.40f, -2.55f, 4.60f, 1.20f, 5.10f, 24, 0),
            new ArmorCube(Mount.BODY, 4.60f, 21.60f, -2.45f, 4.50f, 1.80f, 4.90f, 0, 0),
            new ArmorCube(Mount.BODY, 5.40f, 19.80f, -2.35f, 4.20f, 2.20f, 4.70f, 24, 0),
            new ArmorCube(Mount.BODY, 4.40f, 24.15f, -2.45f, 3.80f, 0.50f, 4.90f, 0, 28),
            new ArmorCube(Mount.BODY, 3.92f, 12.20f, -2.16f, 4.16f, 5.40f, 4.32f, 0, 0),
            new ArmorCube(Mount.BODY, 3.90f, 16.20f, -2.25f, 4.20f, 0.65f, 4.50f, 0, 28),
            new ArmorCube(Mount.BODY, 3.90f, 12.80f, -2.25f, 4.20f, 0.65f, 4.50f, 0, 28),
            new ArmorCube(Mount.BODY, -8.25f, 22.80f, -2.40f, 4.40f, 1.40f, 4.80f, 24, 28),
            new ArmorCube(Mount.BODY, -8.60f, 23.40f, -2.55f, 4.60f, 1.20f, 5.10f, 24, 0),
            new ArmorCube(Mount.BODY, -9.10f, 21.60f, -2.45f, 4.50f, 1.80f, 4.90f, 0, 0),
            new ArmorCube(Mount.BODY, -9.60f, 19.80f, -2.35f, 4.20f, 2.20f, 4.70f, 24, 0),
            new ArmorCube(Mount.BODY, -8.20f, 24.15f, -2.45f, 3.80f, 0.50f, 4.90f, 0, 28),
            new ArmorCube(Mount.BODY, -8.08f, 12.20f, -2.16f, 4.16f, 5.40f, 4.32f, 0, 0),
            new ArmorCube(Mount.BODY, -8.10f, 16.20f, -2.25f, 4.20f, 0.65f, 4.50f, 0, 28),
            new ArmorCube(Mount.BODY, -8.10f, 12.80f, -2.25f, 4.20f, 0.65f, 4.50f, 0, 28)
        );
    }

    private static List<ArmorCube> scrollWrapLeggings() {
        return List.of(
            new ArmorCube(Mount.LEFT_LEG, -2.12f, 6.84f, -2.10f, 4.24f, 4.80f, 4.24f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -1.80f, 7.84f, -2.36f, 3.60f, 3.40f, 0.35f, 24, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.18f, 10.44f, -2.16f, 4.36f, 0.75f, 4.36f, 0, 28),
            new ArmorCube(Mount.LEFT_LEG, -2.18f, 7.24f, -2.16f, 4.36f, 0.75f, 4.36f, 0, 28),
            new ArmorCube(Mount.LEFT_LEG, -2.10f, 4.84f, -2.58f, 4.20f, 2.60f, 0.75f, 24, 28),
            new ArmorCube(Mount.LEFT_LEG, -1.60f, 5.04f, -2.90f, 3.20f, 2.20f, 0.45f, 24, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.22f, 5.84f, -2.20f, 4.44f, 0.70f, 4.44f, 0, 28),
            new ArmorCube(Mount.LEFT_LEG, -2.08f, 1.84f, -2.06f, 4.16f, 3.40f, 4.16f, 0, 0),
            new ArmorCube(Mount.LEFT_LEG, -2.14f, 3.64f, -2.12f, 4.28f, 0.65f, 4.28f, 0, 28),
            new ArmorCube(Mount.LEFT_LEG, -2.14f, 2.04f, -2.12f, 4.28f, 0.65f, 4.28f, 0, 28),
            new ArmorCube(Mount.LEFT_LEG, 1.70f, 3.44f, -0.58f, 0.55f, 1.10f, 1.20f, 0, 28),
            new ArmorCube(Mount.RIGHT_LEG, -2.12f, 6.76f, -2.14f, 4.24f, 4.80f, 4.24f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -1.80f, 7.76f, -2.40f, 3.60f, 3.40f, 0.35f, 24, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.18f, 10.36f, -2.20f, 4.36f, 0.75f, 4.36f, 0, 28),
            new ArmorCube(Mount.RIGHT_LEG, -2.18f, 7.16f, -2.20f, 4.36f, 0.75f, 4.36f, 0, 28),
            new ArmorCube(Mount.RIGHT_LEG, -2.10f, 4.76f, -2.62f, 4.20f, 2.60f, 0.75f, 24, 28),
            new ArmorCube(Mount.RIGHT_LEG, -1.60f, 4.96f, -2.94f, 3.20f, 2.20f, 0.45f, 24, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.22f, 5.76f, -2.24f, 4.44f, 0.70f, 4.44f, 0, 28),
            new ArmorCube(Mount.RIGHT_LEG, -2.08f, 1.76f, -2.10f, 4.16f, 3.40f, 4.16f, 0, 0),
            new ArmorCube(Mount.RIGHT_LEG, -2.14f, 3.56f, -2.16f, 4.28f, 0.65f, 4.28f, 0, 28),
            new ArmorCube(Mount.RIGHT_LEG, -2.14f, 1.96f, -2.16f, 4.28f, 0.65f, 4.28f, 0, 28),
            new ArmorCube(Mount.RIGHT_LEG, -2.25f, 3.36f, -0.62f, 0.55f, 1.10f, 1.20f, 0, 28)
        );
    }

    private static List<ArmorCube> scrollWrapBoots() {
        return List.of(
            new ArmorCube(Mount.LEFT_FOOT, -2.20f, -0.02f, -3.18f, 4.40f, 1.20f, 5.40f, 24, 28),
            new ArmorCube(Mount.LEFT_FOOT, -2.32f, 0.23f, -3.30f, 4.64f, 0.55f, 5.64f, 0, 28),
            new ArmorCube(Mount.LEFT_FOOT, -2.05f, 0.83f, -3.03f, 4.10f, 1.80f, 3.20f, 24, 0),
            new ArmorCube(Mount.LEFT_FOOT, -1.20f, 1.43f, -3.18f, 2.40f, 0.65f, 0.35f, 0, 28),
            new ArmorCube(Mount.LEFT_FOOT, -2.12f, 1.63f, -2.10f, 4.24f, 3.60f, 4.24f, 0, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.18f, 4.63f, -2.16f, 4.36f, 1.00f, 4.36f, 24, 0),
            new ArmorCube(Mount.LEFT_FOOT, -2.24f, 1.83f, -2.33f, 4.48f, 0.65f, 4.70f, 0, 28),
            new ArmorCube(Mount.LEFT_FOOT, -2.24f, 2.83f, -2.33f, 4.48f, 0.65f, 4.70f, 0, 28),
            new ArmorCube(Mount.LEFT_FOOT, -0.75f, 2.63f, -2.58f, 1.50f, 1.10f, 0.55f, 0, 28),
            new ArmorCube(Mount.RIGHT_FOOT, -2.20f, -0.08f, -3.22f, 4.40f, 1.20f, 5.40f, 24, 28),
            new ArmorCube(Mount.RIGHT_FOOT, -2.32f, 0.17f, -3.34f, 4.64f, 0.55f, 5.64f, 0, 28),
            new ArmorCube(Mount.RIGHT_FOOT, -2.05f, 0.77f, -3.07f, 4.10f, 1.80f, 3.20f, 24, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -1.20f, 1.37f, -3.22f, 2.40f, 0.65f, 0.35f, 0, 28),
            new ArmorCube(Mount.RIGHT_FOOT, -2.12f, 1.57f, -2.14f, 4.24f, 3.60f, 4.24f, 0, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.18f, 4.57f, -2.20f, 4.36f, 1.00f, 4.36f, 24, 0),
            new ArmorCube(Mount.RIGHT_FOOT, -2.24f, 1.77f, -2.37f, 4.48f, 0.65f, 4.70f, 0, 28),
            new ArmorCube(Mount.RIGHT_FOOT, -2.24f, 2.77f, -2.37f, 4.48f, 0.65f, 4.70f, 0, 28),
            new ArmorCube(Mount.RIGHT_FOOT, -0.75f, 2.57f, -2.62f, 1.50f, 1.10f, 0.55f, 0, 28)
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
