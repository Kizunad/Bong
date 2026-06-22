package com.bong.client.network;

import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class WeaponEquippedHandlerTest {
    @BeforeEach
    void setUp() {
        WeaponEquippedStore.resetForTests();
        EquippedShieldStore.resetForTests();
    }
    @AfterEach
    void tearDown() {
        WeaponEquippedStore.resetForTests();
        EquippedShieldStore.resetForTests();
    }

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
    void supportsTwoHandSlot() {
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"two_hand",
             "weapon":{"instance_id":3,"template_id":"wooden_staff",
                       "weapon_kind":"staff","durability_current":149.5,
                       "durability_max":150.0,"quality_tier":0}}
            """));

        EquippedWeapon weapon = WeaponEquippedStore.get("two_hand");
        assertNotNull(weapon);
        assertEquals("wooden_staff", weapon.templateId());
        assertEquals("staff", weapon.weaponKind());
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

    // ---- plan-shield-block-v1 §P3：盾牌装备路径 -------------------------

    @Test
    void woodenShield_offHand_writesEquippedShieldStore_notWeaponStore() {
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":10,"template_id":"wooden_shield",
                       "weapon_kind":"shield","durability_current":100.0,
                       "durability_max":100.0,"quality_tier":0}}
            """));

        // 盾应写入 EquippedShieldStore
        assertNotNull(EquippedShieldStore.snapshot(),
            "wooden_shield 装备到 off_hand 应写入 EquippedShieldStore");
        assertEquals("wooden_shield", EquippedShieldStore.snapshot().templateId());
        assertEquals(10L, EquippedShieldStore.snapshot().instanceId());

        // 不应写入 WeaponEquippedStore（盾非武器）
        assertNull(WeaponEquippedStore.get("off_hand"),
            "盾牌不应写入 WeaponEquippedStore");
    }

    @Test
    void boneShield_offHand_writesEquippedShieldStore() {
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":20,"template_id":"bone_shield",
                       "weapon_kind":"shield","durability_current":80.0,
                       "durability_max":100.0,"quality_tier":0}}
            """));

        assertNotNull(EquippedShieldStore.snapshot());
        assertEquals("bone_shield", EquippedShieldStore.snapshot().templateId());
        assertEquals(80f, EquippedShieldStore.snapshot().durabilityCurrent(), 0.001f);
    }

    @Test
    void isShieldTemplate_trueForShieldSuffix() {
        assertTrue(WeaponEquippedHandler.isShieldTemplate("wooden_shield"),
            "wooden_shield 应被识别为盾牌 template");
        assertTrue(WeaponEquippedHandler.isShieldTemplate("bone_shield"),
            "bone_shield 应被识别为盾牌 template");
        assertTrue(WeaponEquippedHandler.isShieldTemplate("custom_iron_shield"),
            "_shield 后缀应被识别为盾牌");
    }

    @Test
    void isShieldTemplate_falseForNonShield() {
        assertFalse(WeaponEquippedHandler.isShieldTemplate("iron_sword"),
            "iron_sword 不是盾牌");
        assertFalse(WeaponEquippedHandler.isShieldTemplate("bone_dagger"),
            "bone_dagger 不是盾牌");
        assertFalse(WeaponEquippedHandler.isShieldTemplate(null),
            "null 不是盾牌");
        assertFalse(WeaponEquippedHandler.isShieldTemplate(""),
            "空字符串不是盾牌");
    }

    @Test
    void offHandWeapon_clearsShieldStore() {
        // 先装备盾牌
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":10,"template_id":"wooden_shield",
                       "weapon_kind":"shield","durability_current":100.0,
                       "durability_max":100.0,"quality_tier":0}}
            """));
        assertNotNull(EquippedShieldStore.snapshot());

        // 再装备普通武器到 off_hand → 盾槽应清空
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":2,"template_id":"bone_dagger",
                       "weapon_kind":"dagger","durability_current":120.0,
                       "durability_max":120.0,"quality_tier":0}}
            """));
        assertNull(EquippedShieldStore.snapshot(),
            "off_hand 改装普通武器后盾槽应清空");
        assertNotNull(WeaponEquippedStore.get("off_hand"),
            "off_hand 普通武器应写入 WeaponEquippedStore");
    }

    @Test
    void offHandShield_afterOffHandWeapon_clearsWeaponStore() {
        // #3 手持盾无盾模型回归：off_hand 先持武器，再换盾 → 必须清空 WeaponEquippedStore 的
        // off_hand 武器，否则手持物渲染 mixin 的 bongOff!=null 分支会渲染残留武器、盾分支
        // (bongOff==null) 永不触发 → 盾不显。
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":7,"template_id":"bone_dagger",
                       "weapon_kind":"dagger","durability_current":120.0,
                       "durability_max":120.0,"quality_tier":0}}
            """));
        assertNotNull(WeaponEquippedStore.get("off_hand"),
            "前置：off_hand 应先持有武器 bone_dagger");

        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":11,"template_id":"wooden_shield",
                       "weapon_kind":"shield","durability_current":100.0,
                       "durability_max":100.0,"quality_tier":0}}
            """));

        assertNull(WeaponEquippedStore.get("off_hand"),
            "换盾后 off_hand 武器槽必须清空（否则渲染残留武器而非盾）");
        assertNotNull(EquippedShieldStore.snapshot(),
            "换盾后 EquippedShieldStore 应持有 wooden_shield");
        assertEquals("wooden_shield", EquippedShieldStore.snapshot().templateId());
    }

    @Test
    void clearOffHand_alsosClearsShieldStore() {
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand",
             "weapon":{"instance_id":10,"template_id":"wooden_shield",
                       "weapon_kind":"shield","durability_current":100.0,
                       "durability_max":100.0,"quality_tier":0}}
            """));
        assertNotNull(EquippedShieldStore.snapshot());

        // 卸下 off_hand
        new WeaponEquippedHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_equipped","slot":"off_hand"}
            """));
        assertNull(EquippedShieldStore.snapshot(),
            "卸下 off_hand 时盾槽应同步清空");
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
