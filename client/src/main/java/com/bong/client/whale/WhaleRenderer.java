package com.bong.client.whale;

import net.minecraft.client.render.entity.EntityRendererFactory;
import software.bernie.geckolib.renderer.GeoEntityRenderer;

public final class WhaleRenderer extends GeoEntityRenderer<WhaleEntity> {
    /** geo.json 自身尺寸即可，不再 10× 放大。 */
    private static final float WHALE_RENDER_SCALE = 1.0f;

    public WhaleRenderer(EntityRendererFactory.Context ctx) {
        super(ctx, new WhaleModel());
        this.withScale(WHALE_RENDER_SCALE);
        // shadowRadius 跟体型匹配；GeckoLib 默认 0.0 在地面会没有阴影
        this.shadowRadius = 1.5f;
    }
}
