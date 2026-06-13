package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-placeable-container-blocks-v1 P2 — 死信箱渲染器。 */
public final class DeadDropBoxRenderer extends BongModeledEntityRenderer {
    public DeadDropBoxRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.DEAD_DROP_BOX);
    }
}
