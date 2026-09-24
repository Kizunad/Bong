package com.bong.client.fauna;

import com.bong.client.daozhan.DaoZhanDisguiseHandler;
import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

/** 生物形态决定几何/动画；噬元鼠档位和道伥伪装决定底图。蜘蛛方块由 renderer 绘制。 */
public final class FaunaModel extends GeoModel<FaunaEntity> {
    public static final Identifier DAOZHAN_DISGUISE_PLAYER_TEXTURE =
        new Identifier("minecraft", "textures/entity/player/wide/steve.png");
    public static final String DEVOUR_RAT_TIER_TEXTURE_PREFIX = "textures/entity/fauna/devour_rat_q";
    public static final String GLOW_TEXTURE_SUFFIX = "_glow";
    private static final String PNG_SUFFIX = ".png";

    @Override
    public Identifier getModelResource(FaunaEntity entity) {
        return new Identifier("bong", "geo/" + entity.playback().profile().path() + ".geo.json");
    }

    public static Identifier devourRatTexture(int tier) {
        return new Identifier("bong", DEVOUR_RAT_TIER_TEXTURE_PREFIX + RatQiTierHandler.clampTier(tier) + PNG_SUFFIX);
    }

    public static Identifier glowTextureFor(Identifier base) {
        String path = base.getPath();
        String glowPath = path.endsWith(PNG_SUFFIX)
            ? path.substring(0, path.length() - PNG_SUFFIX.length()) + GLOW_TEXTURE_SUFFIX + PNG_SUFFIX
            : path + GLOW_TEXTURE_SUFFIX;
        return new Identifier(base.getNamespace(), glowPath);
    }

    public static Identifier selectTexture(FaunaVisualKind kind, int entityId) {
        if (kind == FaunaVisualKind.DAOXIANG && DaoZhanDisguiseHandler.isDisguised(entityId)) {
            return DAOZHAN_DISGUISE_PLAYER_TEXTURE;
        }
        if (kind == FaunaVisualKind.DEVOUR_RAT) return devourRatTexture(RatQiTierHandler.tierOf(entityId));
        return kind.textureId();
    }

    @Override
    public Identifier getTextureResource(FaunaEntity entity) {
        if (!entity.playback().profile().path().equals(entity.visualKind().path())) {
            return new Identifier("bong", "textures/entity/fauna/" + entity.playback().profile().path() + PNG_SUFFIX);
        }
        return selectTexture(entity.visualKind(), entity.getId());
    }

    @Override
    public Identifier getAnimationResource(FaunaEntity entity) {
        return new Identifier("bong", "animations/" + entity.playback().profile().path() + ".animation.json");
    }
}
