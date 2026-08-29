package com.bong.client.inventory;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiScreenScope;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

/**
 * Standalone loot panel component for supply coffin interaction.
 *
 * <p>Not a screen — gets mounted into InspectScreen's outerRow when
 * a coffin session is active. Encapsulates the loot grid, timer bar,
 * and screen-level state/intent boundary.</p>
 */
public final class LootContainerPanel {
    private static final int BG_COLOR = 0xFF0D0D15;
    private static final int BORDER_COLOR = 0xFF4A4050;
    private static final int TIMER_BAR_BG = 0xFF333333;
    private static final int TIMER_BAR_FILL = 0xFFCC8833;
    private static final int TIMER_BAR_URGENT = 0xFFCC3333;

    private final long sessionId;
    private final String sourceKind;
    private final String grade;
    private final int lootRows;
    private final int lootCols;
    private final long timeoutWallSecs;
    private final long openedAtSecs;
    private final String extContainerId;

    private BackpackGridPanel lootGrid;
    private LabelComponent timerLabel;
    private FlowLayout timerBarFill;
    private FlowLayout container; // root FlowLayout of this panel
    private final LootContainerScreenController controller;
    private final DefaultUiScreenScope scope = new DefaultUiScreenScope();
    private boolean closed;

    public LootContainerPanel(LootContainerStateStore.OpenSession session) {
        this.sessionId = session.sessionId();
        this.sourceKind = session.sourceKind();
        this.grade = session.grade();
        this.lootRows = session.rows();
        this.lootCols = session.cols();
        this.timeoutWallSecs = session.timeoutWallSecs();
        this.openedAtSecs = System.currentTimeMillis() / 1000;
        this.extContainerId = "ext_" + sessionId;
        this.controller = LootContainerScreenController.production(session, this::applyViewModel,
            LootContainerPanel::executeOnClientThread);
    }

    /**
     * Build the panel UI. Returns the root FlowLayout to be added to InspectScreen's outerRow.
     */
    public FlowLayout build() {
        container = Containers.verticalFlow(Sizing.content(), Sizing.content());
        container.surface(Surface.flat(BG_COLOR).and(Surface.outline(BORDER_COLOR)));
        container.padding(Insets.of(8));
        container.gap(4);

        container.child(Components.label(Text.literal(containerTitle() + " §r— 容器物品"))
            .horizontalTextAlignment(HorizontalAlignment.CENTER));

        // Loot grid
        lootGrid = new BackpackGridPanel(extContainerId, lootRows, lootCols);
        populateLootGrid(controller.viewModel().openSession());
        container.child(lootGrid.container());

        // Timer bar
        int timerWidth = lootCols * GridSlotComponent.CELL_SIZE;
        FlowLayout timerBarContainer = Containers.horizontalFlow(Sizing.fixed(timerWidth), Sizing.fixed(6));
        timerBarContainer.surface(Surface.flat(TIMER_BAR_BG));
        timerBarFill = Containers.horizontalFlow(Sizing.fixed(timerWidth), Sizing.fixed(6));
        timerBarFill.surface(Surface.flat(TIMER_BAR_FILL));
        timerBarContainer.child(timerBarFill);
        container.child(timerBarContainer);

        timerLabel = Components.label(Text.literal(""));
        timerLabel.color(Color.ofArgb(0xFFAAAAAA));
        container.child(timerLabel);

        updateTimer();
        if (!scope.isOpen()) {
            scope.onOpen();
            controller.onOpen(scope);
        }
        return container;
    }

    /** The root FlowLayout (available after {@link #build()}). */
    public FlowLayout container() { return container; }

    /** The loot grid, exposed for drag-and-drop handling in InspectScreen. */
    public BackpackGridPanel lootGrid() { return lootGrid; }

    private String containerTitle() {
        return switch (sourceKind) {
            case "storage_crate" -> "herb".equals(grade) ? "§a灵草箱" : "§6货箱";
            case "dead_drop" -> "§3死信箱";
            default -> switch (grade) {
                case "rare" -> "§6漆棺";
                case "precious" -> "§e祭坛棺";
                default -> "§7松木棺";
            };
        };
    }

    /** External container id used for move intents. */
    public String extContainerId() { return extContainerId; }

    /** Session id for this loot session. */
    public long sessionId() { return sessionId; }

    /** Whether this panel has been disposed. */
    public boolean isClosed() { return closed; }

    /**
     * Send an external container move request to the server.
     */
    public void sendMove(long instanceId, String fromContainer, int fromRow, int fromCol,
                         String toContainer, int toRow, int toCol) {
        controller.intentSink().dispatch(new LootContainerIntent.Move(
            sessionId, instanceId, fromContainer, fromRow, fromCol, toContainer, toRow, toCol
        ));
    }

    /**
     * Send an external container close request to the server.
     */
    public void sendClose() {
        controller.intentSink().dispatch(new LootContainerIntent.Close(sessionId));
    }

    /**
     * Called by InspectScreen.tick() to update the timer.
     * @return true if the timer has expired (caller should unmount)
     */
    public boolean tickTimer() {
        return updateTimer();
    }

    /**
     * 关闭 screen-level source/controller。必须在从宿主卸载时调用。
     */
    public void dispose() {
        if (scope != null && !scope.isClosed()) scope.close();
        controller.onClose();
        closed = true;
    }

    private void applyViewModel(LootContainerScreenViewModel model) {
        if (closed) return;
        if (model.session() instanceof LootContainerStateStore.OpenSession open
            && open.sessionId() == sessionId) {
            populateLootGrid(open);
        } else {
            // 宿主下一次收到 open/close 事件时会卸载这个已过期 panel。
            dispose();
        }
    }

    private static void executeOnClientThread(Runnable action) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) action.run();
        else client.execute(action);
    }

    private void populateLootGrid(LootContainerStateStore.OpenSession open) {
        lootGrid.clearAll();
        if (open != null && open.sessionId() == sessionId) {
            for (InventoryModel.GridEntry entry : open.placedItems()) {
                lootGrid.place(entry.item(), entry.row(), entry.col());
            }
        }
    }

    /**
     * @return true if the timer has expired
     */
    private boolean updateTimer() {
        if (timerLabel == null || timerBarFill == null) return false;
        if (timeoutWallSecs == 0) {
            timerLabel.text(Text.literal("无时限"));
            timerBarFill.sizing(
                Sizing.fixed(lootCols * GridSlotComponent.CELL_SIZE),
                Sizing.fixed(6)
            );
            timerBarFill.surface(Surface.flat(TIMER_BAR_FILL));
            return false;
        }
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

        return remaining <= 0;
    }
}
