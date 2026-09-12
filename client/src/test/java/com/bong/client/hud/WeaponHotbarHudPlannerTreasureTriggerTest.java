package com.bong.client.hud;

import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.WeaponEquippedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class WeaponHotbarHudPlannerTreasureTriggerTest {
    @BeforeEach
    @AfterEach
    void clear() {
        EquippedShieldStore.resetForTests();
        WeaponEquippedStore.resetForTests();
        TreasureEquippedStore.resetForTests();
    }

    @Test
    void offHandSelectionPreservesEquipmentPriorityAndTriggerFallback() {
        TreasureEquippedStore.putOrClear("trigger_2",
            new EquippedTreasure("trigger_2", 2, "spirit_treasure_jizhaojing", "寂照镜"));
        assertEquals(List.of("spirit_treasure_jizhaojing"), icons(), "触发位允许前面为空");
        TreasureEquippedStore.putOrClear("trigger_0",
            new EquippedTreasure("trigger_0", 1, "first_trigger", "法宝"));
        assertEquals(List.of("first_trigger"), icons(), "选择首个占用的触发位");
        TreasureEquippedStore.putOrClear("off_hand",
            new EquippedTreasure("off_hand", 3, "held_treasure", "法宝"));
        assertEquals(List.of("held_treasure"), icons(), "持械法宝优先于触发位");
        EquippedShieldStore.equip(new EquippedShield(4, "wooden_shield", 80, 100));
        assertEquals(List.of("wooden_shield"), icons(), "盾牌优先于法宝");
    }

    @Test
    void toolNeverMasksTreasureAndEquipmentTransitionsClearOldIcon() {
        WeaponEquippedStore.putOrClear("main_hand",
            new EquippedWeapon("main_hand", 1, "iron_sword", "sword", 80, 100, 0));
        assertEquals(List.of("iron_sword"), icons());
        WeaponEquippedStore.putOrClear("main_hand",
            new EquippedWeapon("main_hand", 2, "tool_mining_pickaxe", "tool", 80, 100, 0));
        assertTrue(icons().isEmpty(), "工具只参与手持模型，不占用战斗装备 HUD");
        WeaponEquippedStore.putOrClear("main_hand", null);
        assertTrue(icons().isEmpty(), "卸下后不得复活旧武器");
        WeaponEquippedStore.putOrClear("off_hand",
            new EquippedWeapon("off_hand", 3, "tool_hoe", "tool", 80, 100, 0));
        TreasureEquippedStore.putOrClear("trigger_0",
            new EquippedTreasure("trigger_0", 4, "spirit_treasure_jizhaojing", "寂照镜"));
        assertEquals(List.of("spirit_treasure_jizhaojing"), icons(), "副手工具不能遮住触发位法宝");
    }

    @Test
    void handsAndDashStaySeparateAtNarrowWidths() {
        WeaponEquippedStore.putOrClear("main_hand",
            new EquippedWeapon("main_hand", 1, "iron_sword", "sword", 80, 100, 0));
        EquippedShieldStore.equip(new EquippedShield(2, "wooden_shield", 80, 100));
        for (int width : new int[]{640, 320, 166}) {
            var hands = WeaponHotbarHudPlanner.buildCommands(width, 180);
            var dash = MovementHudPlanner.buildCommands(
                com.bong.client.movement.MovementState.empty(), true, width, 180, 1_000);
            assertEquals(List.of("iron_sword", "wooden_shield"),
                hands.stream().filter(HudRenderCommand::isItemTexture).map(HudRenderCommand::text).toList());
            for (var hand : hands) {
                assertTrue(hand.x() >= 0 && hand.y() >= 0
                    && hand.x() + hand.width() <= width && hand.y() + hand.height() <= 180,
                    "双手 HUD 必须留在窗口内");
                if (hand.isItemTexture()) assertEquals(hand.width(), hand.height(), "装备图标保持比例");
                for (var movement : dash) {
                    assertTrue(hand.x() + hand.width() <= movement.x()
                        || movement.x() + movement.width() <= hand.x()
                        || hand.y() + hand.height() <= movement.y()
                        || movement.y() + movement.height() <= hand.y(), "窄窗口下 Dash 不能占用持械位");
                }
            }
        }
        assertTrue(WeaponHotbarHudPlanner.buildCommands(80, 60).isEmpty(), "极小窗口不应输出越界槽位");
    }

    private static List<String> icons() {
        return WeaponHotbarHudPlanner.buildCommands(800, 600).stream()
            .filter(HudRenderCommand::isItemTexture).map(HudRenderCommand::text).toList();
    }
}
