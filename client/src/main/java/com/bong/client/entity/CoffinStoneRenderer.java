package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-coffin-tiers-v1 P3 — 石棺渲染器。 */
public final class CoffinStoneRenderer extends BongModeledEntityRenderer {
    public CoffinStoneRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.COFFIN_STONE);
    }
}
