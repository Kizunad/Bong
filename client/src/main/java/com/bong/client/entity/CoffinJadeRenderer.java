package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-coffin-tiers-v1 P3 — 玉棺渲染器。 */
public final class CoffinJadeRenderer extends BongModeledEntityRenderer {
    public CoffinJadeRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.COFFIN_JADE);
    }
}
