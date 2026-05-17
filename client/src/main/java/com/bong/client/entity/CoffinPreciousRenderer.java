package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-supply-coffin-v1 — 祭坛棺渲染器。 */
public final class CoffinPreciousRenderer extends BongModeledEntityRenderer {
    public CoffinPreciousRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.COFFIN_PRECIOUS);
    }
}
