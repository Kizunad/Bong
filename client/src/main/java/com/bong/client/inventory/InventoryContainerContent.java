package com.bong.client.inventory;

import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.text.Text;
import java.util.Locale;

/** XML 负责窗口内布局，格子保持固定比例，通过双向滚动容纳大容器。 */
public final class InventoryContainerContent {
    private final FlowLayout root;
    private BackpackGridPanel grid;
    private InventoryModel model;
    private final Runnable returnPreparation;

    public InventoryContainerContent(FlowLayout root, Runnable returnPreparation) {
        this.root = root;
        this.returnPreparation = returnPreparation;
    }

    public void bind(InventoryContainerWindows windows, String id) {
        var next = windows.grid(id);
        if (windows.snapshot() == model && next == grid) return;
        model = windows.snapshot();
        var preparation = root.childById(FlowLayout.class, "container-preparation");
        preparation.clearChildren();
        if (model.preparationStation() != null && !model.preparedMaterials().isEmpty()) {
            var button = Components.button(Text.literal("取回未开炉材料"), ignored -> returnPreparation.run());
            button.sizing(Sizing.fill(100), Sizing.fixed(22));
            button.textShadow(false);
            button.renderer(ButtonComponent.Renderer.flat(0xFF463C32, 0xFF705B43, 0xFF252320));
            preparation.child(button);
        }
        if (next != grid) {
            var slot = root.childById(FlowLayout.class, "container-grid");
            slot.clearChildren();
            grid = next;
            if (grid != null) slot.child(new GridContents(grid));
        }
        var def = windows.definition(id);
        if (def == null) return;
        double weight = windows.snapshot().gridItems().stream().filter(entry -> id.equals(entry.containerId()))
            .mapToDouble(entry -> entry.item().weight() * Math.max(1, entry.item().stackCount())).sum();
        root.childById(LabelComponent.class, "container-summary").text(Text.literal(
            String.format(Locale.ROOT, "%d × %d   内容重量 %.1f", def.cols(), def.rows(), weight)));
    }

    public boolean gridAt(double x, double y) {
        if (grid == null || !grid.containsPoint(x, y)) return false;
        var horizontal = root.childById(ScrollContainer.class, "container-horizontal");
        var vertical = root.childById(ScrollContainer.class, "container-vertical");
        // 被滚动裁掉的格子及滚动条均不能成为物品操作目标。
        return x >= horizontal.x() && x < Math.min(horizontal.x() + horizontal.width(),
                vertical.x() + vertical.width() - vertical.scrollbarThiccness())
            && y >= Math.max(vertical.y(), horizontal.y())
            && y < Math.min(vertical.y() + vertical.height(),
                horizontal.y() + horizontal.height() - horizontal.scrollbarThiccness());
    }

    private static final class GridContents extends FlowLayout {
        private final BackpackGridPanel grid;

        private GridContents(BackpackGridPanel grid) {
            super(Sizing.content(), Sizing.content(), Algorithm.VERTICAL);
            this.grid = grid;
            child(grid.container());
        }

        @Override public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
            super.draw(context, mouseX, mouseY, partialTicks, delta);
            for (var entry : grid.toGridEntries()) {
                var item = entry.item();
                if (item.gridWidth() == 1 && item.gridHeight() == 1) continue;
                var anchor = grid.slotAt(entry.row(), entry.col());
                int width = item.gridWidth() * GridSlotComponent.CELL_SIZE;
                int height = item.gridHeight() * GridSlotComponent.CELL_SIZE;
                GridSlotComponent.drawItemTexture(context, item, anchor.x() + 2, anchor.y() + 2, width - 4, height - 4);
                GridSlotComponent.drawItemOverlays(context, item, anchor.x(), anchor.y(), width, height);
            }
        }
    }
}
