package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-coffin-tiers-v1 P3 — 铜棺渲染器。 */
public final class CoffinBronzeRenderer extends BongModeledEntityRenderer {
    public CoffinBronzeRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.COFFIN_BRONZE);
    }
}
