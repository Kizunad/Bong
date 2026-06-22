package com.bong.client.weapon;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * #3 手持盾无盾模型：{@link ShieldVanillaIconMap} 的 registry-free 守卫分支。
 *
 * <p>happy path（{@code createStackFor("wooden_shield")} → 真 ItemStack）会走
 * {@code hostItem()} → {@code Items.NAUTILUS_SHELL}，触原版注册表，需 MC Bootstrap——
 * client 测试环境没有（见 {@code BlockVanillaIconMapTest} 注释），由 e2e/真游戏覆盖。
 *
 * <p>这里完整锁住**查注册表之前的早退守卫**：非盾模板 / null / 未知 id 一律返回 null，
 * 绝不把武器/工具/任意 vanilla item 误经盾渲染路径合成 stack。这是「盾路径只渲染盾」的契约。
 */
class ShieldVanillaIconMapTest {

    @Test
    void returnsNullForNullTemplate() {
        assertNull(ShieldVanillaIconMap.createStackFor(null), "null 模板必须在查注册表前返回 null，不得 NPE");
    }

    @Test
    void returnsNullForEmptyTemplate() {
        assertNull(ShieldVanillaIconMap.createStackFor(""), "空串非盾模板，必须返回 null");
        assertNull(ShieldVanillaIconMap.createStackFor("   "), "纯空白非盾模板，必须返回 null");
    }

    @Test
    void returnsNullForNonShieldTemplate() {
        // 武器/工具模板即使在 BongWeaponModelRegistry 里有 entry，也不得经盾路径合成 stack——
        // 否则 off_hand 盾分支会把武器渲染成盾。SHIELD_TEMPLATE_IDS 守卫在查 hostItem 前短路。
        assertNull(ShieldVanillaIconMap.createStackFor("iron_sword"), "剑是武器非盾，盾路径必须返回 null");
        assertNull(ShieldVanillaIconMap.createStackFor("axe_bone"), "斧是工具非盾，盾路径必须返回 null");
        assertNull(ShieldVanillaIconMap.createStackFor("hand_wrap"), "拳套非盾，盾路径必须返回 null");
    }

    @Test
    void returnsNullForUnknownTemplate() {
        assertNull(ShieldVanillaIconMap.createStackFor("not_a_real_template"), "未知模板必须返回 null");
        assertNull(ShieldVanillaIconMap.createStackFor("shield"), "裸 shield 非已注册盾模板，必须返回 null");
    }
}
