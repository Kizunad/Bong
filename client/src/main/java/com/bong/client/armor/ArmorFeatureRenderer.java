package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import com.bong.client.inventory.state.InventoryStateStore;
import net.minecraft.client.MinecraftClient;
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
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * plan-armor-model-render-v1 P0: FeatureRenderer that overlays custom ModelPart armor on the player.
 *
 * <p>Reads equipped armor from {@link InventoryStateStore} and, for each piece whose
 * template_id is registered in {@link ArmorModelRegistry}, renders the corresponding cube table from
 * {@link ArmorPartModel}. Items not in the registry fall through to the vanilla leather dye path.
 *
 * <p>P0-P2 期间正式开关保持关闭，开发环境可用 {@code -Dbong.armor_model_render=true} 强制走完整
 * ModelPart 链路，逐件 F5 校准。P3 定稿后翻开正式开关并由 mixin 抑制已注册甲的皮甲染色兜底。
 */
public final class ArmorFeatureRenderer
    extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {

    /** P3 前保持 false；名称在 P3 与 mixin 一并收敛。 */
    public static final boolean OBJ_RENDER_READY = false;
    static final String DEV_RENDER_PROPERTY = "bong.armor_model_render";

    static final List<EquipSlotType> ARMOR_SLOTS = List.of(
        EquipSlotType.HEAD,
        EquipSlotType.CHEST,
        EquipSlotType.LEGS,
        EquipSlotType.FEET
    );

    record RenderableArmor(EquipSlotType slot, ArmorModelRegistry.ArmorModelSpec spec) {}

    private final Map<String, ModelPart> modelCache = new HashMap<>();

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
        if (!isModelRenderEnabled() || MinecraftClient.getInstance().player != entity) {
            return;
        }

        List<RenderableArmor> renderables = collectRenderable(InventoryStateStore.snapshot().equippedSlots());
        for (RenderableArmor renderable : renderables) {
            ArmorModelRegistry.ArmorModelSpec spec = renderable.spec();
            ModelPart model = modelCache.computeIfAbsent(spec.modelKey(), ArmorPartModel::buildModelPart);
            RenderLayer layer = RenderLayer.getEntityCutoutNoCull(spec.textureId());
            VertexConsumer buffer = vertexConsumers.getBuffer(layer);

            for (ArmorPartModel.Mount mount : ArmorPartModel.mountsForSlot(renderable.slot())) {
                matrices.push();
                contextPart(mount).rotate(matrices);
                model.getChild(mount.childName()).render(
                    matrices,
                    buffer,
                    light,
                    OverlayTexture.DEFAULT_UV,
                    1.0f,
                    1.0f,
                    1.0f,
                    1.0f
                );
                matrices.pop();
            }
        }
    }

    public static boolean isModelRenderEnabled() {
        return OBJ_RENDER_READY || Boolean.getBoolean(DEV_RENDER_PROPERTY);
    }

    static List<RenderableArmor> collectRenderable(Map<EquipSlotType, SlotContents> equippedSlots) {
        List<RenderableArmor> renderables = new ArrayList<>();
        if (equippedSlots == null || equippedSlots.isEmpty()) {
            return renderables;
        }

        for (EquipSlotType slot : ARMOR_SLOTS) {
            SlotContents contents = equippedSlots.get(slot);
            if (contents == null) {
                continue;
            }
            for (InventoryItem item : contents.worn()) {
                if (item.durability() == 0.0) {
                    continue;
                }
                ArmorModelRegistry.get(item.itemId())
                    .filter(spec -> spec.slot() == slot)
                    .ifPresent(spec -> renderables.add(new RenderableArmor(slot, spec)));
            }
        }
        return renderables;
    }

    private ModelPart contextPart(ArmorPartModel.Mount mount) {
        return switch (mount) {
            case HEAD -> getContextModel().head;
            case BODY -> getContextModel().body;
            case LEFT_LEG, LEFT_FOOT -> getContextModel().leftLeg;
            case RIGHT_LEG, RIGHT_FOOT -> getContextModel().rightLeg;
        };
    }
}
