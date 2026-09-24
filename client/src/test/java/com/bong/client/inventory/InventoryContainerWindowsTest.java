package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.state.StoreUiStateSource;
import com.bong.client.ui.window.UiWindowManager;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class InventoryContainerWindowsTest {
    private final UiWindowManager manager = new UiWindowManager(683, 384);
    private final AtomicReference<InventoryModel> source = new AtomicReference<>();
    private final InventoryContainerWindows windows = new InventoryContainerWindows(
        manager, StoreUiStateSource.pullOnOpen(source::get));
    private final UiWindowManager.Rect bounds = new UiWindowManager.Rect(20, 20, 220, 200);

    @Test
    void disappearingContainerClosesPinnedMinimizedWindowAndCannotReopen() {
        source.set(model(3, 3));
        var state = windows.open("pack_7", bounds);
        manager.pin(state.key(), true);
        manager.minimize(state.key());
        source.set(InventoryModel.builder().containers(List.of()).build());
        windows.refresh();

        assertTrue(state.scope().isClosed(), "权威库存移除容器必须结束其窗口，即使窗口已最小化或固定");
        assertNull(windows.grid("pack_7"), "旧容器不能继续作为拖放目标");
        assertNull(windows.open("pack_7", bounds), "缺失容器不能通过旧入口重新打开");
    }

    @Test
    void resizingContainerKeepsWindowIdentityAndRefreshesOnlyItsOwnItems() {
        source.set(model(3, 3));
        var state = windows.open("pack_7", bounds);
        manager.pin(state.key(), true);
        manager.minimize(state.key());
        source.set(model(2, 4));
        windows.refresh();

        var grid = windows.grid("pack_7");
        assertEquals(4, grid.cols(), "容器容量变化必须更新可操作范围");
        assertEquals(List.of(101L), grid.toGridEntries().stream().map(entry -> entry.item().instanceId()).toList(),
            "容器刷新不能混入其他容器中的物品");
        assertSame(state, windows.open("pack_7", bounds), "容量变化与重开不能复制业务窗口");
        assertTrue(state.pinned());
        assertFalse(state.minimized(), "显式重开应恢复原窗口");
        assertFalse(state.scope().isClosed());
    }

    private static InventoryModel model(int rows, int cols) {
        return InventoryModel.builder().containers(List.of(
            new InventoryModel.ContainerDef("pack_7", "破草包", rows, cols, 7L),
            new InventoryModel.ContainerDef("body_pocket", "贴身口袋", 2, 3)))
            .gridItem(item(101L), "pack_7", 0, 0)
            .gridItem(item(102L), "body_pocket", 0, 0).build();
    }

    private static InventoryItem item(long id) {
        return InventoryItem.createFull(id, "herb_bundle", "草药", 1, 1, 0.5, "common", "", 1, 1, 1);
    }
}
