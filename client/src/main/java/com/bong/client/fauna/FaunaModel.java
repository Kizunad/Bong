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
        if (!usesBaseAssets(entity.playback(), entity.visualKind())) {
            return new Identifier("bong", "textures/entity/fauna/" + entity.playback().profile().path() + PNG_SUFFIX);
        }
        return selectTexture(entity.visualKind(), entity.getId());
    }

    /**
     * 当前 playback 是否仍在该形态的默认 Profile（未切到 hybrid_beast core/shard、
     * fuyu_vulture flight 等子形态）。
     *
     * <p>必须对照 {@link FaunaVisualKind#assetPath()}（默认资源三件套路径），不能对照
     * {@link FaunaVisualKind#path()}（entity 注册 id）——v2 迁移后两者永久不同，用 path
     * 判断会让 DAOXIANG/DEVOUR_RAT 的贴图特判（道伥伪装、噬元鼠吸元档位）永远走不到。
     */
    static boolean usesBaseAssets(FaunaPlayback playback, FaunaVisualKind kind) {
        return playback.profile().path().equals(kind.assetPath());
    }

    @Override
    public Identifier getAnimationResource(FaunaEntity entity) {
        return new Identifier("bong", "animations/" + entity.playback().profile().path() + ".animation.json");
    }
}
