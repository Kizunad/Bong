package com.bong.client.fauna;

import net.minecraft.client.render.entity.EntityRendererFactory;
import software.bernie.geckolib.renderer.GeoEntityRenderer;

public final class FaunaRenderer extends GeoEntityRenderer<FaunaEntity> {
    public FaunaRenderer(EntityRendererFactory.Context ctx, FaunaVisualKind visualKind) {
        super(ctx, new FaunaModel());
        this.withScale(visualKind.renderScale());
        this.shadowRadius = visualKind.shadowRadius();
    }
}
