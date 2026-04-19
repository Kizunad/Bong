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
        CancelResult result = new CancelResult(
            draggedItem, sourceKind, sourceRow, sourceCol, sourceEquipSlot, sourceHotbarIndex, sourceQuickUseIndex, sourceMeridianChannel, sourceBodyPart
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
