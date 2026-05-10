package com.bong.client.fauna;

import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

public final class FaunaModel extends GeoModel<FaunaEntity> {
    @Override
    public Identifier getModelResource(FaunaEntity entity) {
        return entity.visualKind().modelId();
    }

    @Override
    public Identifier getTextureResource(FaunaEntity entity) {
        return entity.visualKind().textureId();
    }

    @Override
    public Identifier getAnimationResource(FaunaEntity entity) {
        return entity.visualKind().animationId();
    }
}
