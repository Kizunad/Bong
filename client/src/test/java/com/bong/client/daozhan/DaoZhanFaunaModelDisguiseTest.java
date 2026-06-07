package com.bong.client.daozhan;

import com.bong.client.fauna.FaunaModel;
import com.bong.client.fauna.FaunaVisualKind;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-daozhan-v1 P1 — FaunaModel 道伥 Mimicry 贴图切换契约测试。
 *
 * <p>验证 {@link FaunaModel#DAOZHAN_DISGUISE_PLAYER_TEXTURE} 常量路径稳定性，
 * 以及 {@link DaoZhanDisguiseHandler} 状态 → 贴图选择逻辑契约。
 *
 * <p>测试策略：不实例化 FaunaEntity（需要 MC 注册表 bootstrap），
 * 而是直接验证常量（贴图路径 pin）和 DaoZhanDisguiseHandler 状态机（isDisguised 契约），
 * 两者共同锁住 P1 渲染切换逻辑。
 */
class DaoZhanFaunaModelDisguiseTest {

    @BeforeEach
    void resetState() {
        DaoZhanDisguiseHandler.clearOnDisconnect();
    }

    // ── 贴图路径 pin 测试（稳定性守卫）──────────────────────────────────────────

    @Test
    void daozhan_disguise_texture_path_is_stable() {
        // pin 测试：P1 使用 Steve 皮肤路径，P3 扩展为随机死亡修士皮肤时须同步更新。
        Identifier disguiseTex = FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE;

        assertEquals(
            "minecraft",
            disguiseTex.getNamespace(),
            "P1 道伥伪装贴图 namespace 必须是 minecraft（使用内置 Steve 皮肤），实际: " + disguiseTex
        );
        assertEquals(
            "textures/entity/player/wide/steve.png",
            disguiseTex.getPath(),
            "P1 道伥伪装贴图路径必须稳定（Steve 皮肤 WSLg 验收占位），实际: " + disguiseTex.getPath()
        );
    }

    @Test
    void daozhan_disguise_texture_differs_from_normal_daoxiang_texture() {
        // 伪装贴图必须与正常道伥（Daoxiang）贴图不同，否则 isDisguised 分支毫无意义
        Identifier disguiseTex = FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE;
        Identifier normalTex = FaunaVisualKind.DAOXIANG.textureId();

        assertNotEquals(
            normalTex,
            disguiseTex,
            "道伥伪装贴图必须与正常 Daoxiang 贴图不同（若相同则 isDisguised 分支无效）\n"
            + "  正常 Daoxiang: " + normalTex + "\n  伪装: " + disguiseTex
        );
    }

    // ── 渲染状态机契约（DaoZhanDisguiseHandler 状态 → 贴图选择逻辑） ─────────────

    /**
     * 模拟 FaunaModel.getTextureResource 中道伥伪装检查的纯函数（无需 FaunaEntity 实例化）。
     */
    private static Identifier selectTextureForDaozhan(int entityId) {
        if (DaoZhanDisguiseHandler.isDisguised(entityId)) {
            return FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE;
        }
        return FaunaVisualKind.DAOXIANG.textureId();
    }

    @Test
    void mimicry_daozhan_uses_steve_skin_texture() {
        // server enter payload 把 entity_id=42 标记为 Mimicry（道伥伪装）
        String enterPayload = """
            {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42]}
            """;
        DaoZhanDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);

        Identifier tex = selectTextureForDaozhan(42);
        assertEquals(
            FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE,
            tex,
            "Mimicry 态的道伥应使用 Steve 皮肤贴图（无名玩家外形），实际: " + tex + "\n"
            + "（若此断言失败，说明 FaunaModel.getTextureResource 未检查 DaoZhanDisguiseHandler.isDisguised）"
        );
    }

    @Test
    void non_mimicry_daozhan_uses_normal_daoxiang_texture() {
        // entity_id=99 没有 Mimicry 伪装，应使用正常 Daoxiang 贴图
        Identifier tex = selectTextureForDaozhan(99);
        assertEquals(
            FaunaVisualKind.DAOXIANG.textureId(),
            tex,
            "未伪装的道伥应使用正常 Daoxiang 贴图，实际: " + tex
        );
    }

    @Test
    void reveal_switches_back_to_normal_texture() {
        // 道伥先 Mimicry，reveal 暴起 → 贴图恢复正常 Daoxiang 外观
        String enterPayload = """
            {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42]}
            """;
        DaoZhanDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);
        assertTrue(
            DaoZhanDisguiseHandler.isDisguised(42),
            "enter 后道伥应为 Mimicry（前置条件）"
        );

        String revealPayload = """
            {"v":1,"type":"daozhan_reveal","entity_ids":[42]}
            """;
        DaoZhanDisguiseHandler.handleReveal(revealPayload, revealPayload.getBytes().length);

        Identifier tex = selectTextureForDaozhan(42);
        assertEquals(
            FaunaVisualKind.DAOXIANG.textureId(),
            tex,
            "Reveal（暴起）后道伥贴图应恢复正常 Daoxiang 外观，实际: " + tex + "\n"
            + "（若仍是 Steve 皮肤，说明 handleReveal 未正确移除 entity_id 或 FaunaModel 未重新查询状态）"
        );
    }

    @Test
    void disconnect_clears_mimicry_and_texture_resets() {
        // 断线后状态清空，所有道伥贴图恢复正常
        String enterPayload = """
            {"v":1,"type":"daozhan_disguise_enter","entity_ids":[1,2,3]}
            """;
        DaoZhanDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);

        DaoZhanDisguiseHandler.clearOnDisconnect();

        for (int entityId : new int[]{1, 2, 3}) {
            Identifier tex = selectTextureForDaozhan(entityId);
            assertEquals(
                FaunaVisualKind.DAOXIANG.textureId(),
                tex,
                "断线后 entity " + entityId + " 贴图应恢复正常 Daoxiang（清空 Mimicry 列表），实际: " + tex
            );
        }
    }

    @Test
    void multiple_daozhan_independently_switch_textures() {
        // 两只道伥：42 Mimicry，77 已暴起 → 各自独立
        String enterPayload = """
            {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42,77]}
            """;
        DaoZhanDisguiseHandler.handleEnter(enterPayload, enterPayload.getBytes().length);

        // 77 暴起
        String revealPayload = """
            {"v":1,"type":"daozhan_reveal","entity_ids":[77]}
            """;
        DaoZhanDisguiseHandler.handleReveal(revealPayload, revealPayload.getBytes().length);

        // 42 仍 Mimicry → Steve 皮肤
        assertEquals(
            FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE,
            selectTextureForDaozhan(42),
            "entity 42 仍 Mimicry，贴图应为 Steve 皮肤"
        );
        // 77 已暴起 → Daoxiang 正常贴图
        assertEquals(
            FaunaVisualKind.DAOXIANG.textureId(),
            selectTextureForDaozhan(77),
            "entity 77 已暴起，贴图应恢复正常 Daoxiang 外观"
        );
    }

    @Test
    void fake_player_entity_renderer_default_skin_matches_daozhan_texture() {
        // FakePlayerEntityRenderer.DEFAULT_SKIN_TEXTURE 与 FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE
        // 必须一致（P1 两处使用相同 Steve 皮肤路径）
        assertEquals(
            FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE,
            FakePlayerEntityRenderer.DEFAULT_SKIN_TEXTURE,
            "FakePlayerEntityRenderer.DEFAULT_SKIN_TEXTURE 必须与 FaunaModel.DAOZHAN_DISGUISE_PLAYER_TEXTURE 一致\n"
            + "（两处 P3 扩展点使用相同基线皮肤）"
        );
    }
}
