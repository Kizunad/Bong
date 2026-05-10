package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class RiftPortalRenderer extends BongModeledEntityRenderer {
    public RiftPortalRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.RIFT_PORTAL);
    }
}
