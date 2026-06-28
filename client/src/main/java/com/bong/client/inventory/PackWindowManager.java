package com.bong.client.inventory;

import com.bong.client.inventory.component.PackContainerWindow;
import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Positioning;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * plan-tarkov-floating-windows —— 套包悬浮窗口的多开 / z-order / 关闭管理器。
 *
 * <p>持有 {@code LinkedHashMap<containerId, PackContainerWindow>}：插入顺序即 root 渲染顺序，
 * <b>末尾 = 视觉最上</b>。{@link #open} 去重（已开则置顶），{@link #raise} 点击置顶，
 * {@link #close} / {@link #closeAll} dispose 内含面板（解绑 listener）后从 root removeChild。</p>
 *
 * <p>从 InspectScreen 抽出独立类，使多窗排序 / 去重 / dispose 契约可在 headless 单测下用一个
 * 测试构造的 owo {@link FlowLayout} root 直接覆盖（InspectScreen 自身的 mouseClicked hit-test
 * 需要真布局，仍由真机兜底）。</p>
 */
public final class PackWindowManager {

    private final LinkedHashMap<String, PackContainerWindow> windows = new LinkedHashMap<>();
    private FlowLayout root;

    /** 绑定挂载根（InspectScreen 的 owo root FlowLayout）。窗口以 absolute 定位脱离 flow，不扰动兄弟节点。 */
    public void attach(FlowLayout root) {
        this.root = root;
    }

    /**
     * 打开（或置顶已开）某容器的悬浮窗。
     *
     * <p>已存在同 id 且未关闭 → {@link #raise}（去重，不重复 new）；否则新建 + build + absolute 定位 +
     * root.child + 入 map。</p>
     *
     * @param anchorX/anchorY 仅首次打开时的初始堆叠坐标（错开叠放）。
     * @return 该容器对应的窗口（新建或既有）。
     */
    public PackContainerWindow open(String containerId, InventoryModel model, int anchorX, int anchorY) {
        PackContainerWindow existing = windows.get(containerId);
        if (existing != null && !existing.isClosed()) {
            raise(existing);
            return existing;
        }
        if (existing != null) {
            // 残留已关闭条目：先彻底清理（防 id 占位）。
            close(containerId);
        }
        WornContainerPanel inner = new WornContainerPanel(containerId, model);
        inner.suppressTitle(); // 标题由窗口标题栏画，避免与内含面板自带 label 重复。
        inner.build();
        PackContainerWindow win = new PackContainerWindow(inner);
        win.positioning(Positioning.absolute(anchorX, anchorY));
        if (root != null) {
            root.child(win);
        }
        windows.put(containerId, win);
        return win;
    }

    /** 点击置顶：bringToFront + 在 map 中移到末尾（保持 map 顺序 == 渲染顺序）。 */
    public void raise(PackContainerWindow win) {
        if (win == null) {
            return;
        }
        win.bringToFront(root);
        windows.remove(win.containerId());
        windows.put(win.containerId(), win);
    }

    /** 关闭单窗：dispose（解绑 listener）→ root.removeChild → 出 map。 */
    public void close(String containerId) {
        PackContainerWindow win = windows.remove(containerId);
        if (win == null) {
            return;
        }
        win.dispose();
        if (root != null) {
            root.removeChild(win);
        }
    }

    /** 全关（Screen removed 时）：全部 dispose 后从 root 摘除并清空，防 listener 泄漏。 */
    public void closeAll() {
        for (PackContainerWindow win : windows.values()) {
            win.dispose();
            if (root != null) {
                root.removeChild(win);
            }
        }
        windows.clear();
    }

    public boolean contains(String containerId) {
        return windows.containsKey(containerId);
    }

    public int size() {
        return windows.size();
    }

    public PackContainerWindow get(String containerId) {
        return windows.get(containerId);
    }

    /** 当前窗口按 z-order（首=最底，末=最上）快照；调用方可逆序做 topmost-first 命中。 */
    public List<PackContainerWindow> ordered() {
        return new ArrayList<>(windows.values());
    }

    /** map 条目快照（reconcile 用）。 */
    public List<Map.Entry<String, PackContainerWindow>> entries() {
        return new ArrayList<>(windows.entrySet());
    }
}
