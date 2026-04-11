package com.bong.client.inventory;

import com.bong.client.inventory.component.*;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MockMeridianData;
import com.bong.client.inventory.state.DragState;
import com.bong.client.inventory.state.MeridianStateStore;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.*;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;


public class InspectScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("检视");
    private static final int ICON_SIZE = 128;
    private static final int HOTBAR_SLOTS = 9;

    private static final int TAB_ACTIVE_COLOR = 0xFFCCCCCC;
    private static final int TAB_INACTIVE_COLOR = 0xFF555555;

    private final InventoryModel model;
    private final DragState dragState = new DragState();

    // --- Container grids (driven by model.containers()) ---
    private BackpackGridPanel[] containerGrids;
    private FlowLayout[] containerWrappers;
    private LabelComponent[] containerLabels;
    private int containerCount;
    private int activeContainer = 0;

    private EquipmentPanel equipPanel;
    private StatusBarsPanel statusBars;
    private ItemTooltipPanel tooltipPanel;
    private BottomInfoBar bottomBar;

    // Tabs (left panel)
    private int activeTab = 0;
    private final LabelComponent[] tabLabels = new LabelComponent[3];
    private FlowLayout equipTabContent;
    private FlowLayout cultivationTabContent;
    private FlowLayout alchemyTabContent;

    // Hotbar
    private final GridSlotComponent[] hotbarSlots = new GridSlotComponent[HOTBAR_SLOTS];
    private final InventoryItem[] hotbarItems = new InventoryItem[HOTBAR_SLOTS];
    private FlowLayout hotbarStrip;

    // Discard
    private FlowLayout discardStrip;

    // Meridian body (cultivation tab)
    private MeridianBodyComponent meridianBodyComponent;

    public InspectScreen(InventoryModel model) {
        super(TITLE);
        this.model = model == null ? InventoryModel.empty() : model;
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        // Outermost: [hotbar] [main] [discard]
        FlowLayout outerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        outerRow.gap(2);
        outerRow.verticalAlignment(VerticalAlignment.CENTER);

        // === FAR LEFT: Hotbar ===
        hotbarStrip = buildHotbarStrip();
        outerRow.child(hotbarStrip);

        // === CENTER: Main panel ===
        FlowLayout mainPanel = Containers.verticalFlow(Sizing.content(), Sizing.content());
        mainPanel.surface(Surface.flat(0xFF1A1A1A));
        mainPanel.padding(Insets.of(4));
        mainPanel.gap(2);

        FlowLayout middle = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        middle.gap(4);

        // -- Left column --
        FlowLayout leftCol = Containers.verticalFlow(Sizing.fixed(148), Sizing.content());
        leftCol.gap(2);

        // Tab bar
        FlowLayout tabBar = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        tabBar.gap(6);
        tabBar.padding(Insets.of(1, 2, 1, 2));
        String[] tabNames = {"装备", "修仙", "丹药"};
        for (int i = 0; i < 3; i++) {
            final int idx = i;
            var label = Components.label(Text.literal(tabNames[i]));
            label.color(Color.ofArgb(i == 0 ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            label.cursorStyle(CursorStyle.HAND);
            label.mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) { switchTab(idx); return true; }
                return false;
            });
            tabLabels[i] = label;
            tabBar.child(label);
        }
        leftCol.child(tabBar);

        // Tab 0: Equipment + Status
        equipTabContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        equipTabContent.gap(2);
        equipPanel = new EquipmentPanel();
        equipTabContent.child(equipPanel.container());
        statusBars = new StatusBarsPanel();
        equipTabContent.child(statusBars);
        leftCol.child(equipTabContent);

        // Tab 1: Cultivation (meridian body)
        cultivationTabContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        cultivationTabContent.gap(0);
        meridianBodyComponent = new MeridianBodyComponent();
        MeridianBody meridianData = MeridianStateStore.snapshot();
        meridianBodyComponent.setBody(meridianData != null ? meridianData : MockMeridianData.create());
        cultivationTabContent.child(meridianBodyComponent);
        leftCol.child(cultivationTabContent);
        cultivationTabContent.positioning(Positioning.absolute(-9999, -9999));

        // Tab 2: Alchemy
        alchemyTabContent = makeTabPlaceholder("丹药炼制 — 开发中");
        leftCol.child(alchemyTabContent);
        alchemyTabContent.positioning(Positioning.absolute(-9999, -9999));

        middle.child(leftCol);

        // -- Right column --
        FlowLayout rightCol = Containers.verticalFlow(Sizing.content(), Sizing.content());
        rightCol.gap(2);

        // Container tabs (driven by model)
        var containerDefs = model.containers();
        containerCount = containerDefs.size();
        containerGrids = new BackpackGridPanel[containerCount];
        containerWrappers = new FlowLayout[containerCount];
        containerLabels = new LabelComponent[containerCount];

        FlowLayout containerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        containerRow.gap(2);
        int maxCols = 0;
        for (var def : containerDefs) maxCols = Math.max(maxCols, def.cols());

        for (int i = 0; i < containerCount; i++) {
            final int ci = i;
            var def = containerDefs.get(i);

            FlowLayout tab = Containers.horizontalFlow(Sizing.content(), Sizing.fixed(14));
            tab.surface(Surface.flat(i == 0 ? 0xFF282828 : 0xFF1E1E1E));
            tab.padding(Insets.of(1, 4, 1, 4));
            tab.verticalAlignment(VerticalAlignment.CENTER);
            tab.cursorStyle(CursorStyle.HAND);

            var label = Components.label(Text.literal(
                (i == 0 ? "§f" : "§7") + def.name()
                + " §8(" + def.rows() + "×" + def.cols() + ")"
            ));
            containerLabels[i] = label;
            tab.child(label);
            tab.mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) { switchContainer(ci); return true; }
                return false;
            });
            containerRow.child(tab);
        }
        rightCol.child(containerRow);

        // Build all grids, show only active
        int wrapperW = maxCols * GridSlotComponent.CELL_SIZE + 4;
        for (int i = 0; i < containerCount; i++) {
            var def = containerDefs.get(i);
            containerGrids[i] = new BackpackGridPanel(def.rows(), def.cols());
            FlowLayout w = Containers.verticalFlow(Sizing.fixed(wrapperW), Sizing.content());
            w.surface(Surface.flat(0xFF111111));
            w.padding(Insets.of(2));
            w.child(containerGrids[i].container());
            containerWrappers[i] = w;
            rightCol.child(w);
            if (i != 0) w.positioning(Positioning.absolute(-9999, -9999));
        }

        // Tooltip
        tooltipPanel = new ItemTooltipPanel();
        rightCol.child(tooltipPanel);

        middle.child(rightCol);
        mainPanel.child(middle);

        // Bottom bar
        bottomBar = new BottomInfoBar();
        mainPanel.child(bottomBar);

        outerRow.child(mainPanel);

        // === FAR RIGHT: Discard ===
        discardStrip = buildDiscardStrip();
        outerRow.child(discardStrip);

        root.child(outerRow);
        populateFromModel();
    }

    // ==================== Build helpers ====================

    private FlowLayout makeTabPlaceholder(String text) {
        FlowLayout f = Containers.verticalFlow(Sizing.fill(100), Sizing.fixed(148));
        f.surface(Surface.flat(0xFF181818));
        f.verticalAlignment(VerticalAlignment.CENTER);
        f.horizontalAlignment(HorizontalAlignment.CENTER);
        f.child(Components.label(Text.literal("§7" + text)));
        return f;
    }

    private FlowLayout buildHotbarStrip() {
        int cs = GridSlotComponent.CELL_SIZE;
        FlowLayout strip = Containers.verticalFlow(Sizing.fixed(cs + 6), Sizing.content());
        strip.surface(Surface.flat(0xFF141414));
        strip.padding(Insets.of(3));
        strip.gap(1);
        strip.horizontalAlignment(HorizontalAlignment.CENTER);

        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            GridSlotComponent slot = new GridSlotComponent(i, 0);
            hotbarSlots[i] = slot;
            strip.child(slot);
        }
        return strip;
    }

    private FlowLayout buildDiscardStrip() {
        int cs = GridSlotComponent.CELL_SIZE;
        FlowLayout strip = Containers.verticalFlow(Sizing.fixed(cs + 6), Sizing.content());
        strip.surface(Surface.flat(0xFF201010));
        strip.padding(Insets.of(3));
        strip.gap(2);
        strip.horizontalAlignment(HorizontalAlignment.CENTER);
        strip.verticalAlignment(VerticalAlignment.CENTER);
        strip.child(Components.label(Text.literal("§c丢")));
        strip.child(Components.label(Text.literal("§c弃")));
        return strip;
    }

    // ==================== Active grid shortcut ====================

    private BackpackGridPanel activeGrid() {
        return containerGrids[activeContainer];
    }

    // ==================== Tab / Container switching ====================

    private void switchTab(int idx) {
        if (idx == activeTab) return;
        activeTab = idx;
        for (int i = 0; i < 3; i++)
            tabLabels[i].color(Color.ofArgb(i == idx ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
        FlowLayout[] tabs = {equipTabContent, cultivationTabContent, alchemyTabContent};
        for (int i = 0; i < 3; i++)
            tabs[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
    }

    private void switchContainer(int idx) {
        if (idx == activeContainer || idx < 0 || idx >= containerCount) return;
        activeContainer = idx;
        var defs = model.containers();
        for (int i = 0; i < containerCount; i++) {
            containerWrappers[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
            var def = defs.get(i);
            containerLabels[i].text(Text.literal(
                (i == idx ? "§f" : "§7") + def.name()
                + " §8(" + def.rows() + "×" + def.cols() + ")"
            ));
        }
    }

    // ==================== Populate ====================

    private void populateFromModel() {
        // Main backpack (container 0) gets model data
        containerGrids[0].populateFromModel(model);
        // Other containers start empty
        equipPanel.populateFromModel(model);
        statusBars.updateFromModel(model);
        bottomBar.updateFromModel(model);

        // Equipment state is managed solely by EquipmentPanel

        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            InventoryItem item = i < model.hotbar().size() ? model.hotbar().get(i) : null;
            hotbarItems[i] = item;
            if (hotbarSlots[i] != null) {
                if (item != null) hotbarSlots[i].setItem(item, true);
                else hotbarSlots[i].clearItem();
            }
        }
    }

    // ==================== Hit detection ====================

    private int hotbarSlotAtScreen(double sx, double sy) {
        int cs = GridSlotComponent.CELL_SIZE;
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            GridSlotComponent s = hotbarSlots[i];
            if (s != null && sx >= s.x() && sx < s.x() + cs && sy >= s.y() && sy < s.y() + cs)
                return i;
        }
        return -1;
    }

    private boolean isOverDiscard(double sx, double sy) {
        return sx >= discardStrip.x() && sx < discardStrip.x() + discardStrip.width()
            && sy >= discardStrip.y() && sy < discardStrip.y() + discardStrip.height();
    }

    // ==================== Mouse interaction ====================

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (button == 0) {
            boolean shift = hasShiftDown();
            BackpackGridPanel grid = activeGrid();

            // Grid
            if (grid.containsPoint(mouseX, mouseY)) {
                var pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = grid.itemAt(pos.row(), pos.col());
                    if (item != null) {
                        if (shift) quickEquipFromGrid(item);
                        else {
                            var anchor = grid.anchorOf(item);
                            if (anchor != null) {
                                grid.remove(item);
                                dragState.pickup(item, anchor.row(), anchor.col());
                            }
                        }
                        return true;
                    }
                }
            }

            // Equip
            if (activeTab == 0) {
                var eq = equipPanel.slotAtScreen(mouseX, mouseY);
                if (eq != null && eq.item() != null) {
                    InventoryItem item = eq.item();
                    if (shift) quickUnequipToGrid(eq.slotType(), item);
                    else {
                        eq.clearItem();
                        dragState.pickupFromEquip(item, eq.slotType());
                    }
                    return true;
                }
            }

            // Hotbar
            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            if (hIdx >= 0 && hotbarItems[hIdx] != null) {
                InventoryItem item = hotbarItems[hIdx];
                if (shift) quickMoveHotbarToGrid(hIdx);
                else {
                    hotbarItems[hIdx] = null;
                    hotbarSlots[hIdx].clearItem();
                    dragState.pickupFromHotbar(item, hIdx);
                }
                return true;
            }
        }

        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double deltaX, double deltaY) {
        if (dragState.isDragging()) {
            dragState.updateMouse(mouseX, mouseY);
            updateHighlights(mouseX, mouseY);
            return true;
        }
        return super.mouseDragged(mouseX, mouseY, button, deltaX, deltaY);
    }

    @Override
    public boolean mouseReleased(double mouseX, double mouseY, int button) {
        if (button == 0 && dragState.isDragging()) {
            attemptDrop(mouseX, mouseY);
            return true;
        }
        return super.mouseReleased(mouseX, mouseY, button);
    }

    // ==================== Drag ====================

    private void attemptDrop(double mouseX, double mouseY) {
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) { dragState.cancel(); clearAllHighlights(); return; }

        // Discard
        if (isOverDiscard(mouseX, mouseY)) {
            dragState.drop();
            clearAllHighlights();
            return;
        }

        // Active grid
        BackpackGridPanel grid = activeGrid();
        if (grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null && grid.canPlace(dragged, pos.row(), pos.col())) {
                grid.place(dragged, pos.row(), pos.col());
                dragState.drop();
                clearAllHighlights();
                return;
            }
        }

        // Equip
        if (activeTab == 0) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null) {
                if (eq.item() == null) {
                    eq.setItem(dragged);
                    dragState.drop();
                } else {
                    InventoryItem old = eq.item();
                    eq.setItem(dragged);
                    dragState.drop();
                    placeItemAnywhere(old);
                }
                clearAllHighlights();
                return;
            }
        }

        // Hotbar
        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0 && dragged.gridWidth() == 1 && dragged.gridHeight() == 1) {
            if (hotbarItems[hIdx] == null) {
                hotbarItems[hIdx] = dragged;
                hotbarSlots[hIdx].setItem(dragged, true);
                dragState.drop();
            } else {
                InventoryItem old = hotbarItems[hIdx];
                hotbarItems[hIdx] = dragged;
                hotbarSlots[hIdx].setItem(dragged, true);
                dragState.drop();
                placeItemAnywhere(old);
            }
            clearAllHighlights();
            return;
        }

        returnDragToSource();
        clearAllHighlights();
    }

    private void returnDragToSource() {
        DragState.CancelResult r = dragState.cancel();
        if (!r.hasItem()) return;
        InventoryItem item = r.item();
        if (r.sourceKind() == null) { placeItemAnywhere(item); return; }
        switch (r.sourceKind()) {
            case GRID -> {
                // Try return to the active grid (user may have switched containers)
                BackpackGridPanel grid = activeGrid();
                if (grid.canPlace(item, r.sourceRow(), r.sourceCol()))
                    grid.place(item, r.sourceRow(), r.sourceCol());
                else placeItemAnywhere(item);
            }
            case EQUIP -> {
                if (r.sourceEquipSlot() != null) {
                    var slot = equipPanel.slotFor(r.sourceEquipSlot());
                    if (slot != null) slot.setItem(item);
                }
            }
            case HOTBAR -> {
                int idx = r.sourceHotbarIndex();
                if (idx >= 0 && idx < HOTBAR_SLOTS) {
                    hotbarItems[idx] = item;
                    hotbarSlots[idx].setItem(item, true);
                }
            }
        }
    }

    private void placeItemAnywhere(InventoryItem item) {
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) activeGrid().place(item, pos.row(), pos.col());
    }

    // ==================== Highlights ====================

    private void updateHighlights(double mouseX, double mouseY) {
        clearAllHighlights();
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) return;

        BackpackGridPanel grid = activeGrid();
        if (grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                boolean valid = grid.canPlace(dragged, pos.row(), pos.col());
                grid.highlightArea(pos.row(), pos.col(), dragged.gridWidth(), dragged.gridHeight(),
                    valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
            }
        }

        if (activeTab == 0) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null) eq.setHighlightState(GridSlotComponent.HighlightState.VALID);
        }

        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0) {
            boolean valid = dragged.gridWidth() == 1 && dragged.gridHeight() == 1;
            hotbarSlots[hIdx].setHighlightState(
                valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
        }

        discardStrip.surface(Surface.flat(isOverDiscard(mouseX, mouseY) ? 0xFF331111 : 0xFF201010));
    }

    private void clearAllHighlights() {
        for (BackpackGridPanel g : containerGrids) g.clearHighlights();
        equipPanel.clearHighlights();
        for (int i = 0; i < HOTBAR_SLOTS; i++)
            if (hotbarSlots[i] != null) hotbarSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
        discardStrip.surface(Surface.flat(0xFF201010));
    }

    // ==================== Quick operations ====================

    private void quickEquipFromGrid(InventoryItem item) {
        for (EquipSlotType type : EquipSlotType.values()) {
            var slot = equipPanel.slotFor(type);
            if (slot != null && slot.item() == null) {
                activeGrid().remove(item);
                slot.setItem(item);
                return;
            }
        }
    }

    private void quickUnequipToGrid(EquipSlotType slotType, InventoryItem item) {
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            equipPanel.slotFor(slotType).clearItem();
            activeGrid().place(item, pos.row(), pos.col());
        }
    }

    private void quickMoveHotbarToGrid(int index) {
        InventoryItem item = hotbarItems[index];
        if (item == null) return;
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            hotbarItems[index] = null;
            hotbarSlots[index].clearItem();
            activeGrid().place(item, pos.row(), pos.col());
        }
    }

    // ==================== Render ====================

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        super.render(context, mouseX, mouseY, delta);
        drawMultiCellItems(context);
        updateTooltipFromHover(mouseX, mouseY);

        // Meridian tooltip — drawn here to escape owo-lib component clipping
        if (activeTab == 1 && meridianBodyComponent != null) {
            var matrices = context.getMatrices();
            matrices.push();
            matrices.translate(0, 0, 400);
            meridianBodyComponent.drawMeridianTooltip(context, mouseX, mouseY);
            matrices.pop();
        }

        if (dragState.isDragging() && dragState.draggedItem() != null) {
            InventoryItem item = dragState.draggedItem();
            int cs = GridSlotComponent.CELL_SIZE;
            int gw = item.gridWidth() * cs, gh = item.gridHeight() * cs;

            Identifier tex = new Identifier("bong-client", "textures/gui/items/" + item.itemId() + ".png");
            var matrices = context.getMatrices();
            matrices.push();
            matrices.translate(0, 0, 200);

            int fitSize = Math.min(gw, gh);
            int fitX = mouseX - fitSize / 2, fitY = mouseY - fitSize / 2;

            RenderSystem.enableBlend();
            RenderSystem.defaultBlendFunc();
            RenderSystem.setShaderColor(1f, 1f, 1f, 0.75f);
            matrices.push();
            matrices.translate(fitX, fitY, 0);
            matrices.scale((float) fitSize / ICON_SIZE, (float) fitSize / ICON_SIZE, 1f);
            context.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
            matrices.pop();

            RenderSystem.setShaderColor(1f, 1f, 1f, 1f);
            RenderSystem.disableBlend();
            matrices.pop();
        }
    }

    private void drawMultiCellItems(DrawContext context) {
        BackpackGridPanel grid = activeGrid();
        for (var entry : grid.toGridEntries()) {
            InventoryItem item = entry.item();
            if (item.gridWidth() == 1 && item.gridHeight() == 1) continue;
            if (dragState.isDragging() && dragState.draggedItem() == item) continue;

            GridSlotComponent anchor = grid.slotAt(entry.row(), entry.col());
            if (anchor == null) continue;

            int px = anchor.x() + 2, py = anchor.y() + 2;
            int pw = item.gridWidth() * GridSlotComponent.CELL_SIZE - 4;
            int ph = item.gridHeight() * GridSlotComponent.CELL_SIZE - 4;
            drawItemTextureRaw(context, item, px, py, pw, ph);
        }
    }

    private static void drawItemTextureRaw(DrawContext ctx, InventoryItem item, int dx, int dy, int dw, int dh) {
        if (item == null || item.isEmpty()) return;
        Identifier tex = new Identifier("bong-client", "textures/gui/items/" + item.itemId() + ".png");
        int fitSize = Math.min(dw, dh);
        int ox = (dw - fitSize) / 2, oy = (dh - fitSize) / 2;

        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.enableDepthTest();
        var m = ctx.getMatrices();
        m.push();
        m.translate(dx + ox, dy + oy, 100);
        m.scale((float) fitSize / ICON_SIZE, (float) fitSize / ICON_SIZE, 1f);
        ctx.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
        m.pop();
        RenderSystem.disableBlend();
    }

    private void updateTooltipFromHover(double mx, double my) {
        if (dragState.isDragging()) { tooltipPanel.setHoveredItem(dragState.draggedItem()); return; }
        InventoryItem hovered = null;
        BackpackGridPanel grid = activeGrid();
        if (grid.containsPoint(mx, my)) {
            var pos = grid.screenToGrid(mx, my);
            if (pos != null) hovered = grid.itemAt(pos.row(), pos.col());
        }
        if (hovered == null && activeTab == 0) {
            var eq = equipPanel.slotAtScreen(mx, my);
            if (eq != null) hovered = eq.item();
        }
        if (hovered == null) {
            int idx = hotbarSlotAtScreen(mx, my);
            if (idx >= 0) hovered = hotbarItems[idx];
        }
        tooltipPanel.setHoveredItem(hovered);
    }
}
