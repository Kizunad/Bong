package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

public final class BoneSkeletonRenderer extends BongModeledEntityRenderer {
    public BoneSkeletonRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.BONE_SKELETON);
    }
}
