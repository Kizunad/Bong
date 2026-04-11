package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Positioning;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;

import java.util.EnumMap;
import java.util.Map;

public class EquipmentPanel {
    private static final int PANEL_WIDTH = 140;
    private static final int PANEL_HEIGHT = 112;
    private static final int S = EquipSlotComponent.SLOT_SIZE;
    private static final int CX = PANEL_WIDTH / 2;

    private final FlowLayout container;
    private final EnumMap<EquipSlotType, EquipSlotComponent> slotComponents = new EnumMap<>(EquipSlotType.class);

    public EquipmentPanel() {
        container = Containers.verticalFlow(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(PANEL_HEIGHT));
        container.surface(Surface.flat(0xFF181818));

        // Row 1: Head
        addSlot(EquipSlotType.HEAD, CX - S / 2, 2);
        // Row 2: Off-hand ... Main-hand
        addSlot(EquipSlotType.OFF_HAND, 6, 28);
        addSlot(EquipSlotType.MAIN_HAND, PANEL_WIDTH - S - 6, 28);
        // Row 3: Chest, Two-hand, Feet
        addSlot(EquipSlotType.CHEST, 6, 54);
        addSlot(EquipSlotType.TWO_HAND, CX - S / 2, 54);
        addSlot(EquipSlotType.FEET, PANEL_WIDTH - S - 6, 54);
        // Row 4: Legs
        addSlot(EquipSlotType.LEGS, CX - S / 2, 80);
    }

    private void addSlot(EquipSlotType type, int px, int py) {
        EquipSlotComponent slot = new EquipSlotComponent(type);
        slot.positioning(Positioning.absolute(px, py));
        slotComponents.put(type, slot);
        container.child(slot);
    }

    public FlowLayout container() { return container; }

    public EquipSlotComponent slotFor(EquipSlotType type) { return slotComponents.get(type); }

    public Map<EquipSlotType, EquipSlotComponent> allSlots() { return slotComponents; }

    public void populateFromModel(InventoryModel model) {
        for (var entry : slotComponents.entrySet()) {
            InventoryItem equipped = model.equipped().get(entry.getKey());
            entry.getValue().setItem(equipped);
        }
    }

    public EquipSlotComponent slotAtScreen(double screenX, double screenY) {
        for (EquipSlotComponent slot : slotComponents.values()) {
            if (screenX >= slot.x() && screenX < slot.x() + EquipSlotComponent.SLOT_SIZE
                && screenY >= slot.y() && screenY < slot.y() + EquipSlotComponent.SLOT_SIZE) {
                return slot;
            }
        }
        return null;
    }

    public void clearHighlights() {
        for (EquipSlotComponent slot : slotComponents.values()) {
            slot.setHighlightState(GridSlotComponent.HighlightState.NONE);
        }
    }
}
