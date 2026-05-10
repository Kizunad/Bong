package com.bong.client.entity;

import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

public final class BongModeledEntityModel extends GeoModel<BongModeledEntity> {
    private final BongEntityModelKind fallbackKind;

    public BongModeledEntityModel(BongEntityModelKind fallbackKind) {
        this.fallbackKind = fallbackKind;
    }

    @Override
    public Identifier getModelResource(BongModeledEntity entity) {
        return kind(entity).modelResource();
    }

    @Override
    public Identifier getTextureResource(BongModeledEntity entity) {
        int visualState = entity == null ? 0 : entity.visualState();
        return kind(entity).textureForState(visualState);
    }

    @Override
    public Identifier getAnimationResource(BongModeledEntity entity) {
        return kind(entity).animationResource();
    }

    private BongEntityModelKind kind(BongModeledEntity entity) {
        return entity == null ? fallbackKind : entity.modelKind();
    }
}
