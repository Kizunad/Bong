package com.bong.client.inventory.state;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;

import java.util.Objects;

public final class DragState {
    public enum Phase { IDLE, DRAGGING }

    public enum SourceKind { GRID, EQUIP, HOTBAR }

    private Phase phase = Phase.IDLE;
    private InventoryItem draggedItem;
    private SourceKind sourceKind;
    private int sourceRow = -1;
    private int sourceCol = -1;
    private EquipSlotType sourceEquipSlot;
    private int sourceHotbarIndex = -1;
    private double mouseX;
    private double mouseY;

    public void pickup(InventoryItem item, int gridRow, int gridCol) {
        Objects.requireNonNull(item, "item");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
        this.sourceKind = SourceKind.GRID;
        this.sourceRow = gridRow;
        this.sourceCol = gridCol;
        this.sourceEquipSlot = null;
        this.sourceHotbarIndex = -1;
    }

    public void pickupFromEquip(InventoryItem item, EquipSlotType slot) {
        Objects.requireNonNull(item, "item");
        Objects.requireNonNull(slot, "slot");
        this.phase = Phase.DRAGGING;
        this.draggedItem = item;
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
        this.sourceKind = SourceKind.HOTBAR;
        this.sourceHotbarIndex = index;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceEquipSlot = null;
    }

    public InventoryItem drop() {
        InventoryItem item = draggedItem;
        reset();
        return item;
    }

    public CancelResult cancel() {
        CancelResult result = new CancelResult(
            draggedItem, sourceKind, sourceRow, sourceCol, sourceEquipSlot, sourceHotbarIndex
        );
        reset();
        return result;
    }

    public void updateMouse(double x, double y) {
        this.mouseX = x;
        this.mouseY = y;
    }

    public boolean isDragging() {
        return phase == Phase.DRAGGING;
    }

    public Phase phase() { return phase; }
    public InventoryItem draggedItem() { return draggedItem; }
    public SourceKind sourceKind() { return sourceKind; }
    public int sourceRow() { return sourceRow; }
    public int sourceCol() { return sourceCol; }
    public EquipSlotType sourceEquipSlot() { return sourceEquipSlot; }
    public int sourceHotbarIndex() { return sourceHotbarIndex; }
    public double mouseX() { return mouseX; }
    public double mouseY() { return mouseY; }

    private void reset() {
        this.phase = Phase.IDLE;
        this.draggedItem = null;
        this.sourceKind = null;
        this.sourceRow = -1;
        this.sourceCol = -1;
        this.sourceEquipSlot = null;
        this.sourceHotbarIndex = -1;
    }

    public record CancelResult(
        InventoryItem item,
        SourceKind sourceKind,
        int sourceRow,
        int sourceCol,
        EquipSlotType sourceEquipSlot,
        int sourceHotbarIndex
    ) {
        public boolean hasItem() { return item != null; }
    }
}
