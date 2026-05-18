package com.bong.client.dandao;

import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

/**
 * plan-dandao-path-v1 P4 -- GeckoLib model for Baolongwang BOSS.
 *
 * <p>Assets: geo/baolongwang.geo.json + textures/entity/baolongwang.png
 * + animations/baolongwang.animation.json (copied from local_models/baolongwang/).
 */
public final class BaolongwangModel extends GeoModel<BaolongwangEntity> {
    private static final Identifier MODEL =
        new Identifier("bong", "geo/baolongwang.geo.json");
    private static final Identifier TEXTURE =
        new Identifier("bong", "textures/entity/baolongwang.png");
    private static final Identifier ANIMATION =
        new Identifier("bong", "animations/baolongwang.animation.json");

    @Override
    public Identifier getModelResource(BaolongwangEntity entity) {
        return MODEL;
    }

    @Override
    public Identifier getTextureResource(BaolongwangEntity entity) {
        return TEXTURE;
    }

    @Override
    public Identifier getAnimationResource(BaolongwangEntity entity) {
        return ANIMATION;
    }
}
