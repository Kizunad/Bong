package com.bong.client.whale;

import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

/** GeckoLib model bindings for {@link WhaleEntity}. 路径锁死 assets/bong/{geo,textures/entity,animations}/whale.* */
public final class WhaleModel extends GeoModel<WhaleEntity> {
    private static final Identifier MODEL = new Identifier("bong", "geo/whale.geo.json");
    private static final Identifier TEXTURE = new Identifier("bong", "textures/entity/whale.png");
    private static final Identifier ANIMATION = new Identifier("bong", "animations/whale.animation.json");

    @Override
    public Identifier getModelResource(WhaleEntity entity) {
        return MODEL;
    }

    @Override
    public Identifier getTextureResource(WhaleEntity entity) {
        return TEXTURE;
    }

    @Override
    public Identifier getAnimationResource(WhaleEntity entity) {
        return ANIMATION;
    }
}
