package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.function.LongConsumer;

/** 配方驱动的材料行，不用固定格数表达原料种类。 */
public final class CraftMaterialGrid {
    private final FlowLayout root;
    private final FlowLayout rows;
    private final LabelComponent cost;
    private final CraftProgressBar progress = new CraftProgressBar();
    private final List<MaterialRow> targets = new ArrayList<>();
    private final LongConsumer onReturn;
    private int width = CraftScreenLayout.MID_W;
    private InventoryModel inventory = InventoryModel.empty();

    public CraftMaterialGrid(LongConsumer onReturn) {
        this.onReturn = onReturn;
        root = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        root.padding(Insets.of(2));
        root.gap(4);
        root.child(label("材料", 0xFFB9C8BE));
        rows = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        rows.gap(4);
        root.child(rows);
        cost = label("", 0xFFB6C8C1);
        root.child(cost);
        root.child(progress.root());
    }

    public FlowLayout root() { return root; }

    public void layout(int width) {
        this.width = Math.max(1, width);
        root.horizontalSizing(Sizing.fixed(this.width));
        for (var row : targets) updateText(row);
    }

    public String materialAt(double x, double y) {
        for (var row : targets) {
            if (row.root.isInBoundingBox(x, y)) return row.templateId;
        }
        return null;
    }

    public void refreshProgress(CraftRecipe recipe, CraftSessionStateView state) {
        progress.refresh(recipe, state);
    }

    public void refresh(CraftRecipe recipe, InventoryModel inventory, CraftSessionStateView state, int quantity) {
        this.inventory = inventory;
        var states = CraftInventoryCounter.materialStates(recipe, inventory, quantity);
        var templates = states.stream().map(CraftMaterialState::templateId).toList();
        if (!templates.equals(targets.stream().map(MaterialRow::templateId).toList())) {
            rows.clearChildren();
            targets.clear();
        }
        if (recipe == null) {
            cost.text(Text.empty());
            progress.refresh(null, state);
            return;
        }
        for (int index = 0; index < states.size(); index++) {
            var material = states.get(index);
            if (index < targets.size()) {
                var target = targets.get(index);
                target.icon.setContent(material.templateId(), material.have(), material.sufficient());
                target.text.color(Color.ofArgb(material.sufficient() ? 0xFFB7D7B1 : 0xFFD3BCAF));
                target.count.text(Text.literal(material.have() + " / " + material.need()));
                target.root.tooltip(Text.literal(materialName(inventory, material.templateId()) + " · 已放入 "
                    + material.have() + " / 需要 " + material.need() + " · 右键取回一叠"));
                updateText(target);
                continue;
            }
            var row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(CraftScreenLayout.MATERIAL_SLOT_SIZE));
            row.gap(6);
            row.verticalAlignment(VerticalAlignment.CENTER);
            var icon = new CraftMaterialSlotComponent();
            icon.setContent(material.templateId(), material.have(), material.sufficient());
            row.child(icon);
            var text = label("", material.sufficient() ? 0xFFB7D7B1 : 0xFFD3BCAF);
            text.horizontalSizing(Sizing.fixed(Math.max(1, width - 56)));
            var count = label(material.have() + " / " + material.need(), 0xFFB9C8BE);
            var description = Containers.verticalFlow(Sizing.content(), Sizing.content());
            description.gap(4);
            description.child(text).child(count);
            row.child(description);
            row.tooltip(Text.literal(materialName(inventory, material.templateId()) + " · 已放入 "
                + material.have() + " / 需要 " + material.need() + " · 右键取回一叠"));
            row.mouseDown().subscribe((x, y, button) -> {
                if (button != 1) return false;
                this.inventory.craftMaterials().stream().filter(item -> item.itemId().equals(material.templateId()))
                    .findFirst().ifPresent(item -> onReturn.accept(item.instanceId()));
                return true;
            });
            var target = new MaterialRow(material.templateId(), row, icon, text, count);
            targets.add(target);
            updateText(target);
            rows.child(row);
        }
        cost.text(Text.literal(recipe.qiCost() > 0
            ? String.format("真元 %.0f / %.0f", inventory.qiCurrent(), recipe.qiCost() * quantity) : ""));
        progress.refresh(recipe, state);
    }

    private static String materialName(InventoryModel inventory, String id) {
        return java.util.stream.Stream.of(inventory.craftMaterials().stream(),
            inventory.gridItems().stream().map(InventoryModel.GridEntry::item), inventory.hotbar().stream())
            .flatMap(stream -> stream).filter(java.util.Objects::nonNull)
            .filter(item -> id.equals(item.itemId())).map(InventoryItem::displayName).findFirst().orElse(id);
    }

    private void updateText(MaterialRow row) {
        int textWidth = Math.max(1, width - 56);
        String name = materialName(inventory, row.templateId);
        row.text.horizontalSizing(Sizing.fixed(textWidth));
        row.text.text(Text.literal(MinecraftClient.getInstance().textRenderer.trimToWidth(name, textWidth)));
    }

    private static LabelComponent label(String text, int color) {
        var label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        label.horizontalSizing(Sizing.fill(100));
        return label;
    }

    private record MaterialRow(String templateId, FlowLayout root, CraftMaterialSlotComponent icon,
                               LabelComponent text, LabelComponent count) {}
}
