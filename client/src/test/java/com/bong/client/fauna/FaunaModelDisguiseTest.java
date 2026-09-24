package com.bong.client.fauna;

import com.bong.client.spider.SpiderDisguiseHandler;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** 方块伪装替代旧贴图覆盖；保留真实消息→每实体渲染状态契约，移除失效的旧路径 pin。 */
class FaunaModelDisguiseTest {
    @AfterEach
    void reset() { SpiderDisguiseHandler.clearOnDisconnect(); }

    @Test
    void networkDisguiseAndAmbushSelectIndependentShapesWithNormalSpiderAtlas() {
        String enter = "{\"v\":1,\"type\":\"spider_disguise_enter\",\"entity_ids\":[42,77]}";
        assertTrue(SpiderDisguiseHandler.handleEnter(enter, enter.length()));
        var hidden = new FaunaPlayback(FaunaVisualKind.ASH_SPIDER);
        var revealed = new FaunaPlayback(FaunaVisualKind.ASH_SPIDER);
        hidden.syncSpiderDisguise(42);
        revealed.syncSpiderDisguise(77);
        String ambush = "{\"v\":1,\"type\":\"spider_ambush_trigger\",\"entity_ids\":[77]}";
        assertTrue(SpiderDisguiseHandler.handleAmbush(ambush, ambush.length()));
        hidden.syncSpiderDisguise(42);
        revealed.syncSpiderDisguise(77);
        assertTrue(hidden.blockDisguise());
        assertFalse(revealed.blockDisguise());
        assertEquals("ambush_burst", FaunaAnimations.shortName(revealed.actionName()));
        assertEquals(FaunaVisualKind.ASH_SPIDER.textureId(), FaunaModel.selectTexture(FaunaVisualKind.ASH_SPIDER, 42),
            "折叠/暴起时仍用蜘蛛图集，伪装方块由独立渲染分支处理");
        SpiderDisguiseHandler.clearOnDisconnect();
        var nextSession = new FaunaPlayback(FaunaVisualKind.ASH_SPIDER);
        nextSession.syncSpiderDisguise(42);
        assertFalse(nextSession.blockDisguise(), "跨会话不能继承旧实体 ID 的伪装");
    }
}
