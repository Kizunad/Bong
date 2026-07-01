package com.bong.client.lingtian;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F8 附带修复：{@link HoeVanillaIconMap} 的 registry-free 守卫分支。
 *
 * <p>此前 {@code MAP} 是 {@code Map<String, Item>}，值直接引用 {@code Items.IRON_HOE} 等——
 * 静态字段初始化在类加载时立即求值，意味着<b>任何一次</b> {@link HoeVanillaIconMap#isHoe}
 * 调用（哪怕传的是非锄头 id）都会连带触发 {@code Items} 类初始化，而这需要 MC Bootstrap
 * （headless 单测环境没有）。修成 {@code Map<String, Supplier<Item>>}（与
 * {@code WeaponVanillaIconMap}/{@code BongWeaponModelRegistry}/{@code BlockVanillaIconMap}
 * 同款惰性求值模式）后，{@link HoeVanillaIconMap#isHoe} 纯查 key，不触发 {@code Items} 求值。
 *
 * <p>{@code createStackFor(...)} 命中已注册锄头 id 时仍会求值 {@code Items}（happy path），
 * 由 e2e/真游戏覆盖（同 {@code ShieldVanillaIconMapTest} / {@code BlockVanillaIconMapTest}
 * 的既有约定）。这里只锁"查表本身"与"未命中前置守卫"两类 registry-free 分支。
 */
class HoeVanillaIconMapTest {

    @Test
    void isHoe_recognizesAllThreeKnownHoeTemplates() {
        assertTrue(HoeVanillaIconMap.isHoe("hoe_iron"), "hoe_iron 应识别为锄头模板");
        assertTrue(HoeVanillaIconMap.isHoe("hoe_lingtie"), "hoe_lingtie 应识别为锄头模板");
        assertTrue(HoeVanillaIconMap.isHoe("hoe_xuantie"), "hoe_xuantie 应识别为锄头模板");
    }

    @Test
    void isHoe_returnsFalseForNonHoeTemplate() {
        // iron_sword 是真实武器模板（非锄头）：isHoe 必须纯查 key 返回 false，
        // 不应误判、也不应因求值 Items 而抛异常。
        assertFalse(HoeVanillaIconMap.isHoe("iron_sword"));
        assertFalse(HoeVanillaIconMap.isHoe("wooden_shield"));
        assertFalse(HoeVanillaIconMap.isHoe("not_a_real_template"));
    }

    @Test
    void isHoe_returnsFalseForEmptyOrBlankTemplate() {
        assertFalse(HoeVanillaIconMap.isHoe(""));
        assertFalse(HoeVanillaIconMap.isHoe("   "));
    }

    @Test
    void isHoe_returnsFalseForNullTemplate() {
        assertFalse(HoeVanillaIconMap.isHoe(null), "null 模板必须在查表前返回 false，不得 NPE");
    }

    @Test
    void createStackFor_returnsNullForNullTemplate() {
        // null 必须在查 MAP 之前短路返回 null——不查表、不触发 Items 求值。
        org.junit.jupiter.api.Assertions.assertNull(HoeVanillaIconMap.createStackFor(null));
    }

    @Test
    void createStackFor_returnsNullForUnknownTemplate_withoutTouchingItems() {
        // 未注册 id：MAP.get(...) 返回 null，短路返回，不求值任何 Supplier，不触发 Items。
        org.junit.jupiter.api.Assertions.assertNull(
            HoeVanillaIconMap.createStackFor("not_a_real_template"));
    }
}
