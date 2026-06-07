package com.bong.client.fauna;

import com.bong.client.spider.SpiderDisguiseHandler;
import net.minecraft.util.Identifier;
import software.bernie.geckolib.model.GeoModel;

/**
 * plan-fauna-mimic-spider-v1 P2 — 拟态灰烬蛛伪装渲染覆盖。
 *
 * <p>getTextureResource 优先检查 {@link SpiderDisguiseHandler#isDisguised}：
 * 若该实体处于 Disguised 状态，返回灰烬方块贴图（{@link #ASH_SPIDER_DISGUISE_TEXTURE}）；
 * 否则返回正常蜘蛛贴图。
 *
 * <p>渲染层（GeckoLib）在每帧调用此方法，因此状态切换（enter/ambush CustomPayload）
 * 会在下一渲染帧自动生效，无需额外事件总线。
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
        return entity.visualKind().textureId();
    }

    @Override
    public Identifier getAnimationResource(FaunaEntity entity) {
        return entity.visualKind().animationId();
    }
}
