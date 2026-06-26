package com.bong.client.inventory;

import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
import com.bong.client.inventory.state.InventoryStateStore;
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

import java.util.function.Consumer;

/**
 * plan-tarkov-backpack-v1 P3（交付物 #1/#3/#5）—— 穿戴背包件的内含物视图组件。
 *
 * <p>双击装备槽内穿戴的背包件后，挂入 InspectScreen 的 {@code outerRow}，渲染该 {@code pack_<id>}
 * 容器的内含物 grid，支持拖入拖出。</p>
 *
 * <p><b>布局结构复用 {@link LootContainerPanel}</b>（owo verticalFlow + {@link BackpackGridPanel}
 * + dispose listener 解绑模式），但<b>数据源是全局 {@link InventoryStateStore}</b>（非 loot 专属
 * store）：背包件容器已在主 InventorySnapshot 里（穿装即建容器、随 snapshot 下发），双击直接渲染本地
 * 状态，无 C2S round-trip。</p>
 *
 * <p><b>发包路线硬约束（交付物 #1）</b>：本面板的拖入拖出由 InspectScreen 走
 * {@code ClientRequestSender.sendInventoryMove}（编码 {@code InventoryMoveIntent}）。
 * <b>严禁</b>走 {@code sendExternalContainerMove}（loot 专用，需 session_id，协议层不同）。本面板
 * 自身不发包，仅暴露 grid 供 InspectScreen 派发 move intent。</p>
 *
 * <p><b>snapshot 订阅（交付物 #3，禁止 stub）</b>：{@link #build()} 时向 InventoryStateStore
 * 注册 {@code Consumer<InventoryModel>} listener，server resync 后刷新 grid + 包重；
 * {@link #dispose()} 解绑，保证显示态不与实际态发散。listener 在网络线程触发，UI mutation 回主线程。</p>
 */
public final class WornContainerPanel {
    private static final int BG_COLOR = 0xFF101018;
    private static final int BORDER_COLOR = 0xFF4A4050;

    /** 该容器 id（{@code pack_<instance_id>}），驱动 grid populate 的过滤键。 */
    private final String containerId;

    private BackpackGridPanel grid;
    private LabelComponent titleLabel;
    private LabelComponent weightLabel;
    private FlowLayout container; // root FlowLayout of this panel
    private Consumer<InventoryModel> storeListener;
    private boolean closed;

    /** 最近一次用于 populate 的 model（构造期 + listener 回刷），供包重 / 标题派生。 */
    private InventoryModel model;

    public WornContainerPanel(String containerId, InventoryModel initialModel) {
        if (containerId == null || containerId.isBlank()) {
            throw new IllegalArgumentException("containerId must not be blank");
        }
        this.containerId = containerId;
        this.model = initialModel == null ? InventoryModel.empty() : initialModel;
    }

    /**
     * 构建面板 UI，返回根 FlowLayout 供 InspectScreen 挂入 outerRow。
     *
     * <p>grid 尺寸取自 model 中该容器的 {@link InventoryModel.ContainerDef}（rows×cols）；
     * 找不到（容器已不在 snapshot）则退化为 1×1，listener 回刷时仍会按新 model 重建数据。</p>
     */
    public FlowLayout build() {
        container = Containers.verticalFlow(Sizing.content(), Sizing.content());
        container.surface(Surface.flat(BG_COLOR).and(Surface.outline(BORDER_COLOR)));
        container.padding(Insets.of(8));
        container.gap(4);

        // 标题 / 包重 label 依赖 textRenderer（运行时 owo 渲染需要）。headless 单测无 MinecraftClient
        // 时跳过 label 构建——grid + listener 订阅才是行为契约，label 纯显示。
        boolean hasClient = MinecraftClient.getInstance() != null;
        if (hasClient) {
            titleLabel = Components.label(Text.literal(""))
                .horizontalTextAlignment(HorizontalAlignment.CENTER);
            container.child(titleLabel);
        }

        InventoryModel.ContainerDef def = findContainerDef(model);
        int rows = def != null ? def.rows() : 1;
        int cols = def != null ? def.cols() : 1;
        grid = new BackpackGridPanel(containerId, rows, cols);
        container.child(grid.container());

        if (hasClient) {
            weightLabel = Components.label(Text.literal(""));
            weightLabel.color(Color.ofArgb(0xFFAAAAAA));
            container.child(weightLabel);
        }

        // 数据源 = 全局 InventoryStateStore（非 loot store）。server resync 后回刷 grid + 包重。
        // listener 在网络线程触发，UI mutation 必须回主线程（与 InspectScreen.inventoryListener 一致）。
        // headless 测试无 MinecraftClient → 直接同步刷新（execute 不可用）。
        storeListener = next -> {
            if (next == null) return;
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc != null) {
                mc.execute(() -> refresh(next));
            } else {
                refresh(next);
            }
        };
        InventoryStateStore.addListener(storeListener);

        refresh(model);
        return container;
    }

    /** 用最新 model 刷新 grid + 标题 + 包重显示。listener 与构造期共用。 */
    private void refresh(InventoryModel next) {
        this.model = next == null ? InventoryModel.empty() : next;
        if (grid != null) {
            grid.populateFromModel(this.model);
        }
        updateTitle();
        updateWeight();
    }

    private void updateTitle() {
        if (titleLabel == null) return;
        InventoryModel.ContainerDef def = findContainerDef(model);
        String name = def != null ? def.name() : "背包件";
        titleLabel.text(Text.literal("§f" + name + " §r— 容器物品"));
    }

    /**
     * 包重显示（决议 #3）：包重 = 背包件自重 + 该容器内含物自重累加（client 本地从 snapshot 累加）。
     *
     * <p>区别于 InspectScreen 状态条的「总重 = InventoryWeightV1.current」（全层 flat 求和）；
     * 此处只算这一个 pack 的局部重量，供玩家直观判断单包负载。</p>
     */
    private void updateWeight() {
        if (weightLabel == null) return;
        double packWeight = packSelfWeight() + containedItemsWeight();
        weightLabel.text(Text.literal(String.format("包重 %.1f", packWeight)));
    }

    /** 背包件自重 = snapshot 中 owner instance 对应的穿戴 worn 件 weight；找不到记 0。 */
    private double packSelfWeight() {
        java.util.OptionalLong ownerOpt = ownerInstanceId();
        if (ownerOpt.isEmpty() || model == null) {
            return 0.0;
        }
        long owner = ownerOpt.getAsLong();
        for (SlotContents contents : model.equippedSlots().values()) {
            if (contents == null) continue;
            for (InventoryItem worn : contents.worn()) {
                if (worn != null && worn.instanceId() == owner) {
                    return worn.weight();
                }
            }
        }
        return 0.0;
    }

    /** 该容器内含物自重累加（weight × stackCount，按 snapshot placed_items 过滤本容器）。 */
    private double containedItemsWeight() {
        if (model == null) return 0.0;
        double sum = 0.0;
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            if (!containerId.equals(entry.containerId())) continue;
            InventoryItem item = entry.item();
            if (item == null) continue;
            sum += item.weight() * Math.max(1, item.stackCount());
        }
        return sum;
    }

    /**
     * owner instance_id：优先读 ContainerDef.ownerInstanceId（P3 server 下发字段），
     * 缺省（旧 server / 静态容器）回退到 {@code pack_<id>} 前缀反解。
     */
    private java.util.OptionalLong ownerInstanceId() {
        InventoryModel.ContainerDef def = findContainerDef(model);
        if (def != null && def.ownerInstanceId() != null) {
            return java.util.OptionalLong.of(def.ownerInstanceId());
        }
        return InspectScreen.parseWornPackInstance(containerId);
    }

    private InventoryModel.ContainerDef findContainerDef(InventoryModel m) {
        if (m == null) return null;
        for (InventoryModel.ContainerDef def : m.containers()) {
            if (containerId.equals(def.id())) {
                return def;
            }
        }
        return null;
    }

    /** 根 FlowLayout（build() 后可用）。 */
    public FlowLayout container() {
        return container;
    }

    /** 内含物 grid，供 InspectScreen 拖拽落位 / 拾取处理。 */
    public BackpackGridPanel grid() {
        return grid;
    }

    /** 该容器 id（{@code pack_<id>}），InspectScreen 派发 move intent 的 to/from containerId。 */
    public String containerId() {
        return containerId;
    }

    /** 是否已 dispose。 */
    public boolean isClosed() {
        return closed;
    }

    /**
     * 解绑 InventoryStateStore 订阅。unmount / removed() 时必须调用，避免后续 snapshot 到达仍回调
     * 已销毁组件（显示态发散 / 内存泄漏）。
     */
    public void dispose() {
        if (storeListener != null) {
            InventoryStateStore.removeListener(storeListener);
            storeListener = null;
        }
        closed = true;
    }

    // 包重显示宽度对齐 grid（cols × CELL_SIZE），保持视觉一致；预留给后续布局微调。
    int gridPixelWidth() {
        return grid == null ? 0 : grid.cols() * GridSlotComponent.CELL_SIZE;
    }
}
