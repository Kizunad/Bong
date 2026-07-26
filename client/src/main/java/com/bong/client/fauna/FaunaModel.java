package com.bong.client.fauna;

import com.bong.client.daozhan.DaoZhanDisguiseHandler;
import com.bong.client.spider.SpiderDisguiseHandler;
import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

/**
 * plan-fauna-mimic-spider-v1 P2 — 拟态灰烬蛛伪装渲染覆盖。
 * plan-daozhan-v1 P1 — 道伥 Mimicry 态玩家皮肤贴图覆盖。
 *
 * <p>getTextureResource 优先检查伪装状态：
 * <ul>
 *   <li>拟态灰烬蛛：{@link SpiderDisguiseHandler#isDisguised} → {@link #ASH_SPIDER_DISGUISE_TEXTURE}。
 *   <li>道伥（Daoxiang）Mimicry 态：{@link DaoZhanDisguiseHandler#isDisguised} →
 *       {@link #DAOZHAN_DISGUISE_PLAYER_TEXTURE}（Steve 皮肤占位，P3 扩展为随机死亡修士皮肤）。
 * </ul>
 *
 * <p>渲染层（GeckoLib）在每帧调用此方法，因此状态切换（enter/reveal CustomPayload）
 * 会在下一渲染帧自动生效，无需额外事件总线。
 *
 * <p>道伥 Mimicry 期显示为无名玩家：nameplate 由 {@link com.bong.client.mixin.MixinEntityRenderer}
 * 控制（非社交 SocialStateStore 暴露的玩家不显示名牌），因此道伥实体不会显示名牌。
 * （实际上道伥为 FaunaEntity，名牌默认为 FaunaVisualKind 的 displayName，由 FaunaRenderer 处理）。
 */
public final class FaunaModel extends GeoModel<FaunaEntity> {

    /**
     * 拟态灰烬蛛 Disguised 期贴图：灰烬方块外观。
     *
     * <p>Path: {@code bong:textures/entity/fauna/ash_spider_disguised.png}。
     * 实际贴图文件由美术按此路径交付；测试中通过常量 pin 确保路径稳定。
     */
    public static final Identifier ASH_SPIDER_DISGUISE_TEXTURE =
        new Identifier("bong", "textures/entity/fauna/ash_spider_disguised.png");

    /**
     * 道伥 Mimicry 期贴图：无名玩家外形（Steve 默认皮肤，WSLg 验收占位）。
     *
     * <p>plan-daozhan-v1 P1 约束：使用 MC 内置 Steve 皮肤纹理（{@code minecraft:textures/entity/player/wide/steve.png}）
     * 替换道伥的 Daoxiang 贴图。渲染时保持 Daoxiang 体型比例——P1 目标是正确的伪装
     * 视觉标记，完整玩家比例模型替换留 P3 扩展。
     *
     * <p>P3 扩展：将此纹理替换为从 server 下发的随机死亡修士皮肤 UUID 对应的玩家皮肤。
     */
    public static final Identifier DAOZHAN_DISGUISE_PLAYER_TEXTURE =
        new Identifier("minecraft", "textures/entity/player/wide/steve.png");

    /**
     * plan-devour-rat-model P2 —— 噬元鼠吸元档位贴图前缀；实际路径为
     * {@code <前缀>{0,1,2}.png}（尾脊蓝填充 0 / 半 / 全）。
     */
    public static final String DEVOUR_RAT_TIER_TEXTURE_PREFIX = "textures/entity/fauna/devour_rat_q";

    /**
     * emissive glow 贴图的后缀：在底图路径的 {@code .png} 前插入 {@code _glow}。
     *
     * <p>注意**不是** GeckoLib {@code AutoGlowingGeoLayer} 的 {@code _glowmask} 约定——
     * 本仓库走自己的 {@link FaunaEmissiveGlowLayer}（{@code getEntityTranslucentEmissive}
     * 整模重绘），glow 贴图直接给出要发光的颜色，而不是"从底图挖色"的掩码。
     */
    public static final String GLOW_TEXTURE_SUFFIX = "_glow";

    private static final String PNG_SUFFIX = ".png";

    @Override
    public Identifier getModelResource(FaunaEntity entity) {
        return entity.visualKind().modelId();
    }

    /**
     * plan-devour-rat-model P2 —— 噬元鼠按吸元档位取贴图变体。
     *
     * <p>档位越界一律夹到 {@code [0, TIER_MAX]}（见 {@link RatQiTierHandler#clampTier}），
     * 脏值不能变成磁盘上不存在的 {@code devour_rat_q7.png} → missing texture 紫黑格。
     */
    public static Identifier devourRatTexture(int tier) {
        return new Identifier(
            "bong",
            DEVOUR_RAT_TIER_TEXTURE_PREFIX + RatQiTierHandler.clampTier(tier) + PNG_SUFFIX
        );
    }

    /**
     * 底图 Identifier → 对应的 emissive glow 贴图 Identifier（{@code xxx.png} →
     * {@code xxx_glow.png}）。
     *
     * <p>路径不以 {@code .png} 结尾时直接追加后缀——保证返回值恒为一个确定路径，
     * 由 {@link FaunaVisualKind#hasEmissiveGlow()} 负责只对备齐 glow 资产的物种挂层。
     */
    public static Identifier glowTextureFor(Identifier base) {
        String path = base.getPath();
        String glowPath = path.endsWith(PNG_SUFFIX)
            ? path.substring(0, path.length() - PNG_SUFFIX.length()) + GLOW_TEXTURE_SUFFIX + PNG_SUFFIX
            : path + GLOW_TEXTURE_SUFFIX;
        return new Identifier(base.getNamespace(), glowPath);
    }

    /**
     * 贴图选择的纯函数核：只依赖 {@link FaunaVisualKind} + MC 协议 entity id
     * （不碰 {@link FaunaEntity} 实例，故可在无 MC bootstrap 的纯 JUnit 里直测）。
     *
     * <p>{@link #getTextureResource} 是它的薄委托——测试直接锁这里，避免测试自己
     * 抄一份"影子实现"而生产逻辑漂走。
     */
    public static Identifier selectTexture(FaunaVisualKind kind, int entityId) {
        // 拟态灰烬蛛：伪装期返回 ash_block 贴图
        if (kind == FaunaVisualKind.ASH_SPIDER && SpiderDisguiseHandler.isDisguised(entityId)) {
            return ASH_SPIDER_DISGUISE_TEXTURE;
        }
        // 道伥：Mimicry 期返回玩家皮肤贴图（无名玩家外形）
        if (kind == FaunaVisualKind.DAOXIANG && DaoZhanDisguiseHandler.isDisguised(entityId)) {
            return DAOZHAN_DISGUISE_PLAYER_TEXTURE;
        }
        // 噬元鼠：按吸元档位选贴图变体（缺条目 = 0 档）
        if (kind == FaunaVisualKind.DEVOUR_RAT) {
            return devourRatTexture(RatQiTierHandler.tierOf(entityId));
        }
        return kind.textureId();
    }

    /**
     * 返回贴图资源 ID。
     *
     * <p>plan-fauna-mimic-spider-v1 P2：若实体是 AshSpider 且处于 Disguised 状态，
     * 返回灰烬方块贴图，client 端看到的是方块外观；
     * Ambush 后 {@link SpiderDisguiseHandler} 从列表移除该 entity_id，
     * 下一帧自动切回正常蜘蛛贴图。
     *
     * <p>plan-daozhan-v1 P1：若实体是 Daoxiang 且处于 Mimicry 伪装状态，
     * 返回 Steve 皮肤贴图（玩家外形占位）；
     * Reveal 后 {@link DaoZhanDisguiseHandler} 移除 entity_id，下一帧切回正常道伥贴图。
     *
     * <p>plan-devour-rat-model P2：噬元鼠按服务端下发的吸元档位
     * （{@link RatQiTierHandler}）选 {@code devour_rat_q{0,1,2}.png}——尾脊蓝填充随
     * 吸到的真元量递增，配合 {@link FaunaEmissiveGlowLayer} 让蓝脊 + 红眼发光。
     *
     * @param entity GeckoLib 渲染的 FaunaEntity 实体
     * @return 贴图资源 Identifier
     */
    @Override
    public Identifier getTextureResource(FaunaEntity entity) {
        return selectTexture(entity.visualKind(), entity.getId());
    }

    @Override
    public Identifier getAnimationResource(FaunaEntity entity) {
        return entity.visualKind().animationId();
    }
}
