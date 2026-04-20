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
 * "图标带壳" 策略下的 planner 回归测试。layout 计算（投影 + clamp + stabilize）保留，
 * emit：background rect + (optional edge accent) + icon 的 itemTexture。**不** emit 文字标签。
 * 原版本里方向前缀（↑↓←→）相关 6 个测试已删（下线的是 text emit，前缀计算函数留作 dead code 规格）。
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
    void emitsBackdropAndIconForNearestVisibleDroppedItem() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            0.0, 0.0, 7.5,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT);
        // 视野居中、无 clamp 时只出 background + icon（无 edge accent 条）
        assertEquals(2, commands.size());
        assertTrue(commands.get(0).isRect(), "first command = background rect");
        assertTrue(commands.get(1).isItemTexture(), "second command = item icon");
        assertEquals("starter_talisman", commands.get(1).text());
        assertFalse(commands.stream().anyMatch(HudRenderCommand::isText), "no floating text label");
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
    void clampsBackdropAndIconIntoViewportWhenProjectedTargetFallsOffScreen() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L, "main_pack", 0, 0,
            -20.0, 0.0, 5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        // 被 clamp 时会多出 edge accent 条（rect），所以 >= 2
        assertTrue(commands.size() >= 2);
        HudRenderCommand backdrop = commands.get(0);
        assertTrue(backdrop.isRect(), "first command = background rect");
        // 背景方块完全落在屏幕内
        assertTrue(backdrop.x() >= 0, "backdrop x >= 0");
        assertTrue(backdrop.x() + backdrop.width() <= 120, "backdrop fully on-screen horizontally");
        assertTrue(backdrop.y() >= 0, "backdrop y >= 0");
        assertTrue(backdrop.y() + backdrop.height() <= 80, "backdrop fully on-screen vertically");
        // icon 也在屏幕内
        HudRenderCommand icon = commands.stream()
            .filter(HudRenderCommand::isItemTexture).findFirst().orElseThrow();
        assertTrue(icon.x() >= 0 && icon.x() + icon.width() <= 120);
        assertTrue(icon.y() >= 0 && icon.y() + icon.height() <= 80);
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
        List<HudRenderCommand> icons = commands.stream()
            .filter(HudRenderCommand::isItemTexture).toList();
        assertEquals(1, icons.size(), "无论 emit 多少辅助 rect/accent，itemTexture 只能一个");
        assertEquals("starter_talisman", icons.get(0).text(), "应选最近的 entry");
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

        // 用 backdrop rect 做稳定性断言（icon 在 backdrop 内，同步移动）
        HudRenderCommand firstBackdrop = first.get(0);
        HudRenderCommand secondBackdrop = second.get(0);
        assertEquals(firstBackdrop.x(), secondBackdrop.x());
        assertEquals(firstBackdrop.y(), secondBackdrop.y());
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
        int firstBackdropX = initial.get(0).x();

        DroppedItemStore.resetForTests();
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L, "main_pack", 0, 0,
            20.0, 0.0, 5.0,
            InventoryItem.simple("old_coin", "左侧新目标")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT, state);
        // 目标切换应重置 stabilizer，backdrop 位置从新目标投影点算起，而非从旧位置 lerp
        assertNotEquals(firstBackdropX, commands.get(0).x(), "backdrop jumped to new target, not lerped from old");
    }
}
