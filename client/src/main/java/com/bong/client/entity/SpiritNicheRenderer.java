package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class SpiritNicheRenderer extends BongModeledEntityRenderer {
    public SpiritNicheRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.SPIRIT_NICHE);
    }
}
