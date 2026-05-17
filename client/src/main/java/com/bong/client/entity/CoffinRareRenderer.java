package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-supply-coffin-v1 — 漆棺渲染器。 */
public final class CoffinRareRenderer extends BongModeledEntityRenderer {
    public CoffinRareRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.COFFIN_RARE);
    }
}
