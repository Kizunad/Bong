package com.bong.client.armor;

import net.minecraft.client.model.ModelData;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.model.ModelPartBuilder;
import net.minecraft.client.model.ModelPartData;
import net.minecraft.client.model.ModelTransform;
import net.minecraft.client.model.TexturedModelData;

import java.util.List;

/**
 * plan-tarkov-backpack-v1 P4 — 穿戴背包件「破草包」的上身渲染模型烘焙器（spike 路线 (b)）。
 *
 * <p><b>spike 结论 = 渲染路线 (b)，理由</b>：仓库内无「在 vanilla player {@code FeatureRenderer} 里驱动
 * GeckoLib {@code GeoModel}」先例；GeckoLib 4.4.9 的 {@code GeoArmorRenderer<T extends Item & GeoItem>}
 * 从 {@code stack.getItem()} 取 animatable，缺少真实「穿在 armor slot 的 GeoItem ItemStack」会
 * 空指针 / no-op；{@code GeoRenderLayer(GeoRenderer)} 只能挂到 {@code GeoRenderer}，而
 * {@code PlayerEntityRenderer} 不是 {@code GeoRenderer}；且 {@code GeckoLibCache} 在资源重载前为空，
 * headless 测试无法构造。本背包数据源是自定义 inventory snapshot（玩家身上**没有** vanilla equipped
 * 物品），与 GeckoLib armor 管线结构性不匹配（{@code MutationFeatureRenderer} 已为此留空、从未注册）。
 * 故采用纯 vanilla 路线 (b)：把 {@code grass_pouch_back.geo.json} 的 cube 转写成 vanilla
 * {@link ModelPart}，由 {@link WornPackFeatureRenderer} 用 {@code MatrixStack + RenderLayer} 挂在玩家
 * 躯干上渲染——无 GeckoLib runtime 依赖、headless 可单测、静态模型不需要动画引擎。
 *
 * <p><b>坐标转换（Bedrock geo → vanilla ModelPart，挂躯干局部系）</b>：
 * Bedrock geo 为 y 向上、脚在 y=0；vanilla body {@link ModelPart} 局部系为 y 向下、颈/肩在 y=0
 *（body cuboid {@code addCuboid(-4,0,-2,8,12,4)}，y∈[0,12] 颈→腰）。玩家躯干在 Bedrock 系约
 * y∈[12,24]，故 {@code vanilla_y = 24 - bedrock_y}：bedrock 24（肩）→ vanilla 0（顶），bedrock 12
 *（腰）→ vanilla 12（底）。对一个 cube，addCuboid 取最小角（顶角），其 vanilla_y_min =
 * {@code 24 - (origin_y + size_y)}。x 不变（破草包左右对称、居中）；z 不变——Bedrock geo 与 vanilla
 * body 都是 {@code +z = 背面}（geo 命名 grass_pouch_back 的 cube 在 +z；vanilla body 背面在 z=+2），
 * 故背包件在 z∈[1.5,4.5] 落在躯干背后并微微凸出。texture box-uv 在两边布局一致（GeckoLib 正因此能在
 * vanilla 实体上像素级还原 Bedrock geo），故贴图映射正确。
 *
 * <p><b>bone pivot 不参与定位（pivot 校准复核）</b>：geo 单 bone {@code GrassPouch_Back} 的
 * pivot 为 {@code [0,14,3]}，但**无 rotation 字段**。Bedrock geo 里 cube 的 {@code origin} 是
 * **绝对 model-space 坐标**（与 pivot 同系），pivot 仅是 {@code rotation} 的旋转中心；无旋转时 cube 按
 * origin 原位渲染。故本转换**直接用 cube origin**、不叠加 bone pivot 偏移——这是正确做法，请勿
 * 误把 pivot [0,14,3] 当平移量加回去。pivot 的 y=14 仅作参考：{@code 24-14=10} = vanilla 躯干下-中段，
 * 与本表 cube 落点 y∈[6.5,12.5] 一致（plan §P4「pivot Y=14 torso 中段、Z+3 背面」即此）。
 *
 * <p>转写自 {@code assets/bong/geo/grass_pouch_back.geo.json}（geometry.bong.grass_pouch_back，
 * texture 32×32，单 bone GrassPouch_Back，4 cube）。改 geo 时同步本表 + {@code WornPackModelTest} pin。
 */
public final class WornPackModel {

    /** 贴图边长（与 grass_pouch_back.geo.json 的 texture_width/height 一致）。 */
    public static final int TEXTURE_WIDTH = 32;
    public static final int TEXTURE_HEIGHT = 32;

    /** Bedrock 系玩家躯干顶（肩）对应的高度（脚=0）；vanilla body 局部系顶在 y=0。 */
    public static final float BEDROCK_BODY_TOP = 24.0f;

    /**
     * Bedrock geo cube（直接转写自 grass_pouch_back.geo.json，pixel 单位，未做任何 y 翻转）。
     *
     * @param ox/oy/oz Bedrock origin（最小角，y 向上）
     * @param sx/sy/sz size
     * @param u/v      box-uv 左上角
     */
    public record GeoCube(float ox, float oy, float oz, float sx, float sy, float sz, int u, int v) {}

    /**
     * grass_pouch_back.geo.json 的 4 个 cube（顺序、数值与 geo 一一对应，pin 测试核验）。
     */
    public static final List<GeoCube> GRASS_POUCH_BACK_CUBES = List.of(
        new GeoCube(-3.0f, 11.5f, 1.5f, 6.0f, 5.5f, 3.0f, 0, 0),    // 主袋身
        new GeoCube(-3.2f, 16.5f, 1.0f, 6.4f, 1.0f, 4.0f, 0, 10),   // 袋口翻盖
        new GeoCube(-3.2f, 14.0f, 4.5f, 6.4f, 2.5f, 0.5f, 0, 16),   // 背带横条
        new GeoCube(-0.5f, 14.5f, 4.8f, 1.0f, 1.0f, 0.4f, 18, 0)    // 系绳扣
    );

    private WornPackModel() {}

    /**
     * Bedrock origin_y + size_y → vanilla ModelPart cuboid 的最小角 y（顶角）。
     * {@code vanilla_y_min = 24 - (bedrock_origin_y + size_y)}。
     */
    public static float bedrockToVanillaCuboidY(float bedrockOriginY, float sizeY) {
        return BEDROCK_BODY_TOP - (bedrockOriginY + sizeY);
    }

    /**
     * 烘焙背面破草包为 vanilla {@link ModelPart}（cuboid 坐标已在「挂躯干局部系」，
     * 故 {@link WornPackFeatureRenderer} 进入 body frame 后直接 render 即可，无需额外位移）。
     * 纯 vanilla 数据构造，无 GL / MinecraftClient 依赖，headless 可调用。
     */
    public static ModelPart buildBackModelPart() {
        ModelData modelData = new ModelData();
        ModelPartData root = modelData.getRoot();

        ModelPartBuilder builder = ModelPartBuilder.create();
        for (GeoCube cube : GRASS_POUCH_BACK_CUBES) {
            float vanillaY = bedrockToVanillaCuboidY(cube.oy(), cube.sy());
            builder = builder
                .uv(cube.u(), cube.v())
                .cuboid(cube.ox(), vanillaY, cube.oz(), cube.sx(), cube.sy(), cube.sz());
        }
        root.addChild("grass_pouch_back", builder, ModelTransform.NONE);

        return TexturedModelData.of(modelData, TEXTURE_WIDTH, TEXTURE_HEIGHT).createModel();
    }
}
