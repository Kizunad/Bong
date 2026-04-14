package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.core.Sizing;

import java.util.ArrayList;
import java.util.List;

public class BackpackGridPanel {
    public static final int DEFAULT_ROWS = InventoryModel.GRID_ROWS;
    public static final int DEFAULT_COLS = InventoryModel.GRID_COLS;

    private final int rows;
    private final int cols;
    private final String containerId;
    private final GridSlotComponent[][] slots;
    private final InventoryItem[][] occupied;
    private final FlowLayout container;

    public BackpackGridPanel() {
        this(InventoryModel.PRIMARY_CONTAINER_ID, DEFAULT_ROWS, DEFAULT_COLS);
    }

    public BackpackGridPanel(int rows, int cols) {
        this(InventoryModel.PRIMARY_CONTAINER_ID, rows, cols);
    }

    public BackpackGridPanel(String containerId, int rows, int cols) {
        this.rows = rows;
        this.cols = cols;
        this.containerId = containerId == null || containerId.isBlank()
            ? InventoryModel.PRIMARY_CONTAINER_ID
            : containerId;
        this.slots = new GridSlotComponent[rows][cols];
        this.occupied = new InventoryItem[rows][cols];

        container = Containers.verticalFlow(
            Sizing.fixed(cols * GridSlotComponent.CELL_SIZE),
            Sizing.fixed(rows * GridSlotComponent.CELL_SIZE)
        );
        container.gap(0);

        for (int r = 0; r < rows; r++) {
            FlowLayout row = Containers.horizontalFlow(
                Sizing.fixed(cols * GridSlotComponent.CELL_SIZE),
                Sizing.fixed(GridSlotComponent.CELL_SIZE)
            );
            row.gap(0);

            for (int c = 0; c < cols; c++) {
                GridSlotComponent slot = new GridSlotComponent(r, c);
                slots[r][c] = slot;
                row.child(slot);
            }

            container.child(row);
        }
    }

    public int rows() { return rows; }
    public int cols() { return cols; }
    public String containerId() { return containerId; }
    public FlowLayout container() { return container; }

    public GridSlotComponent slotAt(int row, int col) {
        if (row < 0 || row >= rows || col < 0 || col >= cols) return null;
        return slots[row][col];
    }

    public boolean canPlace(InventoryItem item, int row, int col) {
        if (item == null) return false;
        int w = item.gridWidth();
        int h = item.gridHeight();
        if (row < 0 || row + h > rows || col < 0 || col + w > cols) return false;

        for (int r = row; r < row + h; r++) {
            for (int c = col; c < col + w; c++) {
                if (occupied[r][c] != null) return false;
            }
        }
        return true;
    }

    public void place(InventoryItem item, int row, int col) {
        int w = item.gridWidth();
        int h = item.gridHeight();

        for (int r = row; r < row + h; r++) {
            for (int c = col; c < col + w; c++) {
                occupied[r][c] = item;
                slots[r][c].setItem(item, r == row && c == col);
            }
        }
    }

    public void remove(InventoryItem item) {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                if (occupied[r][c] == item) {
                    occupied[r][c] = null;
                    slots[r][c].clearItem();
                }
            }
        }
    }

    public InventoryItem itemAt(int row, int col) {
        if (row < 0 || row >= rows || col < 0 || col >= cols) return null;
        return occupied[row][col];
    }

    public GridPosition anchorOf(InventoryItem item) {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                if (occupied[r][c] == item && slots[r][c].isAnchor()) {
                    return new GridPosition(r, c);
                }
            }
        }
        return null;
    }

    public GridPosition findFreeSpace(InventoryItem item) {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                if (canPlace(item, r, c)) {
                    return new GridPosition(r, c);
                }
            }
        }
        return null;
    }

    public void populateFromModel(InventoryModel model) {
        clearAll();
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            if (!containerId.equals(entry.containerId())) {
                continue;
            }
            place(entry.item(), entry.row(), entry.col());
        }
    }

    public void clearAll() {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                occupied[r][c] = null;
                slots[r][c].clearItem();
            }
        }
    }

    public void clearHighlights() {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                slots[r][c].setHighlightState(GridSlotComponent.HighlightState.NONE);
            }
        }
    }

    public void highlightArea(int row, int col, int w, int h, GridSlotComponent.HighlightState state) {
        for (int r = row; r < Math.min(rows, row + h); r++) {
            for (int c = col; c < Math.min(cols, col + w); c++) {
                if (r >= 0 && c >= 0) {
                    slots[r][c].setHighlightState(state);
                }
            }
        }
    }

    public GridPosition screenToGrid(double screenX, double screenY) {
        int baseX = container.x();
        int baseY = container.y();
        int col = (int) ((screenX - baseX) / GridSlotComponent.CELL_SIZE);
        int row = (int) ((screenY - baseY) / GridSlotComponent.CELL_SIZE);
        if (row >= 0 && row < rows && col >= 0 && col < cols) {
            return new GridPosition(row, col);
        }
        return null;
    }

    public boolean containsPoint(double screenX, double screenY) {
        int baseX = container.x();
        int baseY = container.y();
        return screenX >= baseX && screenX < baseX + cols * GridSlotComponent.CELL_SIZE
            && screenY >= baseY && screenY < baseY + rows * GridSlotComponent.CELL_SIZE;
    }

    public List<InventoryModel.GridEntry> toGridEntries() {
        List<InventoryModel.GridEntry> entries = new ArrayList<>();
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                if (occupied[r][c] != null && slots[r][c].isAnchor()) {
                    entries.add(new InventoryModel.GridEntry(occupied[r][c], containerId, r, c));
                }
            }
        }
        return entries;
    }

    public record GridPosition(int row, int col) {}
}
