package com.bong.client.network;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class WeaponEquippedHandlerTest {
    @BeforeEach
    void setUp() { WeaponEquippedStore.resetForTests(); }
    @AfterEach
    void tearDown() { WeaponEquippedStore.resetForTests(); }

    @Test
    void equipsMainHandWeapon() {
        ServerDataDispatch dispatch = new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"main_hand",
             "weapon":{"instance_id":42,"template_id":"iron_sword",
                       "weapon_kind":"sword","durability_current":185.0,
                       "durability_max":200.0,"quality_tier":0}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        EquippedWeapon w = WeaponEquippedStore.get("main_hand");
        assertNotNull(w);
        assertEquals(42L, w.instanceId());
        assertEquals("iron_sword", w.templateId());
        assertEquals("sword", w.weaponKind());
        assertEquals(185.0f, w.durabilityCurrent(), 1e-5);
        assertEquals(200.0f, w.durabilityMax(), 1e-5);
        assertEquals(0, w.qualityTier());
        assertEquals(0.925f, w.durabilityRatio(), 1e-5);
    }

    @Test
    void clearsSlotWhenWeaponFieldAbsent() {
        // 先装备,再清空
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"main_hand",
             "weapon":{"instance_id":42,"template_id":"iron_sword",
                       "weapon_kind":"sword","durability_current":200.0,
                       "durability_max":200.0,"quality_tier":0}}
            """));
        assertNotNull(WeaponEquippedStore.get("main_hand"));

        ServerDataDispatch dispatch = new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"main_hand"}
            """));
        assertTrue(dispatch.handled());
        assertNull(WeaponEquippedStore.get("main_hand"));
    }

    @Test
    void differentSlotsCoexist() {
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"main_hand",
             "weapon":{"instance_id":1,"template_id":"iron_sword",
                       "weapon_kind":"sword","durability_current":200.0,
                       "durability_max":200.0,"quality_tier":0}}
            """));
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":2,"template_id":"bone_dagger",
                       "weapon_kind":"dagger","durability_current":120.0,
                       "durability_max":120.0,"quality_tier":0}}
            """));

        assertEquals("iron_sword", WeaponEquippedStore.get("main_hand").templateId());
        assertEquals("bone_dagger", WeaponEquippedStore.get("off_hand").templateId());
    }

    @Test
    void durabilityRatioClampsToRange() {
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"main_hand",
             "weapon":{"instance_id":1,"template_id":"odd","weapon_kind":"sword",
                       "durability_current":500.0,"durability_max":100.0,"quality_tier":0}}
            """));
        // overflow → clamp to 1.0(健壮性:server 不应发这种,但 client 兜底)
        assertEquals(1.0f, WeaponEquippedStore.get("main_hand").durabilityRatio(), 1e-5);
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
