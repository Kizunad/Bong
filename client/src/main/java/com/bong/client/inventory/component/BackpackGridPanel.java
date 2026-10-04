package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.core.Sizing;

import java.util.ArrayList;
import java.util.List;

/**
 * 一个库存容器的二维网格投影。
 *
 * <p>网格只维护客户端显示所需的占位关系，并把锚点转换回 {@link InventoryModel.GridEntry}；
 * 服务端库存事务仍由 {@code InspectScreen} 的请求路径负责。布局创建、占位计算和模型同步
 * 分开，便于回读拖放边界而不把 wire 语义藏在 UI 组件里。</p>
 */
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

        container = buildGridLayout();
    }

    /** 创建固定尺寸的行列布局，并为每个格子登记坐标。 */
    private FlowLayout buildGridLayout() {
        FlowLayout grid = Containers.verticalFlow(
            Sizing.fixed(cols * GridSlotComponent.CELL_SIZE),
            Sizing.fixed(rows * GridSlotComponent.CELL_SIZE)
        );
        grid.gap(0);

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

            grid.child(row);
        }
        return grid;
    }

    /** 返回网格行数。 */
    public int rows() { return rows; }
    /** 返回网格列数。 */
    public int cols() { return cols; }
    /** 返回服务端快照使用的容器 ID。 */
    public String containerId() { return containerId; }
    /** 返回用于挂载到 owo 布局的根容器。 */
    public FlowLayout container() { return container; }

    /** 返回坐标对应的格子；越界坐标返回 {@code null}。 */
    public GridSlotComponent slotAt(int row, int col) {
        if (row < 0 || row >= rows || col < 0 || col >= cols) return null;
        return slots[row][col];
    }

    /** 判断物品完整尺寸能否放入目标区域，既不修改网格也不发请求。 */
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

    /** 在本地投影中放置物品，并只把左上格标为锚点。 */
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

    /** 从本地投影中移除物品覆盖的全部格子。 */
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

    /** 返回坐标所在物品；空格或越界返回 {@code null}。 */
    public InventoryItem itemAt(int row, int col) {
        if (row < 0 || row >= rows || col < 0 || col >= cols) return null;
        return occupied[row][col];
    }

    /** 返回物品的锚点坐标，用于生成移动请求的来源位置。 */
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

    /** 按行优先寻找能容纳物品的第一个空区域。 */
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

    /** 用快照重建本地网格；属于纯显示同步，不改变快照本身。 */
    public void populateFromModel(InventoryModel model) {
        clearAll();
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            if (!containerId.equals(entry.containerId())) {
                continue;
            }
            place(entry.item(), entry.row(), entry.col());
        }
    }

    /** 清空所有本地占位和格子显示。 */
    public void clearAll() {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                occupied[r][c] = null;
                slots[r][c].clearItem();
            }
        }
    }

    /** 清除拖放高亮，不触碰物品占位。 */
    public void clearHighlights() {
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                slots[r][c].setHighlightState(GridSlotComponent.HighlightState.NONE);
            }
        }
    }

    /** 在指定区域设置拖放高亮，越界部分会被裁剪。 */
    public void highlightArea(int row, int col, int w, int h, GridSlotComponent.HighlightState state) {
        for (int r = row; r < Math.min(rows, row + h); r++) {
            for (int c = col; c < Math.min(cols, col + w); c++) {
                if (r >= 0 && c >= 0) {
                    slots[r][c].setHighlightState(state);
                }
            }
        }
    }

    /** 将屏幕坐标转换成网格坐标，落在网格外返回 {@code null}。 */
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

    /** 判断屏幕坐标是否落在网格矩形内。 */
    public boolean containsPoint(double screenX, double screenY) {
        int baseX = container.x();
        int baseY = container.y();
        return screenX >= baseX && screenX < baseX + cols * GridSlotComponent.CELL_SIZE
            && screenY >= baseY && screenY < baseY + rows * GridSlotComponent.CELL_SIZE;
    }

    /** 只导出锚点格，避免多格物品在快照投影中重复。 */
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

    /** 网格内的行列坐标，供拖放和快照桥接使用。 */
    public record GridPosition(int row, int col) {}
}
