package com.bong.client.inventory;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SkillIconIds;
import com.bong.client.inventory.component.EquipmentPanel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.component.StatusBarsPanel;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.window.UiWindowDefinition;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.text.Text;
import java.util.Set;

/** 工作台与固定 HUD 共用装备和槽位视图；库存、绑定仍由各领域 Store 拥有。 */
public final class InventoryLoadoutWindows {
    public static final UiWindowDefinition EQUIPMENT = new UiWindowDefinition(
        "inventory-equipment", "inventory-equipment", 180, 140, Set.of(UiWindowDefinition.Capability.WINDOW));
    public static final UiWindowDefinition SHORTCUTS = new UiWindowDefinition(
        "inventory-shortcuts", "inventory-shortcuts", 180, 120, Set.of(UiWindowDefinition.Capability.WINDOW));
    private final EquipmentPanel equipment = new EquipmentPanel();
    private final StatusBarsPanel status = new StatusBarsPanel();
    final GridSlotComponent[] hotbarSlots = slots(SkillBarConfig.SLOT_COUNT);
    final GridSlotComponent[] quickUseSlots = slots(QuickSlotConfig.SLOT_COUNT);
    final InventoryItem[] hotbarItems = new InventoryItem[SkillBarConfig.SLOT_COUNT];
    final InventoryItem[] quickUseItems = new InventoryItem[QuickSlotConfig.SLOT_COUNT];
    private InventoryModel model = InventoryModel.empty();
    private SkillBarConfig skillConfig;
    private QuickSlotConfig quickConfig;

    private static GridSlotComponent[] slots(int count) {
        var slots = new GridSlotComponent[count];
        for (int i = 0; i < count; i++) slots[i] = new GridSlotComponent(i, 0);
        return slots;
    }

    public EquipmentPanel equipment() { return equipment; }
    public GridSlotComponent hotbarSlot(int index) { return hotbarSlots[index]; }
    public GridSlotComponent quickUseSlot(int index) { return quickUseSlots[index]; }

    public void refresh() {
        var next = InventoryStateStore.snapshot();
        if (next != model) populate(next);
        if (SkillBarStore.snapshot() != skillConfig) hydrateSkills();
        if (QuickUseSlotStore.snapshot() != quickConfig) hydrateQuickUse(QuickUseSlotStore.snapshot());
    }

    public void populate(InventoryModel next) {
        model = next;
        equipment.populateFromModel(next);
        status.updateFromModel(next);
        for (int i = 0; i < hotbarItems.length; i++) {
            hotbarItems[i] = i < next.hotbar().size() ? next.hotbar().get(i) : null;
        }
        hydrateSkills();
        hydrateQuickUse(QuickUseSlotStore.snapshot());
    }

    void hydrateQuickUse(QuickSlotConfig config) {
        quickConfig = config;
        for (int i = 0; i < quickUseItems.length; i++) {
            var entry = config.slot(i);
            quickUseItems[i] = entry == null ? null : findInstance(entry.instanceId());
            show(quickUseSlots[i], quickUseItems[i]);
        }
    }

    void hydrateSkills() {
        var config = SkillBarStore.snapshot();
        skillConfig = config;
        for (int i = 0; i < hotbarSlots.length; i++) {
            var entry = config.slot(i);
            InventoryItem shown = hotbarItems[i];
            if (entry != null && entry.kind() == SkillBarEntry.Kind.ITEM) {
                InventoryItem matched = findItem(entry.id());
                // 实例耗尽后清除悬空绑定和选中态；首个库存快照尚未到达时不误清。
                if (matched == null && !model.isEmpty()) SkillBarStore.updateSlot(i, null);
                else shown = matched == null ? InventoryItem.simple(entry.id(), entry.displayName()) : matched;
            } else if (entry != null) {
                shown = InventoryItem.simple("skill_scroll_" + SkillIconIds.safeSkillIconId(entry.id()), entry.displayName());
            }
            show(hotbarSlots[i], shown);
        }
        skillConfig = SkillBarStore.snapshot();
    }

    private static void show(GridSlotComponent slot, InventoryItem item) {
        if (item == null) slot.clearItem();
        else slot.setItem(item, true);
    }

    private InventoryItem findInstance(long id) {
        for (var entry : model.gridItems()) if (entry.item().instanceId() == id) return entry.item();
        for (var item : model.hotbar()) if (item != null && item.instanceId() == id) return item;
        for (var slot : model.equippedSlots().values()) {
            if (slot.held() != null && slot.held().instanceId() == id) return slot.held();
            for (var item : slot.worn()) if (item.instanceId() == id) return item;
        }
        return null;
    }

    private InventoryItem findItem(String id) {
        if (id == null || id.isEmpty()) return null;
        for (var item : model.hotbar()) if (item != null && id.equals(item.itemId())) return item;
        for (var entry : model.gridItems()) if (id.equals(entry.item().itemId())) return entry.item();
        return null;
    }

    public void attach(UiWindowDefinition definition, FlowLayout root) {
        if (definition.equals(EQUIPMENT)) {
            equipment.container().surface(Surface.BLANK);
            root.childById(FlowLayout.class, "equipment-slots").child(equipment.container());
            root.childById(FlowLayout.class, "equipment-status").child(status);
        } else {
            attachSlots(root.childById(FlowLayout.class, "combat-slots"), hotbarSlots, "");
            attachSlots(root.childById(FlowLayout.class, "quick-use-slots"), quickUseSlots, "F");
        }
    }

    public void detach(UiWindowDefinition definition, FlowLayout root) {
        if (definition.equals(EQUIPMENT)) {
            root.childById(FlowLayout.class, "equipment-slots").removeChild(equipment.container());
            root.childById(FlowLayout.class, "equipment-status").removeChild(status);
        } else {
            for (var slot : hotbarSlots) if (slot.parent() instanceof FlowLayout parent) parent.removeChild(slot);
            for (var slot : quickUseSlots) if (slot.parent() instanceof FlowLayout parent) parent.removeChild(slot);
        }
    }

    private static void attachSlots(FlowLayout row, GridSlotComponent[] slots, String prefix) {
        for (int i = 0; i < slots.length; i++) {
            var column = Containers.verticalFlow(Sizing.fixed(GridSlotComponent.CELL_SIZE), Sizing.content());
            column.horizontalAlignment(HorizontalAlignment.CENTER);
            column.gap(4);
            column.child(Components.label(Text.literal(prefix + (i + 1))).color(Color.ofRgb(0xA6AFA8)));
            column.child(slots[i]);
            row.child(column);
        }
    }

    public int hotbarAt(double x, double y) { return slotAt(hotbarSlots, x, y); }
    public int quickUseAt(double x, double y) { return slotAt(quickUseSlots, x, y); }

    private static int slotAt(GridSlotComponent[] slots, double x, double y) {
        for (int i = 0; i < slots.length; i++) if (slots[i].isInBoundingBox(x, y)) return i;
        return -1;
    }
}
