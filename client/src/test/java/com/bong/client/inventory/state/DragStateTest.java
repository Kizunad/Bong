package com.bong.client.inventory.state;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-rotate-v1 — DragState 旋转（R 键）状态机测试。
 * 锁住：旋转奇偶、非拖拽 no-op、正方形 no-op、drop/cancel 复位、cancel 还原原朝向。
 */
public class DragStateTest {

    private static InventoryItem rod2x1() {
        return InventoryItem.createFull(
            42L, "long_rod", "长杆", 2, 1, 1.0, "common", "旋转测试物", 1, 1.0, 1.0);
    }

    private static InventoryItem pebble1x1() {
        return InventoryItem.createFull(
            7L, "pebble", "石子", 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
    }

    // ─── 旋转奇偶 ───────────────────────────────────────────────────────────

    @Test
    void rotateOnceSwapsDimsAndSetsRotatedFlag() {
        DragState state = new DragState();
        state.pickup(rod2x1(), "main_pack", 0, 0);

        assertTrue(state.rotateDraggedItem(), "2x1 拖拽中按 R 应发生旋转");
        assertTrue(state.draggedRotated(), "旋转一次后奇偶标志应为 true");
        assertEquals(1, state.draggedItem().gridWidth(), "旋转后宽应为 1");
        assertEquals(2, state.draggedItem().gridHeight(), "旋转后高应为 2");
    }

    @Test
    void rotateTwiceRestoresOrientationAndClearsFlag() {
        DragState state = new DragState();
        state.pickup(rod2x1(), "main_pack", 0, 0);

        assertTrue(state.rotateDraggedItem());
        assertTrue(state.rotateDraggedItem(), "第二次 R 也应生效");
        assertFalse(state.draggedRotated(), "连按两次 R = 恢复原朝向，奇偶标志应回 false");
        assertEquals(2, state.draggedItem().gridWidth());
        assertEquals(1, state.draggedItem().gridHeight());
    }

    // ─── no-op 分支 ────────────────────────────────────────────────────────

    @Test
    void rotateIsNoopWhenNotDragging() {
        DragState state = new DragState();
        assertFalse(state.rotateDraggedItem(), "IDLE 阶段按 R 应 no-op");
        assertFalse(state.draggedRotated());
        assertNull(state.draggedItem());
    }

    @Test
    void rotateIsNoopAfterDrop() {
        DragState state = new DragState();
        state.pickup(rod2x1(), "main_pack", 0, 0);
        state.drop();
        assertFalse(state.rotateDraggedItem(), "drop 后按 R 应 no-op");
        assertFalse(state.draggedRotated());
    }

    @Test
    void rotateIsNoopForSquareItemAndDoesNotFlipParity() {
        DragState state = new DragState();
        state.pickup(pebble1x1(), "main_pack", 0, 0);

        assertFalse(state.rotateDraggedItem(), "1x1 旋转无意义应 no-op");
        assertFalse(state.draggedRotated(), "no-op 不得翻转奇偶标志（否则会发 rotated:true 的假旋转）");
        assertEquals(1, state.draggedItem().gridWidth());
        assertEquals(1, state.draggedItem().gridHeight());
    }

    // ─── 复位路径 ──────────────────────────────────────────────────────────

    @Test
    void dropResetsRotatedFlag() {
        DragState state = new DragState();
        state.pickup(rod2x1(), "main_pack", 0, 0);
        state.rotateDraggedItem();

        InventoryItem dropped = state.drop();
        assertEquals(1, dropped.gridWidth(), "drop 返回当前（旋转后）朝向的件供落位");
        assertFalse(state.draggedRotated(), "drop 后旋转标志必须复位");
        assertNull(state.originalDraggedItem(), "drop 后原件引用必须清空");
        assertFalse(state.isDragging());
    }

    @Test
    void newPickupAfterRotatedDropStartsUnrotated() {
        DragState state = new DragState();
        state.pickup(rod2x1(), "main_pack", 0, 0);
        state.rotateDraggedItem();
        state.drop();

        state.pickup(rod2x1(), "main_pack", 1, 1);
        assertFalse(state.draggedRotated(), "新一次拾起必须从未旋转状态开始");
    }

    @Test
    void cancelReturnsOriginalOrientationItem() {
        DragState state = new DragState();
        InventoryItem original = rod2x1();
        state.pickup(original, "main_pack", 2, 3);
        state.rotateDraggedItem();

        DragState.CancelResult result = state.cancel();
        assertTrue(result.hasItem());
        assertSame(original, result.item(),
            "取消拖拽必须还原原朝向的件（旋转件放回原位可能与邻居重叠）");
        assertEquals(2, result.item().gridWidth());
        assertEquals(1, result.item().gridHeight());
        assertEquals("main_pack", result.sourceContainerId());
        assertEquals(2, result.sourceRow());
        assertEquals(3, result.sourceCol());
        assertFalse(state.draggedRotated(), "cancel 后旋转标志必须复位");
        assertNull(state.originalDraggedItem());
    }

    // ─── 各拾起来源均记录原件（cancel 还原用） ───────────────────────────────

    @Test
    void pickupFromEquipTracksOriginalItem() {
        DragState state = new DragState();
        InventoryItem original = rod2x1();
        state.pickupFromEquip(original, EquipSlotType.MAIN_HAND);
        state.rotateDraggedItem();

        assertSame(original, state.originalDraggedItem());
        DragState.CancelResult result = state.cancel();
        assertSame(original, result.item());
        assertEquals(EquipSlotType.MAIN_HAND, result.sourceEquipSlot());
    }

    @Test
    void pickupFromHotbarTracksOriginalItem() {
        DragState state = new DragState();
        InventoryItem original = pebble1x1();
        state.pickupFromHotbar(original, 4);

        DragState.CancelResult result = state.cancel();
        assertSame(original, result.item());
        assertEquals(4, result.sourceHotbarIndex());
    }
}
