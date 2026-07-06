package com.bong.client.inventory.state;

import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.MeridianChannel;

import java.util.Objects;

public final class DragState {
    public enum Phase { IDLE, DRAGGING }

    public enum SourceKind { GRID, EQUIP, HOTBAR, QUICK_USE, MERIDIAN, BODY_PART }

    private Phase phase = Phase.IDLE;
    private InventoryItem draggedItem;
    /** plan-rotate-v1 — 拾起时的原朝向物品（取消拖拽 / 非网格落位回退用）。 */
    private InventoryItem originalDraggedItem;
    /** plan-rotate-v1 — 累计旋转奇偶：连按两次 R = 恢复原朝向 = false。 */
    private boolean draggedRotated;
    private SourceKind sourceKind;
    private int sourceRow = -1;
    private int sourceCol = -1;
    private String sourceContainerId;
    private EquipSlotType sourceEquipSlot;
    private int sourceHotbarIndex = -1;
    private int sourceQuickUseIndex = -1;
    private MeridianChannel sourceMeridianChannel;
    private BodyPart sourceBodyPart;
    private double mouseX;
    private double mouseY;

    public void pickup(InventoryItem item, int gridRow, int gridCol) {
        pickup(item, null, gridRow, gridCol);
    }

    public void pickup(InventoryItem item, String containerId, int gridRow, int gridCol) {
        Objects.requireNonNull(item, "item");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.originalDraggedItem = item;
        this.draggedRotated = false;
        this.sourceKind = SourceKind.GRID;
        this.sourceRow = gridRow;
        this.sourceCol = gridCol;
        this.sourceContainerId = containerId;
        this.sourceEquipSlot = null;
        this.sourceHotbarIndex = -1;
    }

    public void pickupFromEquip(InventoryItem item, EquipSlotType slot) {
        Objects.requireNonNull(item, "item");
        Objects.requireNonNull(slot, "slot");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.originalDraggedItem = item;
        this.draggedRotated = false;
        this.sourceKind = SourceKind.EQUIP;
        this.sourceEquipSlot = slot;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceHotbarIndex = -1;
    }

    public void pickupFromHotbar(InventoryItem item, int index) {
        Objects.requireNonNull(item, "item");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.originalDraggedItem = item;
        this.draggedRotated = false;
        this.sourceKind = SourceKind.HOTBAR;
        this.sourceHotbarIndex = index;
        this.sourceQuickUseIndex = -1;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceEquipSlot = null;
        this.sourceMeridianChannel = null;
    }

    public void pickupFromQuickUse(InventoryItem item, int index) {
        Objects.requireNonNull(item, "item");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.originalDraggedItem = item;
        this.draggedRotated = false;
        this.sourceKind = SourceKind.QUICK_USE;
        this.sourceQuickUseIndex = index;
        this.sourceHotbarIndex = -1;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceEquipSlot = null;
        this.sourceMeridianChannel = null;
        this.sourceBodyPart = null;
    }

    public void pickupFromMeridian(InventoryItem item, MeridianChannel channel) {
        Objects.requireNonNull(item, "item");
        Objects.requireNonNull(channel, "channel");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.originalDraggedItem = item;
        this.draggedRotated = false;
        this.sourceKind = SourceKind.MERIDIAN;
        this.sourceMeridianChannel = channel;
        this.sourceBodyPart = null;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceEquipSlot = null;
        this.sourceHotbarIndex = -1;
    }

    public void pickupFromBodyPart(InventoryItem item, BodyPart part) {
        Objects.requireNonNull(item, "item");
        Objects.requireNonNull(part, "part");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.originalDraggedItem = item;
        this.draggedRotated = false;
        this.sourceKind = SourceKind.BODY_PART;
        this.sourceBodyPart = part;
        this.sourceMeridianChannel = null;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceEquipSlot = null;
        this.sourceHotbarIndex = -1;
    }

    public InventoryItem drop() {
        InventoryItem item = draggedItem;
        reset();
        return item;
    }

    public CancelResult cancel() {
        // plan-rotate-v1 — 取消拖拽必须还原「原朝向」的件：旋转只在成功落位时生效，
        // 放回来源位置若带着旋转后的 footprint 可能与邻居重叠。
        InventoryItem restore = originalDraggedItem != null ? originalDraggedItem : draggedItem;
        CancelResult result = new CancelResult(
            restore, sourceKind, sourceContainerId, sourceRow, sourceCol, sourceEquipSlot, sourceHotbarIndex, sourceQuickUseIndex, sourceMeridianChannel, sourceBodyPart
        );
        reset();
        return result;
    }

    public void updateMouse(double x, double y) {
        this.mouseX = x;
        this.mouseY = y;
    }

    /**
     * plan-rotate-v1 — 拖拽中按 R 旋转当前拖拽物（宽高互换）。
     * 仅 DRAGGING 阶段生效；正方形（含 1x1）footprint 旋转无意义，no-op 且不翻转奇偶标志。
     *
     * @return 是否真的发生了旋转
     */
    public boolean rotateDraggedItem() {
        if (phase != Phase.DRAGGING || draggedItem == null) {
            return false;
        }
        if (draggedItem.gridWidth() == draggedItem.gridHeight()) {
            return false;
        }
        draggedItem = draggedItem.withRotatedFootprint();
        draggedRotated = !draggedRotated;
        return true;
    }

    /** plan-rotate-v1 — 当前拖拽物相对拾起时是否处于旋转朝向（发 move intent 时透传）。 */
    public boolean draggedRotated() {
        return draggedRotated;
    }

    /** plan-rotate-v1 — 拾起时的原朝向物品；非拖拽阶段为 null。 */
    public InventoryItem originalDraggedItem() {
        return originalDraggedItem;
    }

    public boolean isDragging() {
        return phase == Phase.DRAGGING;
    }

    public Phase phase() { return phase; }
    public InventoryItem draggedItem() { return draggedItem; }
    public SourceKind sourceKind() { return sourceKind; }
    public int sourceRow() { return sourceRow; }
    public int sourceCol() { return sourceCol; }
    public String sourceContainerId() { return sourceContainerId; }
    public EquipSlotType sourceEquipSlot() { return sourceEquipSlot; }
    public int sourceHotbarIndex() { return sourceHotbarIndex; }
    public int sourceQuickUseIndex() { return sourceQuickUseIndex; }
    public MeridianChannel sourceMeridianChannel() { return sourceMeridianChannel; }
    public BodyPart sourceBodyPart() { return sourceBodyPart; }
    public double mouseX() { return mouseX; }
    public double mouseY() { return mouseY; }

    private void reset() {
        this.phase = Phase.IDLE;
        this.draggedItem = null;
        this.originalDraggedItem = null;
        this.draggedRotated = false;
        this.sourceKind = null;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceContainerId = null;
        this.sourceEquipSlot = null;
        this.sourceHotbarIndex = -1;
        this.sourceQuickUseIndex = -1;
        this.sourceMeridianChannel = null;
        this.sourceBodyPart = null;
    }

    public record CancelResult(
        InventoryItem item,
        SourceKind sourceKind,
        String sourceContainerId,
        int sourceRow,
        int sourceCol,
        EquipSlotType sourceEquipSlot,
        int sourceHotbarIndex,
        int sourceQuickUseIndex,
        MeridianChannel sourceMeridianChannel,
        BodyPart sourceBodyPart
    ) {
        public boolean hasItem() { return item != null; }
    }
}
