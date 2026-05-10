package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class ForgeStationRenderer extends BongModeledEntityRenderer {
    public ForgeStationRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.FORGE_STATION);
    }
}
