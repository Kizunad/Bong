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
 * <p>Current implementation is a structured stub: full SML/GL OBJ rendering requires
 * a runtime GL context and baked model access that will be wired in a follow-up PR.
 * The renderer correctly queries the registry and logs render intent per frame.
 */
public final class ArmorFeatureRenderer
    extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {

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
