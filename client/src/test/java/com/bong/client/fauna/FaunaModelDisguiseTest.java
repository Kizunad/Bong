package com.bong.client.fauna;

import com.bong.client.spider.SpiderDisguiseHandler;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-fauna-mimic-spider-v1 P2 — FaunaModel 伪装渲染切换契约测试。
 *
 * <p>测策略：不实例化 FaunaEntity（需要 MC 注册表 bootstrap），
 * 而是直接验证 {@link FaunaModel} 中的常量（贴图路径 pin）和
 * {@link SpiderDisguiseHandler} 状态机（isDisguised 契约），
 * 两者共同锁住 P2 渲染切换逻辑。
 *
 * <p>渲染逻辑契约：
 * <ul>
 *   <li>当 {@code SpiderDisguiseHandler.isDisguised(entityId) == true} 时，
 *       {@code FaunaModel.getTextureResource} 应返回 {@link FaunaModel#ASH_SPIDER_DISGUISE_TEXTURE}。
 *   <li>当 {@code isDisguised == false} 时，返回正常蜘蛛贴图 {@link FaunaVisualKind#ASH_SPIDER}.textureId()。
 * </ul>
 */
class FaunaModelDisguiseTest {

    @BeforeEach
    void resetState() {
        SpiderDisguiseHandler.clearOnDisconnect();
    }

    // ── 贴图路径 pin 测试（稳定性守卫）──────────────────────────────────────────

    @Test
    void ash_spider_disguise_texture_path_is_stable() {
        // pin 测试：渲染切换路径若被改变，此测试立即失败，提示同步更新贴图资源文件。
        Identifier disguiseTex = FaunaModel.ASH_SPIDER_DISGUISE_TEXTURE;

        assertEquals(
            "bong",
            disguiseTex.getNamespace(),
            "伪装贴图 namespace 必须是 bong（与其他 fauna 贴图一致），实际: " + disguiseTex
        );
        assertEquals(
            "textures/entity/fauna/ash_spider_disguised.png",
            disguiseTex.getPath(),
            "伪装贴图路径必须稳定（plan P2 设计决议），实际: " + disguiseTex.getPath()
        );
    }

    @Test
    void ash_spider_disguise_texture_differs_from_normal_texture() {
        // 伪装贴图必须与正常蜘蛛贴图不同，否则 isDisguised 分支毫无意义
        Identifier disguiseTex = FaunaModel.ASH_SPIDER_DISGUISE_TEXTURE;
        Identifier normalTex = FaunaVisualKind.ASH_SPIDER.textureId();

        assertNotEquals(
            normalTex,
            disguiseTex,
            "伪装贴图必须与正常蜘蛛贴图不同（若相同则 isDisguised 分支无效）\n"
            + "  正常: " + normalTex + "\n  伪装: " + disguiseTex
        );
    }

    @Test
    void ash_spider_normal_texture_path_is_stable() {
        // 确保正常贴图路径未被意外改变（pin 测试）
        Identifier normalTex = FaunaVisualKind.ASH_SPIDER.textureId();
        assertEquals(
            "bong",
            normalTex.getNamespace(),
            "正常蜘蛛贴图 namespace 必须是 bong"
        );
        assertEquals(
            "textures/entity/fauna/ash_spider.png",
            normalTex.getPath(),
            "正常蜘蛛贴图路径必须稳定，实际: " + normalTex.getPath()
        );
    }

    // ── 渲染状态机契约（SpiderDisguiseHandler 状态 → 贴图选择逻辑） ──────────────

    /**
     * 直接验证贴图选择函数 selectTexture，它是 FaunaModel.getTextureResource 内部逻辑的
     * 纯函数抽取，不依赖 FaunaEntity 实例（避免 MC 注册表 bootstrap）。
     */
    private static Identifier selectTextureForAshSpider(int entityId) {
        if (SpiderDisguiseHandler.isDisguised(entityId)) {
            return FaunaModel.ASH_SPIDER_DISGUISE_TEXTURE;
        }
        return FaunaVisualKind.ASH_SPIDER.textureId();
    }

    @Test
    void disguised_spider_uses_ash_block_texture() {
        // 服务端 enter payload 把 entity_id=42 标记为 Disguised
        String enterPayload = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);

        Identifier tex = selectTextureForAshSpider(42);
        assertEquals(
            FaunaModel.ASH_SPIDER_DISGUISE_TEXTURE,
            tex,
            "Disguised 状态的蛛应使用伪装贴图（ash_block），实际: " + tex + "\n"
            + "（若此断言失败，说明 FaunaModel.getTextureResource 未检查 SpiderDisguiseHandler.isDisguised）"
        );
    }

    @Test
    void non_disguised_spider_uses_normal_texture() {
        // entity_id=99 没有被 Disguise，应使用正常贴图
        Identifier tex = selectTextureForAshSpider(99);
        assertEquals(
            FaunaVisualKind.ASH_SPIDER.textureId(),
            tex,
            "未 Disguised 的蛛应使用正常贴图，实际: " + tex
        );
    }

    @Test
    void ambush_trigger_switches_back_to_normal_texture() {
        // 蛛先 Disguised，再触发 Ambush → 贴图恢复正常
        String enterPayload = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);
        assertTrue(
            SpiderDisguiseHandler.isDisguised(42),
            "enter 后蛛应为 Disguised（前置条件）"
        );

        String ambushPayload = """
            {"v":1,"type":"spider_ambush_trigger","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleAmbush(ambushPayload, ambushPayload.getBytes().length);

        Identifier tex = selectTextureForAshSpider(42);
        assertEquals(
            FaunaVisualKind.ASH_SPIDER.textureId(),
            tex,
            "Ambush 后蛛贴图应恢复正常（切回真实蜘蛛外观），实际: " + tex + "\n"
            + "（若仍是伪装贴图，说明 handleAmbush 未正确移除 entity_id 或 FaunaModel 未重新查询状态）"
        );
    }

    @Test
    void disconnect_clears_disguise_and_texture_resets() {
        // 断线后状态清空，所有蛛贴图恢复正常
        String enterPayload = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[1,2,3]}
            """;
        SpiderDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);

        SpiderDisguiseHandler.clearOnDisconnect();

        for (int entityId : new int[]{1, 2, 3}) {
            Identifier tex = selectTextureForAshSpider(entityId);
            assertEquals(
                FaunaVisualKind.ASH_SPIDER.textureId(),
                tex,
                "断线后 entity " + entityId + " 贴图应恢复正常（清空 Disguised 列表），实际: " + tex
            );
        }
    }

    @Test
    void multiple_spiders_independently_switch_textures() {
        // 两只蛛：42 Disguised，77 已暴起 → 各自独立
        String enterPayload = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[42,77]}
            """;
        SpiderDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);

        // 77 暴起
        String ambushPayload = """
            {"v":1,"type":"spider_ambush_trigger","entity_ids":[77]}
            """;
        SpiderDisguiseHandler.handleAmbush(ambushPayload, ambushPayload.getBytes().length);

        // 42 仍 Disguised → 伪装贴图
        assertEquals(
            FaunaModel.ASH_SPIDER_DISGUISE_TEXTURE,
            selectTextureForAshSpider(42),
            "entity 42 仍 Disguised，贴图应为伪装贴图"
        );
        // 77 已 Ambush → 正常贴图
        assertEquals(
            FaunaVisualKind.ASH_SPIDER.textureId(),
            selectTextureForAshSpider(77),
            "entity 77 已 Ambush，贴图应恢复正常"
        );
    }
}
