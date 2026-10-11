package com.bong.client.fauna;

import com.bong.client.daozhan.DaoZhanDisguiseHandler;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 生物接线第二阶段任务 1——7 只新 v2 模型里 6 只切到 {@code <id>_v2} 资源三件套的契约测试。
 *
 * <p>锁住两件事：
 * <ul>
 *   <li>entity 注册 id / raw id（跨端协议契约）不随资源切换变化；
 *   <li>切到 v2 后，{@link FaunaModel#getTextureResource} 的"是否走默认 Profile"判断不能用
 *       entity id 路径误判——否则 DAOXIANG 的道伥伪装特判（Steve 皮肤）永远走不到（回归见
 *       {@link FaunaModel#usesBaseAssets}的 javadoc）。
 * </ul>
 *
 * <p>ASH_SPIDER 暂留 v1（v2 动画缺 "fold"，蜘蛛伪装重新折叠会卡在 ambush_burst 之后永久可见），
 * 本测试同时锁住这条"暂不切换"的决定，防止后续误以为漏做而随手改掉。
 */
class FaunaV2AssetSwitchTest {

    private static final FaunaVisualKind[] SWITCHED_TO_V2 = {
        FaunaVisualKind.VOID_DISTORTED,
        FaunaVisualKind.DAOXIANG,
        FaunaVisualKind.ZHINIAN,
        FaunaVisualKind.TSY_SENTINEL,
        FaunaVisualKind.FUYA,
        FaunaVisualKind.SKULL_FIEND,
    };

    @AfterEach
    void reset() {
        DaoZhanDisguiseHandler.clearOnDisconnect();
    }

    @Test
    void switchedKindsPointGeoTextureAnimationAtV2AssetPath() {
        for (FaunaVisualKind kind : SWITCHED_TO_V2) {
            String expected = kind.path() + "_v2";
            assertEquals("geo/" + expected + ".geo.json", kind.modelId().getPath(),
                kind + " 的 modelId 应指向 v2 几何");
            assertEquals("textures/entity/fauna/" + expected + ".png", kind.textureId().getPath(),
                kind + " 的 textureId 应指向 v2 贴图");
            assertEquals("animations/" + expected + ".animation.json", kind.animationId().getPath(),
                kind + " 的 animationId 应指向 v2 动画文件");
        }
    }

    @Test
    void switchedKindsKeepEntityIdAndRawIdOnV1Path() {
        for (FaunaVisualKind kind : SWITCHED_TO_V2) {
            assertEquals(new Identifier("bong", kind.path()), kind.entityId(),
                kind + " 的 entity 注册 id 不能随资源切换变化（跨端协议契约）");
            assertTrue(kind.path() != null && !kind.path().endsWith("_v2"),
                kind + " 的 path() 仍须是不带 _v2 的协议 id");
        }
    }

    @Test
    void ashSpiderStaysOnV1BecauseV2AnimationMissesFold() {
        assertEquals("geo/ash_spider.geo.json", FaunaVisualKind.ASH_SPIDER.modelId().getPath());
        assertEquals("textures/entity/fauna/ash_spider.png", FaunaVisualKind.ASH_SPIDER.textureId().getPath());
        assertEquals("animations/ash_spider.animation.json", FaunaVisualKind.ASH_SPIDER.animationId().getPath());
    }

    @Test
    void defaultProfileOfEverySwitchedKindUsesBaseAssets() {
        // 未触发任何子形态切换（hybrid_beast core/shard、fuyu_vulture flight 那类）时，
        // playback 的默认 Profile 必须等于 kind 自己的 assetPath，否则 getTextureResource
        // 会误判成"已切到子形态"，直接绕开 selectTexture 里的特判分支。
        for (FaunaVisualKind kind : SWITCHED_TO_V2) {
            var playback = new FaunaPlayback(kind);
            assertTrue(FaunaModel.usesBaseAssets(playback, kind),
                kind + " 的默认 Profile 应判定为走 selectTexture 特判分支");
        }
    }

    @Test
    void daoxiangMimicryStillOverridesV2TextureWithSteveSkin() {
        // 回归锚点：daoxiang 切 v2 前，getTextureResource 的"是否走默认 Profile"判断若对照
        // entity id 路径（"daoxiang"）而不是 assetPath（"daoxiang_v2"），伪装特判永远不可达。
        var playback = new FaunaPlayback(FaunaVisualKind.DAOXIANG);
        assertTrue(FaunaModel.usesBaseAssets(playback, FaunaVisualKind.DAOXIANG));

        int entityId = 4242;
        assertEquals(FaunaVisualKind.DAOXIANG.textureId(),
            FaunaModel.selectTexture(FaunaVisualKind.DAOXIANG, entityId),
            "未伪装时应显示 v2 道伥贴图");

        String enter = "{\"v\":1,\"type\":\"daozhan_disguise_enter\",\"entity_ids\":[" + entityId + "]}";
        assertTrue(DaoZhanDisguiseHandler.handleEnter(enter, enter.length()));
        assertEquals(FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE,
            FaunaModel.selectTexture(FaunaVisualKind.DAOXIANG, entityId),
            "Mimicry 态切 v2 后仍应显示 Steve 皮肤，而不是 daoxiang_v2 贴图");
    }
}
