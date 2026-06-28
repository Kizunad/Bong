package com.bong.client.inventory;

import com.bong.client.inventory.component.PackContainerWindow;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Component;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-floating-windows —— PackWindowManager 多开 / z-order / 去重 / dispose 契约回归。
 *
 * <p>用测试构造的 owo {@link FlowLayout} root，覆盖：open 去重（已开则置顶不重复 new）、open 多窗
 * z-order（map 顺序==渲染顺序，末尾=最上）、raise 重排、close 单窗 dispose + 出 map + 从 root 摘除、
 * closeAll 全 dispose + 清空。InspectScreen 自身 mouseClicked hit-test 需真布局，由真机兜底。</p>
 */
public class PackWindowManagerTest {

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    private static InventoryModel modelWith(String... packIds) {
        var defs = new java.util.ArrayList<InventoryModel.ContainerDef>();
        long owner = 1000L;
        for (String id : packIds) {
            defs.add(new InventoryModel.ContainerDef(id, "破草包", 3, 3, owner++));
        }
        return InventoryModel.builder().containers(defs).build();
    }

    private static FlowLayout newRoot() {
        return Containers.verticalFlow(Sizing.content(), Sizing.content());
    }

    @Test
    void openTwiceWithSameIdDedupesAndRaises() {
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        InventoryModel model = modelWith("pack_1", "pack_2");

        PackContainerWindow first = mgr.open("pack_1", model, 8, 8);
        PackContainerWindow second = mgr.open("pack_1", model, 24, 24);

        assertEquals(1, mgr.size(), "同 id 第二次 open 不应再 new（去重）");
        assertSame(first, second, "open 已开容器应返回既有窗口实例（置顶，不重建）");
        assertSame(first, root.children().get(root.children().size() - 1),
            "去重时仍 bringToFront → 该窗口在 root 末尾（最上）");
    }

    @Test
    void openDistinctIdsStacksInInsertionOrder() {
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        InventoryModel model = modelWith("pack_1", "pack_2");

        PackContainerWindow a = mgr.open("pack_1", model, 8, 8);
        PackContainerWindow b = mgr.open("pack_2", model, 24, 24);

        assertEquals(2, mgr.size(), "两个不同 id → 两窗");
        List<PackContainerWindow> ordered = mgr.ordered();
        assertSame(a, ordered.get(0), "先开的 pack_1 在前（z 更下）");
        assertSame(b, ordered.get(1), "后开的 pack_2 在末尾（z 最上）");
        assertTrue(mgr.contains("pack_1") && mgr.contains("pack_2"));
    }

    @Test
    void raiseMovesWindowToEndOfOrderAndRoot() {
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        InventoryModel model = modelWith("pack_1", "pack_2");

        PackContainerWindow a = mgr.open("pack_1", model, 8, 8);
        PackContainerWindow b = mgr.open("pack_2", model, 24, 24);
        assertSame(b, mgr.ordered().get(1), "前置：pack_2 当前在最上");

        mgr.raise(a); // 点击置顶 pack_1
        assertSame(a, mgr.ordered().get(1), "raise 后 pack_1 应移到 map 末尾（z 最上）");
        assertSame(a, root.children().get(root.children().size() - 1),
            "raise 后 pack_1 窗口应重挂到 root 末尾（视觉最上）");
    }

    @Test
    void closeDisposesRemovesFromMapAndDetachesFromRoot() {
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        InventoryModel model = modelWith("pack_1", "pack_2");

        PackContainerWindow a = mgr.open("pack_1", model, 8, 8);
        mgr.open("pack_2", model, 24, 24);
        assertEquals(2, mgr.size());

        mgr.close("pack_1");
        assertEquals(1, mgr.size(), "close 后 map 应少一窗");
        assertFalse(mgr.contains("pack_1"), "close 后 map 不再含该 id");
        assertTrue(a.isClosed(), "close 应 dispose 该窗（内含面板 listener 解绑）");
        assertFalse(root.children().contains(a), "close 应从 root 摘除该窗");
    }

    @Test
    void closeUnknownIdIsNoOp() {
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        mgr.open("pack_1", modelWith("pack_1"), 8, 8);

        mgr.close("pack_999"); // 不存在
        assertEquals(1, mgr.size(), "close 不存在的 id 不应影响已开窗口");
        assertTrue(mgr.contains("pack_1"));
    }

    @Test
    void closeAllDisposesEveryWindowAndClears() {
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        InventoryModel model = modelWith("pack_1", "pack_2", "pack_3");

        PackContainerWindow a = mgr.open("pack_1", model, 0, 0);
        PackContainerWindow b = mgr.open("pack_2", model, 0, 0);
        PackContainerWindow c = mgr.open("pack_3", model, 0, 0);

        mgr.closeAll();
        assertEquals(0, mgr.size(), "closeAll 后 map 应清空");
        assertTrue(a.isClosed() && b.isClosed() && c.isClosed(),
            "closeAll 应 dispose 每个窗口（防 listener 泄漏）");
        for (Component child : root.children()) {
            assertFalse(child == a || child == b || child == c,
                "closeAll 应把所有窗口从 root 摘除");
        }
    }

    @Test
    void disposedWindowNoLongerReceivesSnapshotRefresh() {
        // 端到端锁住 listener 解绑：closeAll dispose 后再推快照，grid 不应再被回刷清空。
        FlowLayout root = newRoot();
        PackWindowManager mgr = new PackWindowManager();
        mgr.attach(root);
        InventoryModel model = InventoryModel.builder()
            .containers(List.of(new InventoryModel.ContainerDef("pack_1", "破草包", 3, 3, 1000L)))
            .gridItem(com.bong.client.inventory.model.InventoryItem.createFull(
                2001L, "spirit_herb", "spirit_herb", 1, 1, 0.5, "common", "", 1, 1.0, 1.0),
                "pack_1", 0, 0)
            .build();

        PackContainerWindow win = mgr.open("pack_1", model, 0, 0);
        assertTrue(win.grid().itemAt(0, 0) != null, "前置：build 后 grid 有物品");

        mgr.closeAll();
        // 推一个空 pack 快照：若 listener 仍在会清空 grid；解绑后 grid 保留旧内容。
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(List.of(new InventoryModel.ContainerDef("pack_1", "破草包", 3, 3, 1000L)))
            .build());
        assertTrue(win.grid().itemAt(0, 0) != null,
            "dispose 后不应再收快照回调（grid 仍保留 dispose 前内容）");
    }

    @Test
    void openWithoutAttachedRootDoesNotThrow() {
        // root 未 attach（理论上不该发生）时不应 NPE，仅不挂载。
        PackWindowManager mgr = new PackWindowManager();
        PackContainerWindow win = mgr.open("pack_1", modelWith("pack_1"), 8, 8);
        assertEquals(1, mgr.size());
        assertFalse(win.isClosed());
        mgr.closeAll(); // 同样不应 NPE
        assertEquals(0, mgr.size());
    }
}
