package com.bong.client.inventory;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.DragState;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/**
 * plan-supply-coffin-loot-ui P3 — 物资棺搜刮 UI。
 *
 * <p>左侧：玩家容器列表（body_pocket + 已装备的背包等）</p>
 * <p>右侧：棺材 loot 格子（根据 grade 不同大小）</p>
 * <p>底部：倒计时进度条</p>
 * <p>拖拽：共享 DragState，drop 时通过 ClientRequestSender 发送 ExternalContainerMove</p>
 */
public final class LootContainerScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("物资棺");
    private static final int BG_COLOR = 0xFF0D0D15;
    private static final int BORDER_COLOR = 0xFF4A4050;
    private static final int TIMER_BAR_BG = 0xFF333333;
    private static final int TIMER_BAR_FILL = 0xFFCC8833;
    private static final int TIMER_BAR_URGENT = 0xFFCC3333;

    private final long sessionId;
    private final String grade;
    private final int lootRows;
    private final int lootCols;
    private final long timeoutWallSecs;
    private final long openedAtSecs;
    private final String extContainerId;

    private final DragState dragState = new DragState();

    private BackpackGridPanel lootGrid;
    private final List<BackpackGridPanel> playerGrids = new ArrayList<>();
    private LabelComponent timerLabel;
    private FlowLayout timerBarFill;
    private FlowLayout timerBarContainer;

    private Consumer<InventoryModel> inventoryListener;
    private LootContainerStateStore.Listener storeListener;

    public LootContainerScreen(LootContainerStateStore.OpenSession session) {
        super(TITLE);
        this.sessionId = session.sessionId();
        this.grade = session.grade();
        this.lootRows = session.rows();
        this.lootCols = session.cols();
        this.timeoutWallSecs = session.timeoutWallSecs();
        this.openedAtSecs = System.currentTimeMillis() / 1000;
        this.extContainerId = "ext_" + sessionId;
    }

    long sessionId() { return sessionId; }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.content(), Sizing.content());
        panel.surface(Surface.flat(BG_COLOR).and(Surface.outline(BORDER_COLOR)));
        panel.padding(Insets.of(8));
        panel.gap(4);

        // Title
        String gradeLabel = switch (grade) {
            case "rare" -> "§6漆棺";
            case "precious" -> "§e祭坛棺";
            default -> "§7松木棺";
        };
        panel.child(Components.label(Text.literal(gradeLabel + " §r— 物资搜刮"))
            .horizontalTextAlignment(HorizontalAlignment.CENTER));

        // Main content: [player containers] | [loot grid]
        FlowLayout content = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        content.gap(8);
        content.verticalAlignment(VerticalAlignment.TOP);

        // Left: player containers
        FlowLayout leftCol = Containers.verticalFlow(Sizing.content(), Sizing.content());
        leftCol.gap(4);
        leftCol.child(Components.label(Text.literal("§f背包")));

        InventoryModel model = InventoryStateStore.snapshot();
        if (model != null) {
            for (InventoryModel.ContainerDef cdef : model.containers()) {
                BackpackGridPanel grid = new BackpackGridPanel(cdef.id(), cdef.rows(), cdef.cols());
                grid.populateFromModel(model);
                playerGrids.add(grid);
                leftCol.child(Components.label(Text.literal("§7" + cdef.name()))
                    .margins(Insets.of(2, 0, 0, 0)));
                leftCol.child(grid.container());
            }
        }
        content.child(leftCol);

        // Separator
        content.child(Containers.verticalFlow(
            Sizing.fixed(1),
            Sizing.fixed(lootRows * GridSlotComponent.CELL_SIZE + 20))
            .surface(Surface.flat(0xFF555555)));

        // Right: loot grid
        FlowLayout rightCol = Containers.verticalFlow(Sizing.content(), Sizing.content());
        rightCol.gap(4);
        rightCol.child(Components.label(Text.literal("§f棺内物品")));

        lootGrid = new BackpackGridPanel(extContainerId, lootRows, lootCols);
        populateLootGrid();
        rightCol.child(lootGrid.container());

        // Timer bar
        int timerWidth = lootCols * GridSlotComponent.CELL_SIZE;
        timerBarContainer = Containers.horizontalFlow(Sizing.fixed(timerWidth), Sizing.fixed(6));
        timerBarContainer.surface(Surface.flat(TIMER_BAR_BG));
        timerBarFill = Containers.horizontalFlow(Sizing.fixed(timerWidth), Sizing.fixed(6));
        timerBarFill.surface(Surface.flat(TIMER_BAR_FILL));
        timerBarContainer.child(timerBarFill);
        rightCol.child(timerBarContainer);

        timerLabel = Components.label(Text.literal(""));
        timerLabel.color(Color.ofArgb(0xFFAAAAAA));
        rightCol.child(timerLabel);

        content.child(rightCol);
        panel.child(content);
        root.child(panel);

        // Listen for inventory changes
        inventoryListener = inv -> {
            MinecraftClient.getInstance().execute(() -> {
                if (MinecraftClient.getInstance().currentScreen == this) {
                    refreshPlayerGrids(inv);
                }
            });
        };
        InventoryStateStore.addListener(inventoryListener);

        // Listen for loot container state changes
        storeListener = session -> {
            MinecraftClient.getInstance().execute(() -> {
                if (session instanceof LootContainerStateStore.Closed) {
                    close();
                } else if (session instanceof LootContainerStateStore.OpenSession open
                    && open.sessionId() == sessionId) {
                    populateLootGrid();
                }
            });
        };
        LootContainerStateStore.addListener(storeListener);

        updateTimer();
    }

    private void populateLootGrid() {
        lootGrid.clearAll();
        LootContainerStateStore.Session session = LootContainerStateStore.current();
        if (session instanceof LootContainerStateStore.OpenSession open
            && open.sessionId() == sessionId) {
            for (InventoryModel.GridEntry entry : open.placedItems()) {
                lootGrid.place(entry.item(), entry.row(), entry.col());
            }
        }
    }

    private void refreshPlayerGrids(InventoryModel model) {
        for (BackpackGridPanel grid : playerGrids) {
            grid.clearAll();
            grid.populateFromModel(model);
        }
    }

    @Override
    public void tick() {
        super.tick();
        updateTimer();
    }

    private void updateTimer() {
        if (timerLabel == null || timerBarFill == null) return;
        long now = System.currentTimeMillis() / 1000;
        long remaining = Math.max(0, timeoutWallSecs - now);
        long totalDuration = Math.max(1, timeoutWallSecs - openedAtSecs);

        timerLabel.text(Text.literal(String.format("剩余 %d 秒", remaining)));

        double ratio = totalDuration > 0 ? (double) remaining / totalDuration : 0;
        ratio = Math.max(0, Math.min(1, ratio));
        int fullWidth = lootCols * GridSlotComponent.CELL_SIZE;
        int fillWidth = Math.max(1, (int) (fullWidth * ratio));
        timerBarFill.sizing(Sizing.fixed(fillWidth), Sizing.fixed(6));

        int fillColor = remaining <= 10 ? TIMER_BAR_URGENT : TIMER_BAR_FILL;
        timerBarFill.surface(Surface.flat(fillColor));

        if (remaining <= 0) {
            close();
        }
    }

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (button != 0) return super.mouseClicked(mouseX, mouseY, button);

        // Check loot grid click
        if (lootGrid.containsPoint(mouseX, mouseY)) {
            BackpackGridPanel.GridPosition pos = lootGrid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                InventoryItem item = lootGrid.itemAt(pos.row(), pos.col());
                if (item != null && dragState.phase() == DragState.Phase.IDLE) {
                    BackpackGridPanel.GridPosition anchor = lootGrid.anchorOf(item);
                    if (anchor != null) {
                        dragState.pickup(item, extContainerId, anchor.row(), anchor.col());
                        lootGrid.remove(item);
                        return true;
                    }
                }
            }
        }

        // Check player grid click
        for (BackpackGridPanel grid : playerGrids) {
            if (grid.containsPoint(mouseX, mouseY)) {
                BackpackGridPanel.GridPosition pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = grid.itemAt(pos.row(), pos.col());
                    if (item != null && dragState.phase() == DragState.Phase.IDLE) {
                        BackpackGridPanel.GridPosition anchor = grid.anchorOf(item);
                        if (anchor != null) {
                            dragState.pickup(item, grid.containerId(), anchor.row(), anchor.col());
                            grid.remove(item);
                            return true;
                        }
                    }
                }
            }
        }

        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double deltaX, double deltaY) {
        if (dragState.phase() == DragState.Phase.DRAGGING) {
            dragState.updateMouse(mouseX, mouseY);
            updateDragHighlights(mouseX, mouseY);
            return true;
        }
        return super.mouseDragged(mouseX, mouseY, button, deltaX, deltaY);
    }

    @Override
    public boolean mouseReleased(double mouseX, double mouseY, int button) {
        if (button != 0 || dragState.phase() != DragState.Phase.DRAGGING) {
            return super.mouseReleased(mouseX, mouseY, button);
        }

        InventoryItem item = dragState.draggedItem();
        String sourceContainerId = dragState.sourceContainerId();
        int sourceRow = dragState.sourceRow();
        int sourceCol = dragState.sourceCol();

        // Try drop on loot grid
        if (lootGrid.containsPoint(mouseX, mouseY)) {
            BackpackGridPanel.GridPosition pos = lootGrid.screenToGrid(mouseX, mouseY);
            if (pos != null && lootGrid.canPlace(item, pos.row(), pos.col())) {
                lootGrid.place(item, pos.row(), pos.col());
                sendMove(item.instanceId(), sourceContainerId, sourceRow, sourceCol,
                    extContainerId, pos.row(), pos.col());
                dragState.cancel();
                clearAllHighlights();
                return true;
            }
        }

        // Try drop on player grids
        for (BackpackGridPanel grid : playerGrids) {
            if (grid.containsPoint(mouseX, mouseY)) {
                BackpackGridPanel.GridPosition pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null && grid.canPlace(item, pos.row(), pos.col())) {
                    grid.place(item, pos.row(), pos.col());
                    sendMove(item.instanceId(), sourceContainerId, sourceRow, sourceCol,
                        grid.containerId(), pos.row(), pos.col());
                    dragState.cancel();
                    clearAllHighlights();
                    return true;
                }
            }
        }

        // Cancel — put item back
        restoreItem(item, sourceContainerId, sourceRow, sourceCol);
        dragState.cancel();
        clearAllHighlights();
        return true;
    }

    private void sendMove(long instanceId, String fromContainer, int fromRow, int fromCol,
                          String toContainer, int toRow, int toCol) {
        ClientRequestSender.sendExternalContainerMove(
            sessionId,
            instanceId,
            new ClientRequestProtocol.ContainerLoc(fromContainer, fromRow, fromCol),
            new ClientRequestProtocol.ContainerLoc(toContainer, toRow, toCol)
        );
    }

    private void restoreItem(InventoryItem item, String containerId, int row, int col) {
        if (extContainerId.equals(containerId)) {
            lootGrid.place(item, row, col);
        } else {
            for (BackpackGridPanel grid : playerGrids) {
                if (grid.containerId().equals(containerId)) {
                    grid.place(item, row, col);
                    return;
                }
            }
        }
    }

    private void updateDragHighlights(double mouseX, double mouseY) {
        clearAllHighlights();
        InventoryItem item = dragState.draggedItem();
        if (item == null) return;

        if (lootGrid.containsPoint(mouseX, mouseY)) {
            BackpackGridPanel.GridPosition pos = lootGrid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                var state = lootGrid.canPlace(item, pos.row(), pos.col())
                    ? GridSlotComponent.HighlightState.VALID
                    : GridSlotComponent.HighlightState.INVALID;
                lootGrid.highlightArea(pos.row(), pos.col(),
                    item.gridWidth(), item.gridHeight(), state);
            }
        }

        for (BackpackGridPanel grid : playerGrids) {
            if (grid.containsPoint(mouseX, mouseY)) {
                BackpackGridPanel.GridPosition pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    var state = grid.canPlace(item, pos.row(), pos.col())
                        ? GridSlotComponent.HighlightState.VALID
                        : GridSlotComponent.HighlightState.INVALID;
                    grid.highlightArea(pos.row(), pos.col(),
                        item.gridWidth(), item.gridHeight(), state);
                }
            }
        }
    }

    private void clearAllHighlights() {
        lootGrid.clearHighlights();
        for (BackpackGridPanel grid : playerGrids) {
            grid.clearHighlights();
        }
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (keyCode == GLFW.GLFW_KEY_ESCAPE) {
            ClientRequestSender.sendExternalContainerClose(sessionId);
            close();
            return true;
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    @Override
    public void close() {
        if (inventoryListener != null) {
            InventoryStateStore.removeListener(inventoryListener);
            inventoryListener = null;
        }
        if (storeListener != null) {
            LootContainerStateStore.removeListener(storeListener);
            storeListener = null;
        }
        super.close();
    }
}
