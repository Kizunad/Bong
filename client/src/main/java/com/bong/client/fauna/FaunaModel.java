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

    @Override
    public Identifier getModelResource(FaunaEntity entity) {
        return entity.visualKind().modelId();
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
     * @param entity GeckoLib 渲染的 FaunaEntity 实体
     * @return 贴图资源 Identifier
     */
    @Override
    public Identifier getTextureResource(FaunaEntity entity) {
        // 拟态灰烬蛛：伪装期返回 ash_block 贴图
        if (entity.visualKind() == FaunaVisualKind.ASH_SPIDER
                && SpiderDisguiseHandler.isDisguised(entity.getId())) {
            return ASH_SPIDER_DISGUISE_TEXTURE;
        }
        // 道伥：Mimicry 期返回玩家皮肤贴图（无名玩家外形）
        // plan-daozhan-v1 P1：GeckoLib 使用 Daoxiang 体型模型 + Steve 皮肤 → 近似"无名玩家"视觉
        // WSLg 验收标注：外形为 Daoxiang 比例，P3 扩展为真实玩家比例 FakePlayerEntity 渲染
        if (entity.visualKind() == FaunaVisualKind.DAOXIANG
                && DaoZhanDisguiseHandler.isDisguised(entity.getId())) {
            return DAOZHAN_DISGUISE_PLAYER_TEXTURE;
        }
        return entity.visualKind().textureId();
    }

    @Override
    public Identifier getAnimationResource(FaunaEntity entity) {
        return entity.visualKind().animationId();
    }
}
