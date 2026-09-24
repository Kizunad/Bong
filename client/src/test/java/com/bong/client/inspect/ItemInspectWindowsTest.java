package com.bong.client.inspect;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.state.StoreUiStateSource;
import com.bong.client.ui.window.UiWindowManager;
import org.junit.jupiter.api.Test;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import static org.junit.jupiter.api.Assertions.*;

class ItemInspectWindowsTest {
    @Test
    void sharedInventoryRefreshPreservesIdentityAndRevokesMissingItemsWhileHostIsAbsent() {
        var firstItem = item(101L, 1);
        var secondItem = item(102L, 1);
        var snapshot = new AtomicReference<>(InventoryModel.builder()
            .gridItem(firstItem, 0, 0).gridItem(secondItem, 0, 1).build());
        var manager = new UiWindowManager(640, 360);
        var external = new AtomicReference<>(List.of(item(103L, 1)));
        var windows = new ItemInspectWindows(manager, StoreUiStateSource.pullOnOpen(snapshot::get),
            StoreUiStateSource.pullOnOpen(external::get));
        var bounds = new UiWindowManager.Rect(0, 0, 260, 280);
        var first = windows.open(101L, bounds);
        var second = windows.open(102L, bounds);
        var loot = windows.open(103L, bounds);
        assertSame(first, windows.open(101L, bounds));
        manager.pin(second.key(), true);
        manager.minimize(second.key());
        manager.cancelCapture();
        assertFalse(first.scope().isClosed(), "离开宿主仅取消输入，业务窗口仍然存活");

        var updated = item(101L, 3);
        snapshot.set(InventoryModel.builder().gridItem(updated, 0, 2).build());
        external.set(List.of());
        windows.refresh();
        assertSame(updated, windows.item(first.key()), "位置变化与数量更新不能换成另一 instance");
        assertTrue(second.scope().isClosed(), "固定且最小化的窗口也必须消费物品失效");
        assertTrue(loot.scope().isClosed(), "外部容器会话结束后，其物品详情必须失效");
        assertNull(windows.item(second.key()));
        assertNull(windows.open(102L, bounds), "旧点击不得重建已失效物品");
        manager.close(first.key());
        assertNull(windows.item(first.key()));
    }

    private static InventoryItem item(long id, int count) {
        return InventoryItem.createFullWithForgeMeta(id, "rust_iron", "锈铁片", 1, 1, 1,
            "common", "材料", count, 1, 1, "", "", 0, null, "", List.of(), null);
    }
}
