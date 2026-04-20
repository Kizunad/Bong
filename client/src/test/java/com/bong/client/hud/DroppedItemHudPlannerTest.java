package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

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
    void emitsProjectedMarkerForNearestVisibleDroppedItem() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            0.0,
            7.5,
            InventoryItem.createFull(
                1004L,
                "starter_talisman",
                "启程护符",
                1,
                1,
                0.2,
                "common",
                "fixture",
                1,
                0.5,
                1.0
            )
        ));

        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L,
            "main_pack",
            0,
            1,
            0.0,
            0.0,
            12.0,
            InventoryItem.createFull(
                1005L,
                "old_coin",
                "旧铜钱",
                1,
                1,
                0.1,
                "common",
                "fixture",
                1,
                0.5,
                1.0
            )
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT);
        assertEquals(3, commands.size());
        assertEquals(1, commands.stream().filter(HudRenderCommand::isRect).count());
        assertTrue(commands.get(0).isRect());
        assertTrue(commands.get(1).isItemTexture());
        assertTrue(commands.get(2).isText());
        assertEquals("starter_talisman", commands.get(1).text());
        assertTrue(!commands.get(2).text().startsWith("← "));
        assertTrue(!commands.get(2).text().startsWith("→ "));
        assertTrue(!commands.get(2).text().startsWith("↑ "));
        assertTrue(!commands.get(2).text().startsWith("↓ "));
        assertTrue(commands.get(2).text().contains("启程护符"));
        assertTrue(commands.get(2).text().contains("7.5m"));
    }

    @Test
    void hidesMarkerWhenNearestDropIsBehindCamera() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            0.0,
            -4.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        assertTrue(DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT).isEmpty());
    }

    @Test
    void nearestBehindCameraDoesNotFallBackToFartherVisibleDrop() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            0.0,
            -1.0,
            InventoryItem.simple("starter_talisman", "背后近物")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L,
            "main_pack",
            0,
            1,
            0.0,
            0.0,
            8.0,
            InventoryItem.simple("old_coin", "前方远物")
        ));

        assertTrue(DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT).isEmpty());
    }

    @Test
    void clampsMarkerIntoViewportWhenProjectedTargetFallsOffScreen() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            -20.0,
            0.0,
            5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertEquals(4, commands.size());
        assertEquals(2, commands.stream().filter(HudRenderCommand::isRect).count());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isRect() && cmd.width() == 2 && cmd.height() > 2));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("→ ")));
        assertTrue(commands.get(0).x() >= 0);
        assertTrue(commands.get(0).x() + commands.get(0).width() <= 120);
        assertTrue(commands.get(0).y() >= 0);
        assertTrue(commands.get(0).y() + commands.get(0).height() <= 80);
    }

    @Test
    void rightClampedMarkerUsesRightDirectionalPrefix() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            -20.0,
            0.0,
            5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("→ ")));
    }

    @Test
    void leftClampedMarkerUsesLeftDirectionalPrefix() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            20.0,
            0.0,
            5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("← ")));
    }

    @Test
    void topClampedMarkerUsesTopDirectionalPrefix() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            30.0,
            5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("↑ ")));
    }

    @Test
    void bottomClampedMarkerUsesBottomDirectionalPrefix() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            -30.0,
            5.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("↓ ")));
    }

    @Test
    void cornerClampedMarkerUsesVerticalPrefixWhenVerticalOverflowDominates() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            8.0,
            40.0,
            2.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("↑ ")));
    }

    @Test
    void cornerClampedMarkerKeepsHorizontalPrefixWhenHorizontalOverflowDominates() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            -30.0,
            8.0,
            2.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("→ ")));
    }

    @Test
    void rendersAtMostOneMarkerWhenManyDropsExist() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1001L,
            "main_pack",
            0,
            0,
            0.0,
            0.0,
            4.0,
            InventoryItem.simple("starter_talisman", "最近物")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1002L,
            "main_pack",
            0,
            1,
            0.0,
            0.0,
            8.0,
            InventoryItem.simple("old_coin", "中距离物")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1003L,
            "main_pack",
            0,
            2,
            0.0,
            0.0,
            14.0,
            InventoryItem.simple("cloth_wrap", "远距离物")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT);
        assertEquals(3, commands.size());
        assertTrue(commands.get(2).text().contains("最近物"));
    }

    @Test
    void stabilizesSmallProjectedMovementForSameTarget() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            0.0,
            7.5,
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

        assertEquals(first.get(0).x(), second.get(0).x());
        assertEquals(first.get(0).y(), second.get(0).y());
    }

    @Test
    void resetsStabilityWhenTargetChanges() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            0.0,
            0.0,
            7.5,
            InventoryItem.simple("starter_talisman", "近物甲")
        ));
        DroppedItemHudPlanner.MarkerStabilityState state = new DroppedItemHudPlanner.MarkerStabilityState();

        DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 320, 180, TEST_CONTEXT, state);

        DroppedItemStore.resetForTests();
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L,
            "main_pack",
            0,
            0,
            20.0,
            0.0,
            5.0,
            InventoryItem.simple("old_coin", "左侧新目标")
        ));

        List<HudRenderCommand> commands = DroppedItemHudPlanner.buildCommands(FIXED_WIDTH, 220, 120, 80, TEST_CONTEXT, state);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().startsWith("← ")));
    }
}
