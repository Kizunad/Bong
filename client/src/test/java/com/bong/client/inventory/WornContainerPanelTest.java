package com.bong.client.inventory;

import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 P3 —— WornContainerPanel 饱和化单测。
 *
 * <p>覆盖：containerId 驱动 grid 尺寸/populate、canPlace 边界、InventoryStateStore listener
 * 注册 + dispose 解绑（断言解绑后不再收回调）、构造期 null 防御。</p>
 *
 * <p>注：owo 组件可在 headless 下构造（同 BackpackGridPanel/GridSlotComponent 既有测试），
 * 几何方法（containsPoint/screenToGrid）依赖布局坐标不在此测；本测试聚焦数据/订阅契约。</p>
 */
public class WornContainerPanelTest {

    private static final String PACK_ID = "pack_1007";

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    private static InventoryItem item(long instanceId, String id, int w, int h) {
        return InventoryItem.createFull(
            instanceId, id, id, w, h, 0.5, "common", "", 1, 1.0, 1.0);
    }

    /** pack_1007 容器（3×3）含一件物品；其它容器物品不应落入本容器视图。 */
    private static InventoryModel modelWithPack() {
        return InventoryModel.builder()
            .containers(java.util.List.of(
                new InventoryModel.ContainerDef("main_pack", "主背包", 5, 7, null),
                new InventoryModel.ContainerDef(PACK_ID, "破草包", 3, 3, 1007L)
            ))
            .gridItem(item(2001, "spirit_herb", 1, 1), PACK_ID, 0, 0)
            .gridItem(item(2002, "field_ration", 1, 1), "main_pack", 1, 1)
            .build();
    }

    @Test
    void constructorRejectsBlankContainerId() {
        assertThrows(IllegalArgumentException.class,
            () -> new WornContainerPanel("", InventoryModel.empty()));
        assertThrows(IllegalArgumentException.class,
            () -> new WornContainerPanel(null, InventoryModel.empty()));
    }

    @Test
    void buildExposesGridSizedFromContainerDefAndPopulatesOnlyOwnItems() {
        WornContainerPanel panel = new WornContainerPanel(PACK_ID, modelWithPack());
        panel.build();

        BackpackGridPanel grid = panel.grid();
        assertNotNull(grid, "build() 后 grid 应可用");
        assertEquals(PACK_ID, grid.containerId());
        assertEquals(3, grid.rows(), "grid 行数应取自 ContainerDef.rows");
        assertEquals(3, grid.cols(), "grid 列数应取自 ContainerDef.cols");

        // 本容器物品落位（spirit_herb 在 (0,0)），其它容器物品（field_ration）不应出现。
        assertNotNull(grid.itemAt(0, 0), "pack_1007 内的 spirit_herb 应被 populate");
        assertEquals("spirit_herb", grid.itemAt(0, 0).itemId());
        // 扫描整 grid 确认没有混入 main_pack 的物品。
        boolean hasForeign = false;
        for (int r = 0; r < grid.rows(); r++) {
            for (int c = 0; c < grid.cols(); c++) {
                InventoryItem it = grid.itemAt(r, c);
                if (it != null && "field_ration".equals(it.itemId())) hasForeign = true;
            }
        }
        assertFalse(hasForeign, "其它容器（main_pack）的物品不应落入 pack_1007 视图");
    }

    @Test
    void canPlaceRespectsGridBoundsAndOccupancy() {
        WornContainerPanel panel = new WornContainerPanel(PACK_ID, modelWithPack());
        panel.build();
        BackpackGridPanel grid = panel.grid();

        // (0,0) 已被 spirit_herb 占据 → 1×1 不可放。
        assertFalse(grid.canPlace(item(9001, "x", 1, 1), 0, 0),
            "已占据格子不可放");
        // 空闲格子 (2,2) → 可放 1×1。
        assertTrue(grid.canPlace(item(9002, "x", 1, 1), 2, 2),
            "空闲格子可放");
        // 越界：3×3 grid，2×2 件放在 (2,2) 会溢出右下边界 → 不可放（off-by-one 边界）。
        assertFalse(grid.canPlace(item(9003, "x", 2, 2), 2, 2),
            "2×2 件放 (2,2) 溢出 3×3 边界，不可放");
        // 边界内：2×2 件放 (1,1) 恰好占满到 (2,2) → 可放。
        assertTrue(grid.canPlace(item(9004, "x", 2, 2), 1, 1),
            "2×2 件放 (1,1) 恰在边界内，可放");
    }

    @Test
    void registersStoreListenerOnBuildAndRefreshesOnSnapshotChange() {
        // 初始 model 不含 pack 物品；build 后 store 推送含物品的快照 → grid 应回刷出物品。
        InventoryModel initial = InventoryModel.builder()
            .containers(java.util.List.of(
                new InventoryModel.ContainerDef(PACK_ID, "破草包", 3, 3, 1007L)))
            .build();
        WornContainerPanel panel = new WornContainerPanel(PACK_ID, initial);
        panel.build();
        BackpackGridPanel grid = panel.grid();
        assertNull(grid.itemAt(0, 0), "初始无物品");

        // store 推送新快照（含 pack 物品）。replace() 在当前线程同步 notify（测试线程即可见）。
        InventoryStateStore.replace(modelWithPack());
        assertNotNull(grid.itemAt(0, 0), "snapshot 回刷后 grid 应显示新物品（listener 已生效）");
        assertEquals("spirit_herb", grid.itemAt(0, 0).itemId());
    }

    @Test
    void disposeUnregistersListenerSoLaterSnapshotsNoLongerRefresh() {
        WornContainerPanel panel = new WornContainerPanel(PACK_ID, modelWithPack());
        panel.build();
        BackpackGridPanel grid = panel.grid();
        assertNotNull(grid.itemAt(0, 0));

        panel.dispose();
        assertTrue(panel.isClosed(), "dispose 后 isClosed 应为 true");

        // dispose 后推送一个【空 pack】快照：若 listener 仍在，会把 grid 清空；解绑后 grid 不应再变。
        InventoryModel emptyPack = InventoryModel.builder()
            .containers(java.util.List.of(
                new InventoryModel.ContainerDef(PACK_ID, "破草包", 3, 3, 1007L)))
            .build();
        InventoryStateStore.replace(emptyPack);

        assertNotNull(grid.itemAt(0, 0),
            "dispose 解绑后不应再收快照回调；grid 仍保留 dispose 前内容（spirit_herb 未被清）");
        assertEquals("spirit_herb", grid.itemAt(0, 0).itemId());
    }

    @Test
    void containerIdAccessorReflectsConstructorArg() {
        WornContainerPanel panel = new WornContainerPanel(PACK_ID, InventoryModel.empty());
        assertEquals(PACK_ID, panel.containerId());
        assertFalse(panel.isClosed(), "未 dispose 时 isClosed 应为 false");
    }
}
