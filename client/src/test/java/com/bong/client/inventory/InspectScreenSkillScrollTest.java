package com.bong.client.inventory;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InspectScreenSkillScrollTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        SkillSetStore.resetForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void rejectsNonSkillScroll() {
        InspectScreen screen = new InspectScreen(com.bong.client.inventory.model.InventoryModel.empty());
        var item = com.bong.client.inventory.model.InventoryItem.createFull(
            1001L,
            "recipe_scroll_qixue_pill",
            "丹方残卷·气血丹",
            1, 1, 0.05, "uncommon", "古拙纸页", 1, 1.0, 1.0
        );

        assertFalse(screen.tryLearnSkillScroll(item));
        assertEquals("此物非 skill，不可入", screen.debugSkillScrollDropFeedback());
    }

    @Test
    void rejectsAlreadyConsumedSkillScroll() {
        SkillSetStore.replace(SkillSetSnapshot.of(
            java.util.Map.of(),
            java.util.Set.of("skill_scroll_herbalism_baicao_can")
        ));
        InspectScreen screen = new InspectScreen(com.bong.client.inventory.model.InventoryModel.empty());
        var item = com.bong.client.inventory.model.InventoryItem.createFullWithScrollMeta(
            1002L,
            "skill_scroll_herbalism_baicao_can",
            "《百草图考·残》",
            1, 2, 0.05, "uncommon", "残页只存草木形气", 1, 1.0, 1.0,
            "skill_scroll", "herbalism", 500
        );

        assertFalse(screen.tryLearnSkillScroll(item));
        assertEquals("此卷已悟", screen.debugSkillScrollDropFeedback());
    }

    @Test
    void sendsLearnSkillScrollForKnownFreshScroll() {
        install();
        InspectScreen screen = new InspectScreen(com.bong.client.inventory.model.InventoryModel.empty());
        var item = com.bong.client.inventory.model.InventoryItem.createFullWithScrollMeta(
            1003L,
            "skill_scroll_alchemy_danhuo_can",
            "《丹火候论·残》",
            1, 2, 0.05, "uncommon", "断简残章", 1, 1.0, 1.0,
            "skill_scroll", "alchemy", 500
        );

        assertTrue(screen.tryLearnSkillScroll(item));
        assertEquals("已送出顿悟请求", screen.debugSkillScrollDropFeedback());
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"learn_skill_scroll\",\"v\":1,\"instance_id\":1003}",
            sent.get(0).body()
        );
    }

    @Test
    void rejectsUnknownSkillIdScroll() {
        InspectScreen screen = new InspectScreen(com.bong.client.inventory.model.InventoryModel.empty());
        var item = com.bong.client.inventory.model.InventoryItem.createFullWithScrollMeta(
            1004L,
            "skill_scroll_future_unknown",
            "《无名技残卷》",
            1, 2, 0.05, "uncommon", "旧纸不识其门", 1, 1.0, 1.0,
            "skill_scroll", "future_unknown", 500
        );

        assertFalse(screen.tryLearnSkillScroll(item));
        assertEquals("不识此技，暂不能悟", screen.debugSkillScrollDropFeedback());
    }
}
