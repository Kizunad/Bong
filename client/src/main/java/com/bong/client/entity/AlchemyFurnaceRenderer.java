package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class AlchemyFurnaceRenderer extends BongModeledEntityRenderer {
    public AlchemyFurnaceRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.ALCHEMY_FURNACE);
    }
}
