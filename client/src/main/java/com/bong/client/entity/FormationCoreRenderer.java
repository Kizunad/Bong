package com.bong.client.entity;

import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRendererFactory;
import net.minecraft.client.util.math.MatrixStack;

public final class FormationCoreRenderer extends BongModeledEntityRenderer {
    private static final boolean DISABLE_FORMATION_CORE_RENDER = true;

    public FormationCoreRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.FORMATION_CORE);
    }

    @Override
    public void render(
        BongModeledEntity entity,
        float yaw,
        float tickDelta,
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light
    ) {
        if (DISABLE_FORMATION_CORE_RENDER) {
            return;
        }
        super.render(entity, yaw, tickDelta, matrices, vertexConsumers, light);
    }
}
