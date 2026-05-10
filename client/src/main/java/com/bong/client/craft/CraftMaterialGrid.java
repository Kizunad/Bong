package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.text.Text;

import java.util.List;

/** 中栏材料放置区：固定 3×2 材料格 + 需求清单 + 进度条。 */
public final class CraftMaterialGrid {
    private final FlowLayout root;
    private final FlowLayout slotGrid;
    private final FlowLayout requirementList;
    private final LabelComponent title;
    private final CraftProgressBar progressBar;

    public CraftMaterialGrid() {
        root = Containers.verticalFlow(Sizing.fill(100), Sizing.fill(100));
        root.padding(Insets.of(4));
        root.gap(4);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        title = Components.label(Text.literal("未选择配方"));
        title.color(Color.ofArgb(0xFFE8DDC4));
        root.child(title);

        slotGrid = Containers.verticalFlow(Sizing.content(), Sizing.content());
        slotGrid.gap(3);
        root.child(slotGrid);

        requirementList = Containers.verticalFlow(Sizing.fill(100), Sizing.fixed(74));
        requirementList.gap(1);
        root.child(requirementList);

        progressBar = new CraftProgressBar();
        root.child(progressBar.root());
    }

    public FlowLayout root() {
        return root;
    }

    public void refresh(CraftRecipe recipe, InventoryModel inventory, CraftSessionStateView state, int quantity) {
        slotGrid.clearChildren();
        requirementList.clearChildren();
        int batchQuantity = Math.max(1, quantity);
        if (recipe == null) {
            title.text(Text.literal("未选择配方"));
            fillEmptySlots();
            requirementList.child(label("从左栏选择一个配方", 0xFF888888));
            progressBar.refresh(null, state);
            return;
        }

        title.text(Text.literal(CraftRecipeFilter.displayName(recipe)));
        List<CraftMaterialState> states = CraftInventoryCounter.materialStates(recipe, inventory, batchQuantity);
        int shown = 0;
        for (int row = 0; row < CraftScreenLayout.MATERIAL_ROWS; row++) {
            FlowLayout slotRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
            slotRow.gap(3);
            for (int col = 0; col < CraftScreenLayout.MATERIAL_COLUMNS; col++) {
                CraftMaterialSlotComponent slot = new CraftMaterialSlotComponent();
                if (shown < states.size()) {
                    CraftMaterialState material = states.get(shown);
                    slot.setContent(material.templateId(), material.need(), material.sufficient());
                    slot.tooltip(Text.literal(tooltipFor(material)));
                }
                slotRow.child(slot);
                shown++;
            }
            slotGrid.child(slotRow);
        }

        requirementList.child(label("材料需求", 0xFFAFA8A0));
        for (CraftMaterialState material : states) {
            int color = material.sufficient() ? 0xFF7AB45A : 0xFFD05A4A;
            String mark = material.sufficient() ? "✓" : "✗";
            requirementList.child(label(String.format(
                "%s %s x%d (有%d)",
                mark,
                material.templateId(),
                material.need(),
                material.have()
            ), color));
        }
        if (recipe.qiCost() > 0.0) {
            double totalQiCost = recipe.qiCost() * batchQuantity;
            boolean ok = inventory != null && inventory.qiCurrent() >= totalQiCost;
            requirementList.child(label(String.format(
                "%s 自身真元 %.0f (当前 %.0f)",
                ok ? "✓" : "✗",
                totalQiCost,
                inventory == null ? 0.0 : inventory.qiCurrent()
            ), ok ? 0xFF7AB45A : 0xFFD05A4A));
        }

        progressBar.refresh(recipe, state);
    }

    private void fillEmptySlots() {
        for (int row = 0; row < CraftScreenLayout.MATERIAL_ROWS; row++) {
            FlowLayout slotRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
            slotRow.gap(3);
            for (int col = 0; col < CraftScreenLayout.MATERIAL_COLUMNS; col++) {
                slotRow.child(new CraftMaterialSlotComponent());
            }
            slotGrid.child(slotRow);
        }
    }

    private static String tooltipFor(CraftMaterialState material) {
        if (material.sufficient()) {
            return String.format("需要 %s x%d，当前拥有 x%d", material.templateId(), material.need(), material.have());
        }
        return String.format(
            "需要 %s x%d，当前拥有 x%d，还差 x%d",
            material.templateId(),
            material.need(),
            material.have(),
            material.missing()
        );
    }

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        label.maxWidth(270);
        return label;
    }
}
