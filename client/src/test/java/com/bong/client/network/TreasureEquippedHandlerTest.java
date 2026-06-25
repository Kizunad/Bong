package com.bong.client.network;

import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.inspect.WeaponTreasurePanel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TreasureEquippedHandlerTest {
    @BeforeEach
    void setUp() {
        TreasureEquippedStore.resetForTests();
        WeaponTreasurePanel.resetForTests();
    }

    @AfterEach
    void tearDown() {
        TreasureEquippedStore.resetForTests();
        WeaponTreasurePanel.resetForTests();
    }

    @Test
    void equipsTreasureSlotAndSyncsPanel() {
        // plan-layered-equip-v1 P4（决议 #8）：treasure_belt_0..3 槽已删；off_hand 是当前唯一同步进战斗
        // HUD 法宝面板的持械法宝槽（激活态后续迁灵宝 UI 触发位，见 plan §P4 TODO）。
        ServerDataDispatch dispatch = new TreasureEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"treasure_equipped","slot":"off_hand",
             "treasure":{"instance_id":42,"template_id":"starter_talisman","display_name":"启程护符"}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        EquippedTreasure treasure = TreasureEquippedStore.get("off_hand");
        assertNotNull(treasure);
        assertEquals("starter_talisman", treasure.templateId());
        assertEquals(1, WeaponTreasurePanel.treasures().size());
        assertEquals("启程护符", WeaponTreasurePanel.treasures().get(0).displayName());
    }

    @Test
    void clearsTreasureSlotWhenFieldAbsent() {
        new TreasureEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"treasure_equipped","slot":"off_hand",
             "treasure":{"instance_id":7,"template_id":"starter_talisman","display_name":"启程护符"}}
            """));

        ServerDataDispatch dispatch = new TreasureEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"treasure_equipped","slot":"off_hand"}
            """));

        assertTrue(dispatch.handled());
        assertNull(TreasureEquippedStore.get("off_hand"));
        assertTrue(WeaponTreasurePanel.treasures().isEmpty());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
