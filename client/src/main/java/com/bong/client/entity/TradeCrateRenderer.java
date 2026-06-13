package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-placeable-container-blocks-v1 P2 — 货箱渲染器。 */
public final class TradeCrateRenderer extends BongModeledEntityRenderer {
    public TradeCrateRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.TRADE_CRATE);
    }
}
