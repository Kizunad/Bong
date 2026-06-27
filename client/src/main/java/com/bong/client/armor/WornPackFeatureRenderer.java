package com.bong.client.armor;

import com.bong.client.inventory.InventoryEquipRules;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
import com.bong.client.inventory.state.InventoryStateStore;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.entity.feature.FeatureRendererContext;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-tarkov-backpack-v1 P4 — 穿戴背包件（破草包）上身渲染 FeatureRenderer（TPV / F5，spike 路线 (b)）。
 *
 * <p>每帧读 {@link InventoryStateStore} 的 CHEST 穿戴层（worn 栈），**按容器件（container_spec != null）
 * 过滤**——客户端经 {@link InventoryEquipRules#isContainerPublic} 实现该判定（白名单镜像 server
 * container_spec），**不取 category**（category 是 display 分类，会误纳护甲 / 伪皮）。命中的背包件按
 * {@link WornPackModelRegistry} 取贴图 + 模型件，用纯 vanilla {@link ModelPart} 挂在玩家躯干背后渲染。
 *
 * <p>渲染路线 (b) 的 spike 结论与坐标转换见 {@link WornPackModel} 类注释。**严格参
 * {@code ArmorRenderBootstrap} 注册（唯一已验证可用），不参 {@code MutationFeatureRenderer}（未注册孤岛）**。
 *
 * <p>注册见 {@link WornPackRenderBootstrap}。仅 TPV——FPV 看不到自己背包（vanilla 行为正确，无 FPV 入口）。
 */
public final class WornPackFeatureRenderer
    extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {

    /**
     * F5 真机微调位移（pixel；body 局部系）。geo 已按玩家比例转写，默认 0；若 F5 目测需贴合躯干背面
     * 再调（用户验收阶段）。
     */
    private static final float OFFSET_X = 0.0f;
    private static final float OFFSET_Y = 0.0f;
    private static final float OFFSET_Z = 0.0f;

    /** 懒加载缓存的背面破草包 ModelPart（render 线程单线程，无并发问题）。 */
    private ModelPart backModel;

    public WornPackFeatureRenderer(
        FeatureRendererContext<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> context
    ) {
        super(context);
    }

    /**
     * 纯逻辑过滤谓词（headless 可单测）：从 CHEST 穿戴层挑出**可上身渲染的容器件**对应的渲染规格。
     *
     * <p>仅纳入 {@code container_spec != null}（= {@link InventoryEquipRules#isContainerPublic}）且在
     * {@link WornPackModelRegistry} 注册的件。护甲 / 伪皮 / 非容器件**不会**被本 renderer 拾取。
     *
     * @param chest CHEST 槽 worn 栈内容（可 null）
     * @return 待渲染的背包件规格列表（栈底→栈顶顺序），无则空表
     */
    static List<WornPackModelRegistry.WornPackModelSpec> collectRenderable(SlotContents chest) {
        List<WornPackModelRegistry.WornPackModelSpec> out = new ArrayList<>();
        if (chest == null) {
            return out;
        }
        for (InventoryItem item : chest.worn()) {
            if (!InventoryEquipRules.isContainerPublic(item)) {
                continue; // 非容器件（护甲 / 伪皮）——本 renderer 不拾取
            }
            WornPackModelRegistry.get(item.itemId()).ifPresent(out::add);
        }
        return out;
    }

    @Override
    public void render(
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        AbstractClientPlayerEntity entity,
        float limbAngle,
        float limbDistance,
        float tickDelta,
        float animationProgress,
        float headYaw,
        float headPitch
    ) {
        InventoryModel snapshot = InventoryStateStore.snapshot();
        if (snapshot == null) {
            return;
        }
        SlotContents chest = snapshot.equippedSlots().get(EquipSlotType.CHEST);
        List<WornPackModelRegistry.WornPackModelSpec> specs = collectRenderable(chest);
        if (specs.isEmpty()) {
            return; // 无穿戴背包件——零开销跳过
        }

        if (backModel == null) {
            backModel = WornPackModel.buildBackModelPart();
        }

        for (WornPackModelRegistry.WornPackModelSpec spec : specs) {
            // 本阶段仅 BACK 变体（front 留 P5），统一用背面 ModelPart。
            RenderLayer renderLayer = RenderLayer.getEntityCutoutNoCull(spec.textureId());
            VertexConsumer buffer = vertexConsumers.getBuffer(renderLayer);

            matrices.push();
            // 进入躯干局部系：背包件随躯干 pitch（游泳 / 潜行）一同摆动。
            getContextModel().body.rotate(matrices);
            matrices.translate(OFFSET_X / 16.0f, OFFSET_Y / 16.0f, OFFSET_Z / 16.0f);
            backModel.render(matrices, buffer, light, OverlayTexture.DEFAULT_UV, 1.0f, 1.0f, 1.0f, 1.0f);
            matrices.pop();
        }
    }
}
