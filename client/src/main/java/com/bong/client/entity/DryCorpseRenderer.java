package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class DryCorpseRenderer extends BongModeledEntityRenderer {
    public DryCorpseRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.DRY_CORPSE);
    }
}
