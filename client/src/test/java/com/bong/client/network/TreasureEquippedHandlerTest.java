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
        ServerDataDispatch dispatch = new TreasureEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"treasure_equipped","slot":"treasure_belt_0",
             "treasure":{"instance_id":42,"template_id":"starter_talisman","display_name":"启程护符"}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        EquippedTreasure treasure = TreasureEquippedStore.get("treasure_belt_0");
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
