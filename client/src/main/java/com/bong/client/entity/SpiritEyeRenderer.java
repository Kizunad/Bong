package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class SpiritEyeRenderer extends BongModeledEntityRenderer {
    public SpiritEyeRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.SPIRIT_EYE);
    }
}
