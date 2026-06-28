package com.bong.client.inventory.component;

import com.bong.client.inventory.WornContainerPanel;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Positioning;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-floating-windows —— PackContainerWindow 拖动移位 / 置顶 / 委托 的纯逻辑回归。
 *
 * <p>锁住「机制真活非死代码」的核心：{@code onMouseDrag} 真实累加 offset；{@code bringToFront} 坐标守恒
 * （bake offset → 绝对坐标 + offset 归零）并重挂到 root 末尾（视觉最上）。owo 真渲染 / focusHandler 路由
 * （forehead childAt hit-test）依赖真布局，headless 测不到，由 {@code ./gradlew runClient} 真机兜底
 * （见 plan 留后续）。</p>
 */
public class PackContainerWindowTest {

    private static final String PACK_ID = "pack_1007";

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    private static InventoryModel modelWithPack(String id) {
        return InventoryModel.builder()
            .containers(List.of(new InventoryModel.ContainerDef(id, "破草包", 3, 3, 1007L)))
            .build();
    }

    private static PackContainerWindow window(String id) {
        WornContainerPanel inner = new WornContainerPanel(id, modelWithPack(id));
        inner.suppressTitle();
        inner.build();
        return new PackContainerWindow(inner);
    }

    @Test
    void wrappingUnbuiltPanelThrows() {
        // 未 build 的内含面板没有 content FlowLayout → 包裹必须显式失败（防 NPE 静默）。
        WornContainerPanel notBuilt = new WornContainerPanel(PACK_ID, modelWithPack(PACK_ID));
        assertThrows(IllegalStateException.class, () -> new PackContainerWindow(notBuilt),
            "WornContainerPanel.build() 未调用时包裹应抛 IllegalStateException");
    }

    @Test
    void delegatesContainerIdGridAndClosedState() {
        PackContainerWindow win = window(PACK_ID);
        assertEquals(PACK_ID, win.containerId(), "containerId 透传内含面板");
        assertNotNull(win.grid(), "grid 透传内含面板（build 后非 null）");
        assertEquals(PACK_ID, win.grid().containerId());
        assertFalse(win.isClosed(), "未 dispose 时 isClosed=false");

        win.dispose();
        assertTrue(win.isClosed(), "dispose 后 isClosed=true（内含面板 listener 已解绑）");
    }

    @Test
    void onMouseDragAccumulatesOffset() {
        PackContainerWindow win = window(PACK_ID);
        assertEquals(0.0, win.xOffsetForTest(), "初始 xOffset=0");
        assertEquals(0.0, win.yOffsetForTest(), "初始 yOffset=0");

        win.onMouseDrag(0, 0, 5, 7, 0);
        assertEquals(5.0, win.xOffsetForTest(), "拖动 dx=5 后 xOffset=5（真实移位，非死代码）");
        assertEquals(7.0, win.yOffsetForTest(), "拖动 dy=7 后 yOffset=7");

        win.onMouseDrag(0, 0, 3, -2, 0);
        assertEquals(8.0, win.xOffsetForTest(), "再拖 dx=3 → 累加 xOffset=8");
        assertEquals(5.0, win.yOffsetForTest(), "再拖 dy=-2 → 累加 yOffset=5");
    }

    @Test
    void bringToFrontBakesOffsetIntoAbsoluteAndResets() {
        FlowLayout root = Containers.verticalFlow(Sizing.content(), Sizing.content());
        PackContainerWindow win = window(PACK_ID);
        root.child(win);

        win.onMouseDrag(0, 0, 8, 5, 0); // baseX/baseY 仍 0（无布局），offset=(8,5)
        win.bringToFront(root);

        assertEquals(0.0, win.xOffsetForTest(), "bringToFront 后 xOffset 归零");
        assertEquals(0.0, win.yOffsetForTest(), "bringToFront 后 yOffset 归零");
        Positioning pos = win.positioning().get();
        assertEquals(Positioning.Type.ABSOLUTE, pos.type, "应改为 absolute 定位");
        assertEquals(8, pos.x, "absolute.x = baseX(0) + 累加 xOffset(8)，坐标守恒不跳位");
        assertEquals(5, pos.y, "absolute.y = baseY(0) + 累加 yOffset(5)，坐标守恒不跳位");
    }

    @Test
    void bringToFrontReattachesAtEndOfRootForTopZOrder() {
        FlowLayout root = Containers.verticalFlow(Sizing.content(), Sizing.content());
        PackContainerWindow a = window("pack_1");
        PackContainerWindow b = window("pack_2");
        root.child(a);
        root.child(b); // 末尾=最上：当前 b 在上

        a.bringToFront(root); // 把 a 提到末尾（最上）
        List<? extends io.wispforest.owo.ui.core.Component> kids = root.children();
        assertEquals(2, kids.size(), "重挂不应改变窗口数量");
        assertSame(a, kids.get(kids.size() - 1), "bringToFront 后该窗口应在 root 末尾（视觉最上）");
        assertSame(b, kids.get(0), "另一窗口退到前面（视觉更下）");
    }

    @Test
    void isOverCloseButtonFalseWhenUnlaidOut() {
        // 未布局（width==0）时 hit-test 恒 false，防 headless / 首帧误判命中关闭。
        PackContainerWindow win = window(PACK_ID);
        assertFalse(win.isOverCloseButton(0, 0), "width==0（未布局）时 ✕ 命中应恒 false");
        assertFalse(win.isOverCloseButton(1000, 1000), "任意坐标在未布局时都不应命中 ✕");
    }
}
