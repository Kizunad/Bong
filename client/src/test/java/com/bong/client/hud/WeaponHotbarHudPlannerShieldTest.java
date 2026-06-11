package com.bong.client.hud;

import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P3：WeaponHotbarHudPlanner off_hand 盾牌槽渲染单测。
 *
 * <p>覆盖：
 * <ul>
 *   <li>盾牌装备时 off_hand 槽渲染命令不为空</li>
 *   <li>盾牌清空时 off_hand 槽无渲染</li>
 *   <li>off_hand 有武器时盾槽不渲染（武器优先）</li>
 *   <li>盾槽耐久条颜色随耐久变化</li>
 * </ul>
 */
class WeaponHotbarHudPlannerShieldTest {

    private static final int SCREEN_W = 800;
    private static final int SCREEN_H = 600;

    @BeforeEach
    void setUp() {
        EquippedShieldStore.resetForTests();
        WeaponEquippedStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        EquippedShieldStore.resetForTests();
        WeaponEquippedStore.resetForTests();
    }

    @Test
    void noShieldEquipped_rendersNoOffHandCommands() {
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        // 没有武器/盾/宝器，不应有 off_hand 相关 rect
        // WeaponHotbarHudPlanner 只在 offHand != null 时渲染 —— 断言命令数量为 0（main_hand 也没装）
        assertTrue(cmds.isEmpty(),
            "空状态下不应生成任何 HUD 命令，实际 cmds.size()=" + cmds.size());
    }

    @Test
    void shieldEquipped_rendersOffHandSlotCommands() {
        EquippedShieldStore.equip(new EquippedShield(1L, "wooden_shield", 100f, 100f));
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        assertFalse(cmds.isEmpty(),
            "装备盾牌后 WeaponHotbarHudPlanner 应生成 off_hand 渲染命令");
    }

    @Test
    void shieldEquipped_rendersShieldGlyph_盾() {
        EquippedShieldStore.equip(new EquippedShield(1L, "wooden_shield", 100f, 100f));
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        boolean hasShieldGlyph = cmds.stream()
            .anyMatch(cmd -> cmd.isText() && "盾".equals(cmd.text()));
        assertTrue(hasShieldGlyph,
            "盾牌槽应渲染「盾」字标识，但未找到 text='盾' 的 HudRenderCommand");
    }

    @Test
    void shieldEquipped_rendersDurabilityBar() {
        EquippedShieldStore.equip(new EquippedShield(1L, "wooden_shield", 60f, 100f));
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        // 耐久条 = 至少 2 个 rect（bg + filled），加上背景 + border，总 rect 应 > 2
        long rectCount = cmds.stream().filter(HudRenderCommand::isRect).count();
        assertTrue(rectCount >= 2,
            "盾牌槽应渲染背景 + 耐久条（至少 2 个 rect），实际 rectCount=" + rectCount);
    }

    @Test
    void shieldCleared_rendersNoOffHandSlot() {
        EquippedShieldStore.equip(new EquippedShield(1L, "wooden_shield", 100f, 100f));
        EquippedShieldStore.clear();
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        boolean hasShieldGlyph = cmds.stream()
            .anyMatch(cmd -> cmd.isText() && "盾".equals(cmd.text()));
        assertFalse(hasShieldGlyph,
            "盾牌清空后不应渲染「盾」字");
    }

    @Test
    void offHandWeapon_takePrecedenceOverShield() {
        // 同时有 off_hand 武器和盾牌时，武器优先（WeaponEquippedStore.get("off_hand") != null 分支先走）
        EquippedShieldStore.equip(new EquippedShield(1L, "wooden_shield", 100f, 100f));
        WeaponEquippedStore.putOrClear("off_hand",
            new EquippedWeapon("off_hand", 2L, "bone_dagger", "dagger", 120f, 120f, 0));

        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        // 武器字形应出现
        boolean hasDaggerGlyph = cmds.stream()
            .anyMatch(cmd -> cmd.isText() && "匕".equals(cmd.text()));
        assertTrue(hasDaggerGlyph,
            "off_hand 有武器时应渲染武器字形（匕），不应被盾牌覆盖");
        // 盾字不应出现
        boolean hasShieldGlyph = cmds.stream()
            .anyMatch(cmd -> cmd.isText() && "盾".equals(cmd.text()));
        assertFalse(hasShieldGlyph,
            "off_hand 有武器时不应渲染盾牌字形「盾」");
    }

    @Test
    void durabilityColor_fullHealth_isGreen() {
        int color = WeaponHotbarHudPlanner.durabilityColor(1.0f);
        assertEquals(WeaponHotbarHudPlanner.DURABILITY_FG_FULL_COLOR, color,
            "满耐久应为绿色");
    }

    @Test
    void durabilityColor_midRange_isYellow() {
        int color = WeaponHotbarHudPlanner.durabilityColor(0.35f);
        assertEquals(WeaponHotbarHudPlanner.DURABILITY_FG_MID_COLOR, color,
            "35% 耐久应为黄色");
    }

    @Test
    void durabilityColor_low_isRed() {
        int color = WeaponHotbarHudPlanner.durabilityColor(0.10f);
        assertEquals(WeaponHotbarHudPlanner.DURABILITY_FG_LOW_COLOR, color,
            "10% 耐久应为红色");
    }
}
