package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.entity.feature.FeatureRendererContext;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Map;
import java.util.Optional;

/**
 * plan-depth-loop-v1 P1: FeatureRenderer that overlays custom OBJ armor models on the player.
 *
 * <p>Reads equipped armor from {@link InventoryStateStore} and, for each piece whose
 * template_id is registered in {@link ArmorModelRegistry}, renders the corresponding
 * OBJ model. Items not in the registry fall through to the vanilla leather dye path
 * handled by {@code MixinPlayerEntityArmor}.
 *
 * <p><b>OBJ 渲染未实装（{@link #OBJ_RENDER_READY}=false）</b>：真实 SML/GL OBJ 渲染需要把烘焙
 * 模型挂到玩家骨骼上并做正确的 pivot/scale 变换，必须在真机 F5 下反复目测调位——属未完成功能。
 * 在它实装前，本 renderer 直接 early-return（不做任何绘制），并由 {@code MixinPlayerEntityArmor}
 * 让已注册的铁/骨甲也走 leather-dye 兜底（材质染色的原版皮甲外形，可见但非专属 OBJ），避免
 * 「穿甲全透明」。OBJ 渲染实装后把 {@link #OBJ_RENDER_READY} 翻 true 即切回 OBJ 路径（mixin 同步
 * 恢复对已注册甲的抑制），无需改其它接线。
 */
public final class ArmorFeatureRenderer
    extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {

    /**
     * OBJ 护甲渲染就绪开关。false=未实装→走 leather-dye 兜底（甲可见）；true=OBJ 已实装→
     * {@code MixinPlayerEntityArmor} 对已注册甲抑制 leather-dye，改由本 renderer 画 OBJ。
     * 翻 true 前请先在 {@link #render} 实装真实绘制，否则甲会回到全透明。
     */
    public static final boolean OBJ_RENDER_READY = false;

    private static final Logger LOGGER = LoggerFactory.getLogger("bong/armor/feature_renderer");

    private static final EquipSlotType[] ARMOR_SLOTS = {
        EquipSlotType.HEAD,
        EquipSlotType.CHEST,
        EquipSlotType.LEGS,
        EquipSlotType.FEET
    };

    public ArmorFeatureRenderer(
        FeatureRendererContext<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> context
    ) {
        super(context);
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
        // OBJ 渲染未实装：直接 early-return，不做任何每帧工作/日志（甲由 leather-dye 兜底，见类注释）。
        // 实装真实 OBJ 绘制后，把 OBJ_RENDER_READY 翻 true，下面的循环才会执行。
        if (!OBJ_RENDER_READY) return;

        Map<EquipSlotType, InventoryItem> equipped = InventoryStateStore.snapshot().equipped();

        for (EquipSlotType slot : ARMOR_SLOTS) {
            InventoryItem item = equipped.get(slot);
            if (item == null || item.isEmpty()) continue;

            Optional<ArmorModelRegistry.ArmorModelSpec> spec = ArmorModelRegistry.get(item.itemId());
            if (spec.isEmpty()) continue;

            ArmorModelRegistry.ArmorModelSpec armorSpec = spec.get();
            // TODO: wire SML baked model lookup + OBJ rendering when GL context is available
            LOGGER.debug("armor_obj_render: slot={} template={} model={}",
                slot, armorSpec.templateId(), armorSpec.modelPath());
        }
    }
}
