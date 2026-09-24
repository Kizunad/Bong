package com.bong.client.hud;

import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.WeaponEquippedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class WeaponHotbarHudPlannerShieldTest {
    @BeforeEach
    @AfterEach
    void clear() {
        EquippedShieldStore.resetForTests();
        WeaponEquippedStore.resetForTests();
        TreasureEquippedStore.resetForTests();
    }

    @Test
    void shieldUsesItsItemAndYieldsToOffHandWeapon() {
        EquippedShieldStore.equip(new EquippedShield(1, "wooden_shield", 60, 100));
        assertEquals(java.util.List.of("wooden_shield"), icons());
        WeaponEquippedStore.putOrClear("off_hand",
            new EquippedWeapon("off_hand", 2, "bone_dagger", "dagger", 120, 120, 0));
        assertEquals(java.util.List.of("bone_dagger"), icons(), "副手武器优先，盾牌不能覆盖或叠加");
        WeaponEquippedStore.putOrClear("off_hand", null);
        assertEquals(java.util.List.of("wooden_shield"), icons());
        EquippedShieldStore.clear();
        assertTrue(WeaponHotbarHudPlanner.buildCommands(800, 600).isEmpty(), "卸下后不能残留旧装备");
    }

    @Test
    void durabilityUpdatesKeepTheItemAndChangeWearFeedback() {
        EquippedShieldStore.equip(new EquippedShield(1, "wooden_shield", 80, 100));
        var healthy = WeaponHotbarHudPlanner.buildCommands(800, 600);
        EquippedShieldStore.equip(new EquippedShield(1, "wooden_shield", 8, 100));
        var worn = WeaponHotbarHudPlanner.buildCommands(800, 600);
        assertEquals(java.util.List.of("wooden_shield"), icons());
        assertNotEquals(
            healthy.stream().filter(c -> c.isSvgRect() && "hand-tick".equals(c.svgAssetKey()))
                .map(HudRenderCommand::color).toList(),
            worn.stream().filter(c -> c.isSvgRect() && "hand-tick".equals(c.svgAssetKey()))
                .map(HudRenderCommand::color).toList(),
            "耐久变化必须改变刻痕的实际绘制颜色");
        assertFalse(healthy.stream().anyMatch(c -> c.isSvgRect() && "hand-fracture".equals(c.svgAssetKey())),
            "正常耐久不显示损毁警告");
        assertTrue(worn.stream().anyMatch(c -> c.isSvgRect() && "hand-fracture".equals(c.svgAssetKey())),
            "临近损毁时显示裂口标记");
    }

    private static java.util.List<String> icons() {
        return WeaponHotbarHudPlanner.buildCommands(800, 600).stream()
            .filter(HudRenderCommand::isItemTexture).map(HudRenderCommand::text).toList();
    }
}
