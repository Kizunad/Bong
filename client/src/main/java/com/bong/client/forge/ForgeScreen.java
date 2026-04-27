package com.bong.client.forge;

import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.forge.input.TemperingInputHandler;
import com.bong.client.forge.screen.InscriptionPanelComponent;
import com.bong.client.forge.screen.TemperingTrackComponent;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.DragState;
import com.bong.client.inventory.state.InventoryStateStore;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.List;

/**
 * plan-forge-v1 §3.3 MVP 锻炉 UI。
 *
 * 当前为基础占位：显示砧/会话/图谱/最近结果 store 状态。
 * 后续 UI（三列布局 + 节奏轨道 + 铭文槽 + 真元注入条）在后续切片中补全。
 */
public final class ForgeScreen extends Screen {
    private static final int FORGE_GRID_ROWS = 5;
    private static final int FORGE_GRID_COLS = 7;
    private static final int FORGE_GRID_PAD = 18;

    private final TemperingTrackComponent temperingTrack = new TemperingTrackComponent();
    private final InscriptionPanelComponent inscriptionPanel = new InscriptionPanelComponent();
    private final DragState dragState = new DragState();
    private int forgeGridX = -1;
    private int forgeGridY = -1;

    public ForgeScreen() {
        super(Text.literal("锻炉"));
    }

    @Override
    protected void init() {
        super.init();
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        super.render(context, mouseX, mouseY, delta);

        int y = 20;
        int left = 12;

        ForgeStationStore.Snapshot station = ForgeStationStore.snapshot();
        context.drawText(textRenderer, Text.literal("§l砧: §r" + station.ownerName()
            + " tier=" + station.tier() + " 完整度=" + String.format("%.0f%%", station.integrity() * 100)
            + (station.hasSession() ? " §a[在炉]" : " §7[空闲]")),
            left, y, 0xFFFFFF, true);
        y += 14;

        ForgeSessionStore.Snapshot session = ForgeSessionStore.snapshot();
        if (session.sessionId() > 0) {
            context.drawText(textRenderer, Text.literal("§l会话: §r" + session.blueprintName()
                + " 步骤=" + session.currentStep() + " tier=" + session.achievedTier()),
                left, y, 0xFFFFFF, true);
            y += 14;

            if ("tempering".equals(session.currentStep())) {
                TemperingTrackComponent.drawTrack(OwoUIDrawContext.of(context),
                    temperingTrack.currentRenderState(), left, y + 4);
                y += TemperingTrackComponent.TRACK_HEIGHT + 10;
            } else if ("inscription".equals(session.currentStep())) {
                OwoUIDrawContext owoContext = OwoUIDrawContext.of(context);
                int panelY = y + 4;
                inscriptionPanel.placeAt(left, panelY);
                InscriptionPanelComponent.drawPanel(owoContext, inscriptionPanel.currentRenderState(),
                    left, panelY, dragState.isDragging() && inscriptionPanel.isInBoundingBox(mouseX, mouseY));
                drawForgeBackpack(context, owoContext,
                    left + InscriptionPanelComponent.PANEL_WIDTH + 12, panelY);
                y += Math.max(InscriptionPanelComponent.PANEL_HEIGHT,
                    FORGE_GRID_ROWS * GridSlotComponent.CELL_SIZE + FORGE_GRID_PAD) + 10;
            }
        }

        BlueprintScrollStore.Entry current = BlueprintScrollStore.current();
        if (current != null) {
            context.drawText(textRenderer, Text.literal("§l图谱: §r" + current.displayName()
                + " (tier_cap=" + current.tierCap() + " " + current.stepCount() + "步)"),
                left, y, 0xFFFFAA, true);
        } else {
            context.drawText(textRenderer, Text.literal("§l图谱: §7未学任何图谱"),
                left, y, 0xAAAAAA, true);
        }
        y += 14;

        ForgeOutcomeStore.Snapshot outcome = ForgeOutcomeStore.lastOutcome();
        if (outcome.sessionId() > 0) {
            String colorInfo = outcome.colorName() != null ? " 色=" + outcome.colorName() : " 无色";
            context.drawText(textRenderer, Text.literal("§l上次结果: §r" + outcome.bucket()
                + " " + (outcome.weaponItem() != null ? outcome.weaponItem() : "废料")
                + " 品质=" + String.format("%.0f%%", outcome.quality() * 100) + colorInfo
                + " tier=" + outcome.achievedTier()),
                left, y, outcome.flawedPath() ? 0xFFAA00 : 0x00FFAA, true);
            y += 14;
        }

        context.drawText(textRenderer, Text.literal("§7按 U 关闭 | 图谱翻页: ←/→"),
            left, y + 8, 0x888888, true);

        drawDraggedItem(context, OwoUIDrawContext.of(context));
    }

    private void drawForgeBackpack(DrawContext context, OwoUIDrawContext owoContext, int x, int y) {
        forgeGridX = x;
        forgeGridY = y + FORGE_GRID_PAD;
        int cell = GridSlotComponent.CELL_SIZE;
        int width = FORGE_GRID_COLS * cell;
        int height = FORGE_GRID_ROWS * cell;

        context.drawText(textRenderer, Text.literal("§l背包残卷"), x, y + 4, 0xFFE8D8F4, true);
        context.fill(x - 2, forgeGridY - 2, x + width + 2, forgeGridY + height + 2, 0xAA0F1014);

        for (int row = 0; row < FORGE_GRID_ROWS; row++) {
            for (int col = 0; col < FORGE_GRID_COLS; col++) {
                int sx = forgeGridX + col * cell;
                int sy = forgeGridY + row * cell;
                context.fill(sx, sy, sx + cell, sy + cell,
                    ((row + col) & 1) == 0 ? 0xFF1E1E1E : 0xFF232323);
                drawSlotBorder(context, sx, sy, cell, cell, 0xFF3A3A3A);
            }
        }

        for (InventoryModel.GridEntry entry : primaryGridItems()) {
            InventoryItem item = entry.item();
            if (item == null || item.isEmpty()) continue;
            if (dragState.isDragging() && dragState.draggedItem() == item) continue;
            if (entry.row() < 0 || entry.row() >= FORGE_GRID_ROWS
                || entry.col() < 0 || entry.col() >= FORGE_GRID_COLS) continue;

            int sx = forgeGridX + entry.col() * cell;
            int sy = forgeGridY + entry.row() * cell;
            GridSlotComponent.drawItemTexture(owoContext, item, sx + 2, sy + 2, cell - 4, cell - 4);
            GridSlotComponent.drawItemOverlays(context, item, sx, sy, cell, cell);
        }
    }

    private void drawDraggedItem(DrawContext context, OwoUIDrawContext owoContext) {
        if (!dragState.isDragging() || dragState.draggedItem() == null) return;
        InventoryItem item = dragState.draggedItem();
        int size = GridSlotComponent.CELL_SIZE;
        int x = (int) dragState.mouseX() - size / 2;
        int y = (int) dragState.mouseY() - size / 2;
        GridSlotComponent.drawItemTexture(owoContext, item, x + 2, y + 2, size - 4, size - 4);
        GridSlotComponent.drawItemOverlays(context, item, x, y, size, size);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (button == GLFW.GLFW_MOUSE_BUTTON_LEFT && isInscriptionStep() && !dragState.isDragging()) {
            InventoryModel.GridEntry entry = forgeGridEntryAt(mouseX, mouseY);
            if (entry != null && entry.item() != null) {
                dragState.pickup(entry.item(), entry.containerId(), entry.row(), entry.col());
                dragState.updateMouse(mouseX, mouseY);
                return true;
            }
        }
        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double deltaX, double deltaY) {
        if (dragState.isDragging()) {
            dragState.updateMouse(mouseX, mouseY);
            return true;
        }
        return super.mouseDragged(mouseX, mouseY, button, deltaX, deltaY);
    }

    @Override
    public boolean mouseReleased(double mouseX, double mouseY, int button) {
        if (button == GLFW.GLFW_MOUSE_BUTTON_LEFT && dragState.isDragging()) {
            InventoryItem dragged = dragState.draggedItem();
            if (inscriptionPanel.isInBoundingBox(mouseX, mouseY)
                && inscriptionPanel.tryDropScroll(dragged)) {
                dragState.drop();
                return true;
            }
            dragState.cancel();
            return true;
        }
        return super.mouseReleased(mouseX, mouseY, button);
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (TemperingInputHandler.handleKey(this, keyCode)) {
            return true;
        }
        if (keyCode == 85) { // U
            this.close();
            return true;
        }
        if (keyCode == 263) { // ←
            BlueprintScrollStore.turn(-1);
            return true;
        }
        if (keyCode == 262) { // →
            BlueprintScrollStore.turn(1);
            return true;
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    private boolean isInscriptionStep() {
        return "inscription".equals(ForgeSessionStore.snapshot().currentStep());
    }

    private List<InventoryModel.GridEntry> primaryGridItems() {
        return InventoryStateStore.snapshot().gridItems().stream()
            .filter(entry -> InventoryModel.PRIMARY_CONTAINER_ID.equals(entry.containerId()))
            .toList();
    }

    private InventoryModel.GridEntry forgeGridEntryAt(double mouseX, double mouseY) {
        if (forgeGridX < 0 || forgeGridY < 0) return null;
        int cell = GridSlotComponent.CELL_SIZE;
        int col = (int) ((mouseX - forgeGridX) / cell);
        int row = (int) ((mouseY - forgeGridY) / cell);
        if (row < 0 || row >= FORGE_GRID_ROWS || col < 0 || col >= FORGE_GRID_COLS) {
            return null;
        }
        for (InventoryModel.GridEntry entry : primaryGridItems()) {
            InventoryItem item = entry.item();
            if (item == null) continue;
            if (row >= entry.row() && row < entry.row() + item.gridHeight()
                && col >= entry.col() && col < entry.col() + item.gridWidth()) {
                return entry;
            }
        }
        return null;
    }

    private static void drawSlotBorder(DrawContext context, int x, int y, int w, int h, int color) {
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }
}
