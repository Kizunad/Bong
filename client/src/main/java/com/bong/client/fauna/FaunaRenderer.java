package com.bong.client.fauna;

import net.minecraft.client.render.entity.EntityRendererFactory;
import software.bernie.geckolib.renderer.GeoEntityRenderer;

public final class FaunaRenderer extends GeoEntityRenderer<FaunaEntity> {
    public FaunaRenderer(EntityRendererFactory.Context ctx, FaunaVisualKind visualKind) {
        super(ctx, new FaunaModel());
        this.withScale(visualKind.renderScale());
        this.shadowRadius = visualKind.shadowRadius();
        // plan-devour-rat-model P2 —— 只给备齐 `<底图>_glow.png` 的物种挂 emissive 层；
        // 缺资产的物种挂了会渲染 missing texture（紫黑格）盖住整只怪。
        if (visualKind.hasEmissiveGlow()) {
            this.addRenderLayer(new FaunaEmissiveGlowLayer(this));
        }
    }
}
