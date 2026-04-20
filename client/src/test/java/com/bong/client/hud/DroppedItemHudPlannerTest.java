package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

/**
 * "只画 icon" 策略下的 planner 回归测试。layout 计算（投影 + clamp + stabilize）保留，
 * emit 阶段只一个 itemTexture command——不再有 background rect / edge accent / directional label。
 * 原版本里方向前缀（↑↓←→）相关 6 个测试已删（下线的是 text 不是 layout，前缀函数留作 dead code 规格）。
 */
public class DroppedItemHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;
    private static final DroppedItemHudPlanner.ProjectionContext TEST_CONTEXT = new DroppedItemHudPlanner.ProjectionContext(
        new Vec3d(0.0, 0.0, 0.0),
        new Vec3d(0.0, 1.6, 0.0),
        new Vec3d(0.0, 0.0, 1.0),
        new Vec3d(-1.0, 0.0, 0.0),
        new Vec3d(0.0, 1.0, 0.0),
        90.0
    );

    @AfterEach
    void tearDown() {
        DroppedItemStore.resetForTests();
        DroppedItemHudPlanner.resetForTests();
    }

    @Test
    void noCommandsWhenNoDroppedItemsExist() {
        assertTrue(DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT).isEmpty());
    }

    @Test
    void emitsIconOnlyForNearestVisibleDroppedItem() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            0.0, 0.0, 7.5,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT);
        assertEquals(1, commands.size());
        HudRenderCommand cmd = commands.get(0);
        assertTrue(cmd.isItemTexture(), "sole command should be an item texture (icon)");
        assertEquals("starter_talisman", cmd.text(), "itemTexture command carries itemId in text field");
        assertFalse(commands.stream().anyMatch(HudRenderCommand::isRect), "no background rect");
        assertFalse(commands.stream().anyMatch(HudRenderCommand::isText), "no floating label");
    }

    @Test
    void hidesMarkerWhenNearestDropIsBehindCamera() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            0.0, 0.0, -4.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        assertTrue(DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT).isEmpty());
    }

    @Test
    void nearestBehindCameraDoesNotFallBackToFartherVisibleDrop() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            0.0, 0.0, -1.0,
            InventoryItem.simple("starter_talisman", "背后近物")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L, "main_pack", 0, 1,
            0.0, 0.0, 8.0,
            InventoryItem.simple("old_coin", "前方远物")
        ));

        assertTrue(DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT).isEmpty());
    }

    @Test
    void clampsIconIntoViewportWhenProjectedTargetFallsOffScreen() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            -20.0, 0.0, 5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertEquals(1, commands.size());
        HudRenderCommand icon = commands.get(0);
        assertTrue(icon.isItemTexture());
        // icon 应被 clamp 完全落在屏幕内
        assertTrue(icon.x() >= 0, "icon x >= 0");
        assertTrue(icon.x() + icon.width() <= 120, "icon fully on-screen horizontally");
        assertTrue(icon.y() >= 0, "icon y >= 0");
        assertTrue(icon.y() + icon.height() <= 80, "icon fully on-screen vertically");
    }

    @Test
    void rendersAtMostOneIconWhenManyDropsExist() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1001L, "main_pack", 0, 0, 0.0, 0.0, 4.0,
            InventoryItem.simple("starter_talisman", "最近物")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1002L, "main_pack", 0, 1, 0.0, 0.0, 8.0,
            InventoryItem.simple("old_coin", "中距离物")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1003L, "main_pack", 0, 2, 0.0, 0.0, 14.0,
            InventoryItem.simple("cloth_wrap", "远距离物")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT);
        assertEquals(1, commands.size());
        assertEquals("starter_talisman", commands.get(0).text(), "应选最近的 entry（距离平方最小）");
    }

    @Test
    void stabilizesSmallProjectedMovementForSameTarget() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            0.0, 0.0, 7.5,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));
        DroppedItemHudPlanner.MarkerStabilityState state = new DroppedItemHudPlanner.MarkerStabilityState();
        DroppedItemHudPlanner.ProjectionContext slightlyShifted = new DroppedItemHudPlanner.ProjectionContext(
            new Vec3d(0.03, 0.0, 0.0),
            new Vec3d(0.03, 1.6, 0.0),
            new Vec3d(0.0, 0.0, 1.0),
            new Vec3d(-1.0, 0.0, 0.0),
            new Vec3d(0.0, 1.0, 0.0),
            90.0
        );

        List<HudRenderCommand> first = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT, state);
        List<HudRenderCommand> second = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, slightlyShifted, state);

        assertEquals(1, first.size());
        assertEquals(1, second.size());
        // icon 在死区内的微小相机位移时位置保持（stabilization dead-zone 生效）
        assertEquals(first.get(0).x(), second.get(0).x());
        assertEquals(first.get(0).y(), second.get(0).y());
    }

    @Test
    void resetsStabilityWhenTargetChanges() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            0.0, 0.0, 7.5,
            InventoryItem.simple("starter_talisman", "近物甲")
        ));
        DroppedItemHudPlanner.MarkerStabilityState state = new DroppedItemHudPlanner.MarkerStabilityState();

        List<HudRenderCommand> initial = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT, state);
        assertEquals(1, initial.size());
        int firstIconX = initial.get(0).x();

        DroppedItemStore.resetForTests();
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L, "main_pack", 0, 0,
            20.0, 0.0, 5.0,
            InventoryItem.simple("old_coin", "左侧新目标")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT, state);
        assertEquals(1, commands.size());
        // 目标切换应重置 stabilizer，icon 位置从新目标投影点算起，而非从旧 icon lerp
        assertNotEquals(firstIconX, commands.get(0).x(), "icon jumped to new target, not lerped from old");
    }
}
