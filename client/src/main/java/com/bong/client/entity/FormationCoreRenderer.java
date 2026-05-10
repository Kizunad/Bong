package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class FormationCoreRenderer extends BongModeledEntityRenderer {
    public FormationCoreRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.FORMATION_CORE);
    }
}
