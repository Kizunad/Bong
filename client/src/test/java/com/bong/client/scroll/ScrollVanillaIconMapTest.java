package com.bong.client.scroll;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-scroll-reading-v1 P2 — {@link ScrollVanillaIconMap} 的 registry-free 守卫分支。
 *
 * <p>照抄 {@code HoeVanillaIconMapTest} 的既有约定：{@link ScrollVanillaIconMap#isReadableScroll}
 * 必须纯查 key、不触发 {@code net.minecraft.item.Items} 求值（headless 单测环境无 MC
 * Bootstrap）。{@code createStackFor(...)} 命中已注册残卷 id 时会求值 {@code Items}
 * （happy path），由 e2e/真游戏覆盖，这里只锁"查表本身"与"未命中前置守卫"两类分支。
 */
class ScrollVanillaIconMapTest {

    @Test
    void isReadableScroll_recognizesKnownScrollTemplate() {
        assertTrue(ScrollVanillaIconMap.isReadableScroll("scroll_meridian_primer"),
            "scroll_meridian_primer 应识别为可阅读残卷模板");
    }

    @Test
    void isReadableScroll_returnsFalseForNonScrollTemplate() {
        // iron_sword / wooden_shield / hoe_iron 均为真实已注册模板（非残卷）：
        // isReadableScroll 必须纯查 key 返回 false，不应误判、也不应因求值 Items 而抛异常。
        assertFalse(ScrollVanillaIconMap.isReadableScroll("iron_sword"));
        assertFalse(ScrollVanillaIconMap.isReadableScroll("wooden_shield"));
        assertFalse(ScrollVanillaIconMap.isReadableScroll("hoe_iron"));
        assertFalse(ScrollVanillaIconMap.isReadableScroll("not_a_real_template"));
    }

    @Test
    void isReadableScroll_returnsFalseForEmptyOrBlankTemplate() {
        assertFalse(ScrollVanillaIconMap.isReadableScroll(""));
        assertFalse(ScrollVanillaIconMap.isReadableScroll("   "));
    }

    @Test
    void isReadableScroll_returnsFalseForNullTemplate() {
        assertFalse(ScrollVanillaIconMap.isReadableScroll(null),
            "null 模板必须在查表前返回 false，不得 NPE");
    }

    @Test
    void createStackFor_returnsNullForNullTemplate() {
        // null 必须在查 MAP 之前短路返回 null——不查表、不触发 Items 求值。
        assertNull(ScrollVanillaIconMap.createStackFor(null));
    }

    @Test
    void createStackFor_returnsNullForUnknownTemplate_withoutTouchingItems() {
        // 未注册 id：MAP.get(...) 返回 null，短路返回，不求值任何 Supplier，不触发 Items。
        assertNull(ScrollVanillaIconMap.createStackFor("not_a_real_template"));
    }
}
