package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-placeable-container-blocks-v1 P2 — 放置版灵草箱渲染器。 */
public final class HerbCratePlacedRenderer extends BongModeledEntityRenderer {
    public HerbCratePlacedRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.HERB_CRATE_PLACED);
    }
}
