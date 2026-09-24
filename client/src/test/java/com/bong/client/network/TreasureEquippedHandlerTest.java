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

    @Test
    void disconnectClearDropsOldPanelProjectionAndAllowsFreshSync() {
        new TreasureEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"treasure_equipped","slot":"off_hand",
             "treasure":{"instance_id":7,"template_id":"old_talisman","display_name":"旧服护符"}}
            """));
        WeaponTreasurePanel.replaceWeapon(new WeaponTreasurePanel.Weapon(
            "sword", "rusted_iron", "worn", 0.5f
        ));

        WeaponTreasurePanel.clearOnDisconnect();

        assertNull(WeaponTreasurePanel.weapon(),
            "断线必须清空旧服武器 projection，避免 inspect tooltip 跨 session 残留");
        assertTrue(WeaponTreasurePanel.treasures().isEmpty(),
            "断线必须清空旧服法宝 projection，不能等待新服首个 treasure payload 才纠正");

        new TreasureEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"treasure_equipped","slot":"off_hand",
             "treasure":{"instance_id":8,"template_id":"fresh_talisman","display_name":"新服护符"}}
            """));
        assertEquals("新服护符", WeaponTreasurePanel.treasures().get(0).displayName(),
            "生产清理不能变成一次性开关；新 session projection 必须能重新写入");
    }


    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
