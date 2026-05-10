package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class StoragePouchRenderer extends BongModeledEntityRenderer {
    public StoragePouchRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.STORAGE_POUCH);
    }
}
