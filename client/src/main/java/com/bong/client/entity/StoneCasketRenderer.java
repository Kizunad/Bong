package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class StoneCasketRenderer extends BongModeledEntityRenderer {
    public StoneCasketRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.STONE_CASKET);
    }
}
