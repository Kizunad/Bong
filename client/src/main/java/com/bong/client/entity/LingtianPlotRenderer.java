package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class LingtianPlotRenderer extends BongModeledEntityRenderer {
    public LingtianPlotRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.LINGTIAN_PLOT);
    }
}
