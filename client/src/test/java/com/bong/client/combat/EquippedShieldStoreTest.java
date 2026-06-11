package com.bong.client.combat;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P3：EquippedShieldStore 状态机饱和单测。
 *
 * <p>覆盖：
 * <ul>
 *   <li>初始状态：snapshot == null</li>
 *   <li>equip → snapshot 返回装备的盾</li>
 *   <li>equip(null) → 清空</li>
 *   <li>clear() → 清空</li>
 *   <li>clearIfInstance(-1) 无条件清空</li>
 *   <li>clearIfInstance(匹配) → 清空</li>
 *   <li>clearIfInstance(不匹配) → 保留</li>
 *   <li>覆盖：装备新盾时旧盾被替换</li>
 *   <li>durabilityRatio 边界（0/最大/空）</li>
 * </ul>
 */
class EquippedShieldStoreTest {

    private static final EquippedShield WOODEN = new EquippedShield(1L, "wooden_shield", 80f, 100f);
    private static final EquippedShield BONE   = new EquippedShield(2L, "bone_shield",   50f, 100f);

    @AfterEach
    void tearDown() {
        EquippedShieldStore.resetForTests();
    }

    // ---- 初始状态 -------------------------------------------------------

    @Test
    void initiallyEmpty() {
        assertNull(EquippedShieldStore.snapshot(),
            "初始状态下 EquippedShieldStore 应为空（null）");
    }

    // ---- equip --------------------------------------------------------

    @Test
    void equip_storesShield() {
        EquippedShieldStore.equip(WOODEN);
        EquippedShield snap = EquippedShieldStore.snapshot();
        assertNotNull(snap, "equip 后 snapshot 不应为 null");
        assertEquals("wooden_shield", snap.templateId(),
            "equip 后 templateId 应为 wooden_shield");
        assertEquals(1L, snap.instanceId(),
            "equip 后 instanceId 应为 1");
    }

    @Test
    void equip_null_clearsStore() {
        EquippedShieldStore.equip(WOODEN);
        EquippedShieldStore.equip(null);
        assertNull(EquippedShieldStore.snapshot(),
            "equip(null) 应清空 store");
    }

    @Test
    void equip_replacesExistingShield() {
        EquippedShieldStore.equip(WOODEN);
        EquippedShieldStore.equip(BONE);
        assertEquals("bone_shield", EquippedShieldStore.snapshot().templateId(),
            "再次 equip 时应覆盖旧盾");
    }

    // ---- clear ---------------------------------------------------------

    @Test
    void clear_emptyStoreIsNoOp() {
        assertDoesNotThrow(EquippedShieldStore::clear,
            "对空 store 调用 clear 不应抛异常");
        assertNull(EquippedShieldStore.snapshot());
    }

    @Test
    void clear_removesEquippedShield() {
        EquippedShieldStore.equip(WOODEN);
        EquippedShieldStore.clear();
        assertNull(EquippedShieldStore.snapshot(),
            "clear() 后 snapshot 应为 null");
    }

    // ---- clearIfInstance -----------------------------------------------------

    @Test
    void clearIfInstance_negativeOne_unconditionalClear() {
        EquippedShieldStore.equip(WOODEN);
        EquippedShieldStore.clearIfInstance(-1L);
        assertNull(EquippedShieldStore.snapshot(),
            "clearIfInstance(-1) 应无条件清空");
    }

    @Test
    void clearIfInstance_matchingId_clearsStore() {
        EquippedShieldStore.equip(WOODEN);                     // instanceId=1
        EquippedShieldStore.clearIfInstance(1L);
        assertNull(EquippedShieldStore.snapshot(),
            "clearIfInstance 匹配 instanceId 时应清空");
    }

    @Test
    void clearIfInstance_mismatchedId_keepsStore() {
        EquippedShieldStore.equip(WOODEN);                     // instanceId=1
        EquippedShieldStore.clearIfInstance(999L);             // 不匹配
        assertNotNull(EquippedShieldStore.snapshot(),
            "clearIfInstance 不匹配时不应清空（防竞态覆写）");
        assertEquals(1L, EquippedShieldStore.snapshot().instanceId());
    }

    @Test
    void clearIfInstance_onEmpty_doesNotThrow() {
        assertDoesNotThrow(() -> EquippedShieldStore.clearIfInstance(1L),
            "对空 store 调用 clearIfInstance 不应抛异常");
    }

    // ---- durabilityRatio -----------------------------------------------

    @Test
    void equippedShield_durabilityRatio_fullHealth() {
        EquippedShield shield = new EquippedShield(1L, "wooden_shield", 100f, 100f);
        assertEquals(1.0f, shield.durabilityRatio(), 0.001f,
            "满耐久度 durabilityRatio 应为 1.0");
    }

    @Test
    void equippedShield_durabilityRatio_zero() {
        EquippedShield shield = new EquippedShield(1L, "wooden_shield", 0f, 100f);
        assertEquals(0.0f, shield.durabilityRatio(), 0.001f,
            "耐久为 0 时 durabilityRatio 应为 0.0");
    }

    @Test
    void equippedShield_durabilityRatio_clampsBelowZero() {
        EquippedShield shield = new EquippedShield(1L, "wooden_shield", -10f, 100f);
        assertEquals(0.0f, shield.durabilityRatio(), 0.001f,
            "负耐久度应被 clamp 到 0.0");
    }

    @Test
    void equippedShield_durabilityRatio_clampsAboveOne() {
        EquippedShield shield = new EquippedShield(1L, "wooden_shield", 120f, 100f);
        assertEquals(1.0f, shield.durabilityRatio(), 0.001f,
            "超过 max 的耐久度应被 clamp 到 1.0");
    }

    @Test
    void equippedShield_durabilityRatio_zeroMaxIsZero() {
        EquippedShield shield = new EquippedShield(1L, "wooden_shield", 50f, 0f);
        assertEquals(0.0f, shield.durabilityRatio(), 0.001f,
            "durabilityMax=0 时 ratio 应为 0 避免除零");
    }

    @Test
    void equippedShield_durabilityRatio_halfDurability() {
        EquippedShield shield = new EquippedShield(1L, "wooden_shield", 50f, 100f);
        assertEquals(0.5f, shield.durabilityRatio(), 0.001f,
            "50/100 耐久度应为 0.5");
    }

    // ---- 装备 → 显示耐久条状态转换（集成快照验证）-------------------------------

    @Test
    void equipAndSnapshot_durabilityCurrentAndMaxPreserved() {
        EquippedShieldStore.equip(new EquippedShield(3L, "bone_shield", 37f, 200f));
        EquippedShield snap = EquippedShieldStore.snapshot();
        assertNotNull(snap);
        assertEquals(37f, snap.durabilityCurrent(), 0.001f,
            "durabilityCurrents 应被准确存储");
        assertEquals(200f, snap.durabilityMax(), 0.001f,
            "durabilityMax 应被准确存储");
    }
}
