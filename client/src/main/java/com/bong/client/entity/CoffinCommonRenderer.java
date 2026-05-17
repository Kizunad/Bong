package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-supply-coffin-v1 — 松木棺渲染器。 */
public final class CoffinCommonRenderer extends BongModeledEntityRenderer {
    public CoffinCommonRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.COFFIN_COMMON);
    }
}
