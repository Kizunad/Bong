package com.bong.client.inventory;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.inventory.component.*;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.DragState;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.inventory.state.PhysicalBodyStore;
import net.minecraft.client.MinecraftClient;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.*;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.function.Consumer;


public class InspectScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("检视");
    private static final int ICON_SIZE = 128;
    private static final int HOTBAR_SLOTS = 9;

    private static final int TAB_ACTIVE_COLOR = 0xFFCCCCCC;
    private static final int TAB_INACTIVE_COLOR = 0xFF555555;

    private InventoryModel model;
    private final DragState dragState = new DragState();
    /** Screen 存活期间持有的 InventoryStateStore 订阅，close 时解绑避免泄漏。 */
    private Consumer<InventoryModel> inventoryListener;

    // --- Container grids (driven by model.containers()) ---
    private BackpackGridPanel[] containerGrids;
    private FlowLayout[] containerWrappers;
    private LabelComponent[] containerLabels;
    private int containerCount;
    private int activeContainer = 0;

    private EquipmentPanel equipPanel;
    private StatusBarsPanel statusBars;
    private ItemTooltipPanel tooltipPanel;
    private BottomInfoBar bottomBar;

    // Tabs (left panel)
    private int activeTab = 0;
    private final LabelComponent[] tabLabels = new LabelComponent[2];
    private FlowLayout equipTabContent;
    private FlowLayout cultivationTabContent;

    // Hotbar
    private final GridSlotComponent[] hotbarSlots = new GridSlotComponent[HOTBAR_SLOTS];
    private final InventoryItem[] hotbarItems = new InventoryItem[HOTBAR_SLOTS];
    private FlowLayout hotbarStrip;

    // Quick-use bar (F1-F9, plan-HUD-v1 §2.2 上层)
    private final GridSlotComponent[] quickUseSlots = new GridSlotComponent[HOTBAR_SLOTS];
    private final InventoryItem[] quickUseItems = new InventoryItem[HOTBAR_SLOTS];
    private FlowLayout quickUseStrip;

    // Discard
    private FlowLayout discardStrip;

    // Body inspect (cultivation tab) — dual-layer: physical + meridian
    private BodyInspectComponent bodyInspect;
    private LabelComponent physicalLayerLabel;
    private LabelComponent meridianLayerLabel;
    private FlowLayout meridianFilterBar;
    private io.wispforest.owo.ui.container.ScrollContainer<?> cultivationActionScroll;
    /** Screen 存活期间持有的 MeridianStateStore 订阅，close 时移除避免泄漏。 */
    private Consumer<MeridianBody> meridianBodyListener;
    private final LabelComponent[] filterLabels = new LabelComponent[4];

    public InspectScreen(InventoryModel model) {
        super(TITLE);
        this.model = model == null ? InventoryModel.empty() : model;
    }

    @Override
    public void removed() {
        // Screen 被关闭时解绑全局 store 订阅，防止后续快照到达仍回调已销毁组件。
        if (meridianBodyListener != null) {
            MeridianStateStore.removeListener(meridianBodyListener);
            meridianBodyListener = null;
        }
        if (inventoryListener != null) {
            InventoryStateStore.removeListener(inventoryListener);
            inventoryListener = null;
        }
        super.removed();
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        // Outermost: [hotbar] [main] [discard]
        FlowLayout outerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        outerRow.gap(2);
        outerRow.verticalAlignment(VerticalAlignment.CENTER);

        // === FAR LEFT: Hotbar (1-9 战斗栏) + Quick-use (F1-F9 快捷使用栏) ===
        hotbarStrip = buildHotbarStrip();
        outerRow.child(hotbarStrip);
        quickUseStrip = buildQuickUseStrip();
        outerRow.child(quickUseStrip);
        hydrateQuickUseFromStore();

        // === CENTER: Main panel ===
        FlowLayout mainPanel = Containers.verticalFlow(Sizing.content(), Sizing.content());
        mainPanel.surface(Surface.flat(0xFF1A1A1A));
        mainPanel.padding(Insets.of(4));
        mainPanel.gap(2);

        FlowLayout middle = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        middle.gap(4);

        // -- Left column --
        // 宽 172 = 经脉层 body (168) + 4 内边距。装备层内容固定更小，在此列内左对齐。
        FlowLayout leftCol = Containers.verticalFlow(Sizing.fixed(172), Sizing.content());
        leftCol.gap(2);

        // Tab bar
        FlowLayout tabBar = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        tabBar.gap(6);
        tabBar.padding(Insets.of(1, 2, 1, 2));
        String[] tabNames = {"装备", "修仙"};
        for (int i = 0; i < 2; i++) {
            final int idx = i;
            var label = Components.label(Text.literal(tabNames[i]));
            label.color(Color.ofArgb(i == 0 ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            label.cursorStyle(CursorStyle.HAND);
            label.mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) { switchTab(idx); return true; }
                return false;
            });
            tabLabels[i] = label;
            tabBar.child(label);
        }
        leftCol.child(tabBar);

        // Tab 0: Equipment + Status
        equipTabContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        equipTabContent.gap(2);
        equipPanel = new EquipmentPanel();
        equipTabContent.child(equipPanel.container());
        statusBars = new StatusBarsPanel();
        equipTabContent.child(statusBars);
        leftCol.child(equipTabContent);

        // Tab 1: Cultivation (body inspect — dual layer)
        cultivationTabContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        cultivationTabContent.gap(2);
        cultivationTabContent.horizontalAlignment(HorizontalAlignment.CENTER);

        // Layer toggle: [体表] [经脉]
        FlowLayout layerBar = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        layerBar.gap(6);
        layerBar.padding(Insets.of(1, 2, 1, 2));
        physicalLayerLabel = Components.label(Text.literal("体表"));
        physicalLayerLabel.color(Color.ofArgb(TAB_ACTIVE_COLOR));
        physicalLayerLabel.cursorStyle(CursorStyle.HAND);
        physicalLayerLabel.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { switchBodyLayer(BodyInspectComponent.Layer.PHYSICAL); return true; }
            return false;
        });
        meridianLayerLabel = Components.label(Text.literal("经脉"));
        meridianLayerLabel.color(Color.ofArgb(TAB_INACTIVE_COLOR));
        meridianLayerLabel.cursorStyle(CursorStyle.HAND);
        meridianLayerLabel.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { switchBodyLayer(BodyInspectComponent.Layer.MERIDIAN); return true; }
            return false;
        });
        layerBar.child(physicalLayerLabel);
        layerBar.child(meridianLayerLabel);
        cultivationTabContent.child(layerBar);

        // Meridian filter bar: [全部] [手经] [足经] [奇经] — 仅经脉层显示
        meridianFilterBar = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        meridianFilterBar.gap(6);
        meridianFilterBar.padding(Insets.of(1, 2, 1, 2));
        BodyInspectComponent.MeridianFilter[] filters = BodyInspectComponent.MeridianFilter.values();
        for (int i = 0; i < filters.length; i++) {
            final int idx = i;
            var lbl = Components.label(Text.literal(filters[i].label()));
            lbl.color(Color.ofArgb(i == 0 ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            lbl.cursorStyle(CursorStyle.HAND);
            lbl.mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) { switchMeridianFilter(filters[idx]); return true; }
                return false;
            });
            filterLabels[i] = lbl;
            meridianFilterBar.child(lbl);
        }
        cultivationTabContent.child(meridianFilterBar);
        meridianFilterBar.positioning(Positioning.absolute(-9999, -9999));

        // Body 必须在 action bar 之前创建，action bar 需要引用 selectedChannel
        bodyInspect = new BodyInspectComponent();
        PhysicalBody physData = PhysicalBodyStore.snapshot();
        bodyInspect.setPhysicalBody(physData != null ? physData : MockPhysicalData.create());
        MeridianBody meridianData = MeridianStateStore.snapshot();
        bodyInspect.setMeridianBody(meridianData != null ? meridianData : MockMeridianData.create());

        // Cultivation action bar: [设为目标] [突破] [淬炼·流速] [淬炼·容量]
        // 「突破」常开；其余三项需选中某条经脉 — 灰态表示禁用。
        FlowLayout actionBar = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        actionBar.gap(4);
        actionBar.padding(Insets.of(2, 2, 2, 2));
        actionBar.verticalAlignment(VerticalAlignment.CENTER);
        Object[] setTargetBtn = buildActionButton("设为目标", () -> {
            var sel = bodyInspect.selectedChannel();
            if (sel != null) {
                com.bong.client.network.ClientRequestSender.sendSetMeridianTarget(
                    com.bong.client.network.ClientRequestProtocol.toMeridianId(sel));
            }
        });
        Object[] breakthroughBtn = buildActionButton("突破",
            com.bong.client.network.ClientRequestSender::sendBreakthroughRequest);
        Object[] forgeRateBtn = buildActionButton("淬炼·流速", () -> {
            var sel = bodyInspect.selectedChannel();
            if (sel != null) {
                com.bong.client.network.ClientRequestSender.sendForgeRequest(
                    com.bong.client.network.ClientRequestProtocol.toMeridianId(sel),
                    com.bong.client.network.ClientRequestProtocol.ForgeAxis.Rate);
            }
        });
        Object[] forgeCapBtn = buildActionButton("淬炼·容量", () -> {
            var sel = bodyInspect.selectedChannel();
            if (sel != null) {
                com.bong.client.network.ClientRequestSender.sendForgeRequest(
                    com.bong.client.network.ClientRequestProtocol.toMeridianId(sel),
                    com.bong.client.network.ClientRequestProtocol.ForgeAxis.Capacity);
            }
        });
        var setTargetLabel = (LabelComponent) setTargetBtn[1];
        var forgeRateLabel = (LabelComponent) forgeRateBtn[1];
        var forgeCapLabel = (LabelComponent) forgeCapBtn[1];
        actionBar.child((io.wispforest.owo.ui.core.Component) setTargetBtn[0]);
        actionBar.child((io.wispforest.owo.ui.core.Component) breakthroughBtn[0]);
        actionBar.child((io.wispforest.owo.ui.core.Component) forgeRateBtn[0]);
        actionBar.child((io.wispforest.owo.ui.core.Component) forgeCapBtn[0]);
        // 横向可滚动容器：塞不下时可拖滚动条或滚轮横向浏览
        var actionScroll = Containers.horizontalScroll(Sizing.fill(100), Sizing.content(), actionBar);
        actionScroll.scrollbarThiccness(3);
        cultivationActionScroll = actionScroll;
        cultivationTabContent.child(actionScroll);
        // 初始 layer = PHYSICAL，按钮组应与 meridianFilterBar 一样初始隐藏；
        // 否则首次切到 心·身·境 tab 时按钮短暂可见，直到用户切一次经脉层 switchBodyLayer 才触发 hide。
        actionScroll.positioning(Positioning.absolute(-9999, -9999));

        // 状态条：境界 · 污染总量（数据来源 cultivation_detail S2C）
        LabelComponent bodyStatusLabel = Components.label(Text.literal(""));
        bodyStatusLabel.color(Color.ofArgb(0xFFAAAAAA));
        cultivationTabContent.child(bodyStatusLabel);
        Runnable refreshBodyStatus = () -> {
            MeridianBody b = bodyInspect.meridianBody();
            if (b == null) { bodyStatusLabel.text(Text.literal("")); return; }
            StringBuilder sb = new StringBuilder();
            if (b.realm() != null && !b.realm().isEmpty()) {
                sb.append("§7境界 §f").append(b.realm());
            }
            if (b.contaminationTotal() > 0.0) {
                if (sb.length() > 0) sb.append("  §8·  ");
                sb.append(String.format("§d污染 §f%.1f", b.contaminationTotal()));
            }
            bodyStatusLabel.text(Text.literal(sb.toString()));
        };
        refreshBodyStatus.run();
        // 网络新快照到达时推到 UI（BodyInspect 内部不会自动感知 MeridianStateStore.replace）。
        // 存成字段以便 removed() 回调里解绑 —— 否则每开一次 InspectScreen 都累积一个悬挂监听。
        meridianBodyListener = body -> {
            if (body != null) bodyInspect.setMeridianBody(body);
            refreshBodyStatus.run();
        };
        MeridianStateStore.addListener(meridianBodyListener);
        bodyInspect.addSelectionListener(ch -> refreshBodyStatus.run());

        // 根据当前选择应用灰态，并订阅变化
        Runnable refreshActionColors = () -> {
            boolean hasSel = bodyInspect.selectedChannel() != null;
            int c = hasSel ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR;
            setTargetLabel.color(Color.ofArgb(c));
            forgeRateLabel.color(Color.ofArgb(c));
            forgeCapLabel.color(Color.ofArgb(c));
        };
        refreshActionColors.run();
        bodyInspect.addSelectionListener(ch -> refreshActionColors.run());

        cultivationTabContent.child(bodyInspect);

        leftCol.child(cultivationTabContent);
        cultivationTabContent.positioning(Positioning.absolute(-9999, -9999));

        middle.child(leftCol);

        // 经脉详情直接绘制在 body 画布内部（见 BodyInspectComponent.drawMeridianDetailInline）
        // 不再作为独立组件，以免增加列宽/列高

        // -- Right column --
        FlowLayout rightCol = Containers.verticalFlow(Sizing.content(), Sizing.content());
        rightCol.gap(2);

        // Container tabs (driven by model)
        var containerDefs = model.containers();
        containerCount = containerDefs.size();
        containerGrids = new BackpackGridPanel[containerCount];
        containerWrappers = new FlowLayout[containerCount];
        containerLabels = new LabelComponent[containerCount];

        FlowLayout containerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        containerRow.gap(2);
        int maxCols = 0;
        for (var def : containerDefs) maxCols = Math.max(maxCols, def.cols());

        for (int i = 0; i < containerCount; i++) {
            final int ci = i;
            var def = containerDefs.get(i);

            FlowLayout tab = Containers.horizontalFlow(Sizing.content(), Sizing.fixed(14));
            tab.surface(Surface.flat(i == 0 ? 0xFF282828 : 0xFF1E1E1E));
            tab.padding(Insets.of(1, 4, 1, 4));
            tab.verticalAlignment(VerticalAlignment.CENTER);
            tab.cursorStyle(CursorStyle.HAND);

            var label = Components.label(Text.literal(
                (i == 0 ? "§f" : "§7") + def.name()
                + " §8(" + def.rows() + "×" + def.cols() + ")"
            ));
            containerLabels[i] = label;
            tab.child(label);
            tab.mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) { switchContainer(ci); return true; }
                return false;
            });
            containerRow.child(tab);
        }
        rightCol.child(containerRow);

        // Build all grids, show only active
        int wrapperW = maxCols * GridSlotComponent.CELL_SIZE + 4;
        for (int i = 0; i < containerCount; i++) {
            var def = containerDefs.get(i);
            containerGrids[i] = new BackpackGridPanel(def.id(), def.rows(), def.cols());
            FlowLayout w = Containers.verticalFlow(Sizing.fixed(wrapperW), Sizing.content());
            w.surface(Surface.flat(0xFF111111));
            w.padding(Insets.of(2));
            w.child(containerGrids[i].container());
            containerWrappers[i] = w;
            rightCol.child(w);
            if (i != 0) w.positioning(Positioning.absolute(-9999, -9999));
        }

        // Tooltip
        tooltipPanel = new ItemTooltipPanel();
        rightCol.child(tooltipPanel);

        middle.child(rightCol);
        mainPanel.child(middle);

        // Bottom bar
        bottomBar = new BottomInfoBar();
        mainPanel.child(bottomBar);

        outerRow.child(mainPanel);

        // === FAR RIGHT: Discard ===
        discardStrip = buildDiscardStrip();
        outerRow.child(discardStrip);

        root.child(outerRow);
        populateFromModel();

        // Server 增量到达时（InventoryEventHandler 写入或新 snapshot 落地）刷新 UI。
        // listener 在网络线程触发，UI mutation 必须回主线程。
        inventoryListener = next -> {
            if (next == null) return;
            MinecraftClient.getInstance().execute(() -> {
                this.model = next;
                populateFromModel();
            });
        };
        InventoryStateStore.addListener(inventoryListener);
    }

    // ==================== Build helpers ====================

    /** 返回 [wrapperFlow, innerLabel]；wrapper 用于添加到 actionBar，label 用于后续 .color() 调整。 */
    private Object[] buildActionButton(String text, Runnable onClick) {
        var lbl = Components.label(Text.literal(text));
        lbl.color(Color.ofArgb(TAB_ACTIVE_COLOR));
        lbl.cursorStyle(CursorStyle.HAND);
        lbl.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { onClick.run(); return true; }
            return false;
        });
        FlowLayout wrap = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        wrap.padding(Insets.of(3, 3, 6, 6));
        wrap.surface(Surface.flat(0xFF2A2A2A).and(Surface.outline(0xFF555555)));
        wrap.cursorStyle(CursorStyle.HAND);
        wrap.child(lbl);
        wrap.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { onClick.run(); return true; }
            return false;
        });
        return new Object[] { wrap, lbl };
    }

    private io.wispforest.owo.ui.component.LabelComponent buildActionLabel(String text, Runnable onClick) {
        // 兼容旧调用点：直接返回纯文字 label（未使用）
        var lbl = Components.label(Text.literal(text));
        lbl.color(Color.ofArgb(TAB_ACTIVE_COLOR));
        lbl.cursorStyle(CursorStyle.HAND);
        lbl.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { onClick.run(); return true; }
            return false;
        });
        return lbl;
    }

    private FlowLayout buildHotbarStrip() {
        int cs = GridSlotComponent.CELL_SIZE;
        FlowLayout strip = Containers.verticalFlow(Sizing.fixed(cs + 6), Sizing.content());
        strip.surface(Surface.flat(0xFF141414));
        strip.padding(Insets.of(3));
        strip.gap(1);
        strip.horizontalAlignment(HorizontalAlignment.CENTER);

        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            GridSlotComponent slot = new GridSlotComponent(i, 0);
            hotbarSlots[i] = slot;
            strip.child(slot);
        }
        return strip;
    }

    /** plan-HUD-v1 §2.2 上层：F1-F9 快捷使用栏（绿色背景 + 顶部 F 头标区分）。 */
    private FlowLayout buildQuickUseStrip() {
        int cs = GridSlotComponent.CELL_SIZE;
        FlowLayout strip = Containers.verticalFlow(Sizing.fixed(cs + 6), Sizing.content());
        strip.surface(Surface.flat(0xFF0F1A14));
        strip.padding(Insets.of(3));
        strip.gap(1);
        strip.horizontalAlignment(HorizontalAlignment.CENTER);

        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            GridSlotComponent slot = new GridSlotComponent(i, 0);
            quickUseSlots[i] = slot;
            strip.child(slot);
        }
        return strip;
    }

    private void hydrateQuickUseFromStore() {
        QuickSlotConfig config = QuickUseSlotStore.snapshot();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            QuickSlotEntry entry = config.slot(i);
            if (entry == null) {
                quickUseItems[i] = null;
                if (quickUseSlots[i] != null) quickUseSlots[i].clearItem();
                continue;
            }
            InventoryItem matched = findItemInModel(entry.itemId());
            quickUseItems[i] = matched;
            if (quickUseSlots[i] != null) {
                if (matched != null) quickUseSlots[i].setItem(matched, true);
                else quickUseSlots[i].clearItem();
            }
        }
    }

    private InventoryItem findItemInModel(String itemId) {
        if (itemId == null || itemId.isEmpty()) return null;
        for (InventoryItem h : model.hotbar()) {
            if (h != null && itemId.equals(h.itemId())) return h;
        }
        for (var entry : model.gridItems()) {
            InventoryItem it = entry.item();
            if (it != null && itemId.equals(it.itemId())) return it;
        }
        return null;
    }

    private void publishQuickUseSlot(int index, InventoryItem item) {
        QuickSlotConfig current = QuickUseSlotStore.snapshot();
        QuickSlotEntry entry = item == null ? null : new QuickSlotEntry(
            item.itemId(),
            item.displayName(),
            QUICK_USE_DEFAULT_CAST_MS,
            QUICK_USE_DEFAULT_COOLDOWN_MS,
            ""
        );
        QuickUseSlotStore.replace(current.withSlot(index, entry));
        com.bong.client.network.ClientRequestSender.sendQuickSlotBind(
            index, item == null ? null : item.itemId());
    }

    private static final int QUICK_USE_DEFAULT_CAST_MS = 1500;
    private static final int QUICK_USE_DEFAULT_COOLDOWN_MS = 500;

    private FlowLayout buildDiscardStrip() {
        int cs = GridSlotComponent.CELL_SIZE;
        FlowLayout strip = Containers.verticalFlow(Sizing.fixed(cs + 6), Sizing.content());
        strip.surface(Surface.flat(0xFF201010));
        strip.padding(Insets.of(3));
        strip.gap(2);
        strip.horizontalAlignment(HorizontalAlignment.CENTER);
        strip.verticalAlignment(VerticalAlignment.CENTER);
        strip.child(Components.label(Text.literal("§c丢")));
        strip.child(Components.label(Text.literal("§c弃")));
        return strip;
    }

    // ==================== Active grid shortcut ====================

    private BackpackGridPanel activeGrid() {
        return containerGrids[activeContainer];
    }

    // ==================== Tab / Container switching ====================

    private void switchTab(int idx) {
        if (idx == activeTab) return;
        activeTab = idx;
        for (int i = 0; i < 2; i++)
            tabLabels[i].color(Color.ofArgb(i == idx ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
        FlowLayout[] tabs = {equipTabContent, cultivationTabContent};
        for (int i = 0; i < 2; i++)
            tabs[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
    }

    private void switchBodyLayer(BodyInspectComponent.Layer layer) {
        if (bodyInspect == null) return;
        bodyInspect.setActiveLayer(layer);
        boolean isPhys = layer == BodyInspectComponent.Layer.PHYSICAL;
        physicalLayerLabel.color(Color.ofArgb(isPhys ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
        meridianLayerLabel.color(Color.ofArgb(isPhys ? TAB_INACTIVE_COLOR : TAB_ACTIVE_COLOR));
        if (meridianFilterBar != null) {
            meridianFilterBar.positioning(isPhys ? Positioning.absolute(-9999, -9999) : Positioning.layout());
        }
        if (cultivationActionScroll != null) {
            cultivationActionScroll.positioning(isPhys ? Positioning.absolute(-9999, -9999) : Positioning.layout());
        }
    }

    private void switchMeridianFilter(BodyInspectComponent.MeridianFilter filter) {
        if (bodyInspect == null) return;
        bodyInspect.setMeridianFilter(filter);
        BodyInspectComponent.MeridianFilter[] all = BodyInspectComponent.MeridianFilter.values();
        for (int i = 0; i < all.length; i++) {
            filterLabels[i].color(Color.ofArgb(all[i] == filter ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
        }
    }

    private void switchContainer(int idx) {
        if (idx == activeContainer || idx < 0 || idx >= containerCount) return;
        activeContainer = idx;
        var defs = model.containers();
        for (int i = 0; i < containerCount; i++) {
            containerWrappers[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
            var def = defs.get(i);
            containerLabels[i].text(Text.literal(
                (i == idx ? "§f" : "§7") + def.name()
                + " §8(" + def.rows() + "×" + def.cols() + ")"
            ));
        }
    }

    // ==================== Populate ====================

    private void populateFromModel() {
        populateContainerGrids(model, containerGrids);

        equipPanel.populateFromModel(model);
        statusBars.updateFromModel(model);
        bottomBar.updateFromModel(model);

        // Equipment state is managed solely by EquipmentPanel

        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            InventoryItem item = i < model.hotbar().size() ? model.hotbar().get(i) : null;
            hotbarItems[i] = item;
            if (hotbarSlots[i] != null) {
                if (item != null) hotbarSlots[i].setItem(item, true);
                else hotbarSlots[i].clearItem();
            }
        }

        hydrateQuickUseFromStore();
    }

    static void populateContainerGrids(InventoryModel model, BackpackGridPanel[] containerGrids) {
        if (containerGrids == null) {
            return;
        }

        for (BackpackGridPanel containerGrid : containerGrids) {
            if (containerGrid != null) {
                containerGrid.populateFromModel(model);
            }
        }
    }

    InventoryModel model() {
        return model;
    }

    static InventoryModel.ContainerDef containerDefAt(InventoryModel model, int index) {
        return model.containers().get(index);
    }

    static java.util.List<InventoryModel.GridEntry> gridEntriesForContainer(InventoryModel model, String containerId) {
        java.util.ArrayList<InventoryModel.GridEntry> entries = new java.util.ArrayList<>();
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            if (containerId.equals(entry.containerId())) {
                entries.add(entry);
            }
        }
        return java.util.List.copyOf(entries);
    }

    // ==================== Hit detection ====================

    private int hotbarSlotAtScreen(double sx, double sy) {
        int cs = GridSlotComponent.CELL_SIZE;
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            GridSlotComponent s = hotbarSlots[i];
            if (s != null && sx >= s.x() && sx < s.x() + cs && sy >= s.y() && sy < s.y() + cs)
                return i;
        }
        return -1;
    }

    private int quickUseSlotAtScreen(double sx, double sy) {
        int cs = GridSlotComponent.CELL_SIZE;
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            GridSlotComponent s = quickUseSlots[i];
            if (s != null && sx >= s.x() && sx < s.x() + cs && sy >= s.y() && sy < s.y() + cs)
                return i;
        }
        return -1;
    }

    private boolean isOverDiscard(double sx, double sy) {
        return sx >= discardStrip.x() && sx < discardStrip.x() + discardStrip.width()
            && sy >= discardStrip.y() && sy < discardStrip.y() + discardStrip.height();
    }

    // ==================== Mouse interaction ====================

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (button == 0) {
            boolean shift = hasShiftDown();
            BackpackGridPanel grid = activeGrid();

            // Grid
            if (grid.containsPoint(mouseX, mouseY)) {
                var pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = grid.itemAt(pos.row(), pos.col());
                    if (item != null) {
                        if (shift) quickEquipFromGrid(item);
                        else {
                            var anchor = grid.anchorOf(item);
                            if (anchor != null) {
                                grid.remove(item);
                                dragState.pickup(item, grid.containerId(), anchor.row(), anchor.col());
                            }
                        }
                        return true;
                    }
                }
            }

            // Equip
            if (activeTab == 0) {
                var eq = equipPanel.slotAtScreen(mouseX, mouseY);
                if (eq != null && eq.item() != null) {
                    InventoryItem item = eq.item();
                    if (shift) quickUnequipToGrid(eq.slotType(), item);
                    else {
                        eq.clearItem();
                        dragState.pickupFromEquip(item, eq.slotType());
                    }
                    return true;
                }
            }

            // Body inspect applied items (physical or meridian layer)
            if (activeTab == 1 && bodyInspect != null) {
                if (bodyInspect.activeLayer() == BodyInspectComponent.Layer.PHYSICAL) {
                    BodyPart bp = bodyInspect.bodyPartAtScreen(mouseX, mouseY);
                    if (bp != null) {
                        InventoryItem item = bodyInspect.physicalItemAt(bp);
                        if (item != null) {
                            if (shift) { bodyInspect.removePhysicalItem(bp); placeItemAnywhere(item); }
                            else { bodyInspect.removePhysicalItem(bp); dragState.pickupFromBodyPart(item, bp); }
                            return true;
                        }
                    }
                } else {
                    MeridianChannel ch = bodyInspect.channelAtScreen(mouseX, mouseY);
                    if (ch != null) {
                        InventoryItem item = bodyInspect.meridianItemAt(ch);
                        if (item != null) {
                            if (shift) { bodyInspect.removeMeridianItem(ch); placeItemAnywhere(item); }
                            else { bodyInspect.removeMeridianItem(ch); dragState.pickupFromMeridian(item, ch); }
                            return true;
                        }
                        // 无物品 — 纯点击即"选中此脉"，锁定详情面板
                        bodyInspect.clickSelectMeridian(mouseX, mouseY);
                        return true;
                    }
                }
            }

            // Hotbar
            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            if (hIdx >= 0 && hotbarItems[hIdx] != null) {
                InventoryItem item = hotbarItems[hIdx];
                if (shift) quickMoveHotbarToGrid(hIdx);
                else {
                    hotbarItems[hIdx] = null;
                    hotbarSlots[hIdx].clearItem();
                    dragState.pickupFromHotbar(item, hIdx);
                }
                return true;
            }

            // Quick-use bar (F1-F9)
            int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
            if (qIdx >= 0 && quickUseItems[qIdx] != null) {
                InventoryItem item = quickUseItems[qIdx];
                if (shift) quickMoveQuickUseToGrid(qIdx);
                else {
                    quickUseItems[qIdx] = null;
                    quickUseSlots[qIdx].clearItem();
                    publishQuickUseSlot(qIdx, null);
                    dragState.pickupFromQuickUse(item, qIdx);
                }
                return true;
            }
        }

        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double deltaX, double deltaY) {
        if (dragState.isDragging()) {
            dragState.updateMouse(mouseX, mouseY);
            updateHighlights(mouseX, mouseY);
            return true;
        }
        return super.mouseDragged(mouseX, mouseY, button, deltaX, deltaY);
    }

    @Override
    public boolean mouseReleased(double mouseX, double mouseY, int button) {
        if (button == 0 && dragState.isDragging()) {
            attemptDrop(mouseX, mouseY);
            return true;
        }
        return super.mouseReleased(mouseX, mouseY, button);
    }

    // ==================== Drag ====================

    private void attemptDrop(double mouseX, double mouseY) {
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) { dragState.cancel(); clearAllHighlights(); return; }

        // Capture source before drop() resets dragState; needed for the C2S move intent.
        com.bong.client.network.ClientRequestProtocol.InvLocation fromLoc = snapshotSourceLocation();

        // Discard
        if (isOverDiscard(mouseX, mouseY)) {
            dragState.drop();
            clearAllHighlights();
            return;
        }

        // Active grid
        BackpackGridPanel grid = activeGrid();
        if (grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null && grid.canPlace(dragged, pos.row(), pos.col())) {
                grid.place(dragged, pos.row(), pos.col());
                dragState.drop();
                dispatchMoveIntent(dragged, fromLoc,
                    new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                        grid.containerId(), pos.row(), pos.col()));
                clearAllHighlights();
                return;
            }
        }

        // Equip (with hand restriction from physical body)
        if (activeTab == 0) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null) {
                // Check if hand slot is usable
                if (!isEquipSlotUsable(eq.slotType())) {
                    // Can't equip — hand severed
                    returnDragToSource();
                    clearAllHighlights();
                    return;
                }
                if (eq.item() == null) {
                    eq.setItem(dragged);
                    dragState.drop();
                } else {
                    InventoryItem old = eq.item();
                    eq.setItem(dragged);
                    dragState.drop();
                    placeItemAnywhere(old);
                }
                dispatchMoveIntent(dragged, fromLoc,
                    new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                        eq.slotType().name().toLowerCase()));
                clearAllHighlights();
                return;
            }
        }

        // Body inspect drop (physical or meridian layer) — only 1×1 items
        if (activeTab == 1 && bodyInspect != null
                && dragged.gridWidth() == 1 && dragged.gridHeight() == 1) {
            if (bodyInspect.activeLayer() == BodyInspectComponent.Layer.PHYSICAL) {
                BodyPart bp = bodyInspect.bodyPartAtScreen(mouseX, mouseY);
                if (bp != null) {
                    InventoryItem existing = bodyInspect.physicalItemAt(bp);
                    bodyInspect.applyPhysicalItem(bp, dragged);
                    dragState.drop();
                    if (existing != null) placeItemAnywhere(existing);
                    clearAllHighlights();
                    return;
                }
            } else {
                MeridianChannel ch = bodyInspect.channelAtScreen(mouseX, mouseY);
                if (ch != null) {
                    InventoryItem existing = bodyInspect.meridianItemAt(ch);
                    bodyInspect.applyMeridianItem(ch, dragged);
                    dragState.drop();
                    if (existing != null) placeItemAnywhere(existing);
                    clearAllHighlights();
                    return;
                }
            }
        }

        // Hotbar
        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0 && dragged.gridWidth() == 1 && dragged.gridHeight() == 1) {
            if (hotbarItems[hIdx] == null) {
                hotbarItems[hIdx] = dragged;
                hotbarSlots[hIdx].setItem(dragged, true);
                dragState.drop();
            } else {
                InventoryItem old = hotbarItems[hIdx];
                hotbarItems[hIdx] = dragged;
                hotbarSlots[hIdx].setItem(dragged, true);
                dragState.drop();
                placeItemAnywhere(old);
            }
            dispatchMoveIntent(dragged, fromLoc,
                new com.bong.client.network.ClientRequestProtocol.HotbarLoc(hIdx));
            clearAllHighlights();
            return;
        }

        // Quick-use bar (F1-F9)
        int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
        if (qIdx >= 0 && dragged.gridWidth() == 1 && dragged.gridHeight() == 1) {
            InventoryItem old = quickUseItems[qIdx];
            quickUseItems[qIdx] = dragged;
            quickUseSlots[qIdx].setItem(dragged, true);
            publishQuickUseSlot(qIdx, dragged);
            dragState.drop();
            if (old != null) placeItemAnywhere(old);
            clearAllHighlights();
            return;
        }

        returnDragToSource();
        clearAllHighlights();
    }

    /**
     * 在 dragState.drop() 之前调用，从当前 dragState 计算 server-shaped {@code from}。
     * 仅 GRID/EQUIP/HOTBAR 三种来源对应 server 库存；QUICK_USE/MERIDIAN/BODY_PART 返回 null
     * （server 端无对应表示，move intent 不发）。
     */
    private com.bong.client.network.ClientRequestProtocol.InvLocation snapshotSourceLocation() {
        if (dragState.sourceKind() == null) return null;
        return switch (dragState.sourceKind()) {
            case GRID -> {
                String cid = dragState.sourceContainerId();
                if (cid == null) yield null;
                yield new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    cid, dragState.sourceRow(), dragState.sourceCol());
            }
            case EQUIP -> dragState.sourceEquipSlot() == null ? null
                : new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                    dragState.sourceEquipSlot().name().toLowerCase());
            case HOTBAR -> dragState.sourceHotbarIndex() < 0 ? null
                : new com.bong.client.network.ClientRequestProtocol.HotbarLoc(
                    dragState.sourceHotbarIndex());
            case QUICK_USE, MERIDIAN, BODY_PART -> null;
        };
    }

    private void dispatchMoveIntent(
        InventoryItem item,
        com.bong.client.network.ClientRequestProtocol.InvLocation from,
        com.bong.client.network.ClientRequestProtocol.InvLocation to
    ) {
        if (item == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent skipped: item is null");
            return;
        }
        if (from == null || to == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent skipped: from={} to={} item={}",
                from, to, item.itemId());
            return;
        }
        if (item.instanceId() == 0L) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent skipped: item {} has instanceId=0 "
                    + "(likely Mock data — server snapshot didn't load)",
                item.itemId());
            return;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchMoveIntent instance={} from={} to={} item={}",
            item.instanceId(), from, to, item.itemId());
        com.bong.client.network.ClientRequestSender.sendInventoryMove(item.instanceId(), from, to);
    }

    private void returnDragToSource() {
        DragState.CancelResult r = dragState.cancel();
        if (!r.hasItem()) return;
        InventoryItem item = r.item();
        if (r.sourceKind() == null) { placeItemAnywhere(item); return; }
        switch (r.sourceKind()) {
            case GRID -> {
                // Try return to the active grid (user may have switched containers)
                BackpackGridPanel grid = activeGrid();
                if (grid.canPlace(item, r.sourceRow(), r.sourceCol()))
                    grid.place(item, r.sourceRow(), r.sourceCol());
                else placeItemAnywhere(item);
            }
            case EQUIP -> {
                if (r.sourceEquipSlot() != null) {
                    var slot = equipPanel.slotFor(r.sourceEquipSlot());
                    if (slot != null) slot.setItem(item);
                }
            }
            case HOTBAR -> {
                int idx = r.sourceHotbarIndex();
                if (idx >= 0 && idx < HOTBAR_SLOTS) {
                    hotbarItems[idx] = item;
                    hotbarSlots[idx].setItem(item, true);
                }
            }
            case QUICK_USE -> {
                int idx = r.sourceQuickUseIndex();
                if (idx >= 0 && idx < HOTBAR_SLOTS) {
                    quickUseItems[idx] = item;
                    quickUseSlots[idx].setItem(item, true);
                    publishQuickUseSlot(idx, item);
                }
            }
            case MERIDIAN -> {
                MeridianChannel ch = r.sourceMeridianChannel();
                if (ch != null && bodyInspect != null) bodyInspect.applyMeridianItem(ch, item);
            }
            case BODY_PART -> {
                BodyPart bp = r.sourceBodyPart();
                if (bp != null && bodyInspect != null) bodyInspect.applyPhysicalItem(bp, item);
            }
        }
    }

    private void placeItemAnywhere(InventoryItem item) {
        // 优先放当前容器
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) { activeGrid().place(item, pos.row(), pos.col()); return; }
        // 尝试其他容器
        for (int i = 0; i < containerCount; i++) {
            if (i == activeContainer) continue;
            var pos2 = containerGrids[i].findFreeSpace(item);
            if (pos2 != null) { containerGrids[i].place(item, pos2.row(), pos2.col()); return; }
        }
        // 所有容器都满了 — 放快捷栏（仅 1×1）
        if (item.gridWidth() == 1 && item.gridHeight() == 1) {
            for (int i = 0; i < HOTBAR_SLOTS; i++) {
                if (hotbarItems[i] == null) {
                    hotbarItems[i] = item;
                    hotbarSlots[i].setItem(item, true);
                    return;
                }
            }
        }
        // 实在放不下 — 强制放进当前容器第一格（覆盖，避免数据丢失）
        activeGrid().place(item, 0, 0);
    }

    // ==================== Highlights ====================

    private void updateHighlights(double mouseX, double mouseY) {
        clearAllHighlights();
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) return;

        BackpackGridPanel grid = activeGrid();
        if (grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                boolean valid = grid.canPlace(dragged, pos.row(), pos.col());
                grid.highlightArea(pos.row(), pos.col(), dragged.gridWidth(), dragged.gridHeight(),
                    valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
            }
        }

        if (activeTab == 0) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null) {
                boolean usable = isEquipSlotUsable(eq.slotType());
                eq.setHighlightState(usable
                    ? GridSlotComponent.HighlightState.VALID
                    : GridSlotComponent.HighlightState.INVALID);
            }
        }

        // Body inspect highlight
        if (activeTab == 1 && bodyInspect != null) {
            boolean valid1x1 = dragged.gridWidth() == 1 && dragged.gridHeight() == 1;
            if (bodyInspect.activeLayer() == BodyInspectComponent.Layer.PHYSICAL) {
                BodyPart bp = bodyInspect.bodyPartAtScreen(mouseX, mouseY);
                if (bp != null) bodyInspect.setPhysicalHighlight(bp, valid1x1);
            } else {
                MeridianChannel ch = bodyInspect.channelAtScreen(mouseX, mouseY);
                if (ch != null) bodyInspect.setMeridianHighlight(ch, valid1x1);
            }
        }

        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0) {
            boolean valid = dragged.gridWidth() == 1 && dragged.gridHeight() == 1;
            hotbarSlots[hIdx].setHighlightState(
                valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
        }

        int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
        if (qIdx >= 0) {
            boolean valid = dragged.gridWidth() == 1 && dragged.gridHeight() == 1;
            quickUseSlots[qIdx].setHighlightState(
                valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
        }

        discardStrip.surface(Surface.flat(isOverDiscard(mouseX, mouseY) ? 0xFF331111 : 0xFF201010));
    }

    private void clearAllHighlights() {
        for (BackpackGridPanel g : containerGrids) g.clearHighlights();
        equipPanel.clearHighlights();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            if (hotbarSlots[i] != null) hotbarSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
            if (quickUseSlots[i] != null) quickUseSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
        }
        if (bodyInspect != null) bodyInspect.clearHighlight();
        discardStrip.surface(Surface.flat(0xFF201010));
    }

    /** 检查装备槽是否可用（断臂不能持物） */
    private boolean isEquipSlotUsable(EquipSlotType slot) {
        PhysicalBody pb = bodyInspect != null ? bodyInspect.physicalBody() : null;
        if (pb == null) return true; // 无体表数据时不限制
        return switch (slot) {
            case MAIN_HAND, TWO_HAND -> pb.canUseHand(PhysicalBody.Side.RIGHT);
            case OFF_HAND -> pb.canUseHand(PhysicalBody.Side.LEFT);
            default -> true;
        };
    }

    // ==================== Quick operations ====================

    private void quickEquipFromGrid(InventoryItem item) {
        for (EquipSlotType type : EquipSlotType.values()) {
            var slot = equipPanel.slotFor(type);
            if (slot != null && slot.item() == null) {
                activeGrid().remove(item);
                slot.setItem(item);
                return;
            }
        }
    }

    private void quickUnequipToGrid(EquipSlotType slotType, InventoryItem item) {
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            equipPanel.slotFor(slotType).clearItem();
            activeGrid().place(item, pos.row(), pos.col());
        }
    }

    private void quickMoveHotbarToGrid(int index) {
        InventoryItem item = hotbarItems[index];
        if (item == null) return;
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            hotbarItems[index] = null;
            hotbarSlots[index].clearItem();
            activeGrid().place(item, pos.row(), pos.col());
        }
    }

    private void quickMoveQuickUseToGrid(int index) {
        InventoryItem item = quickUseItems[index];
        if (item == null) return;
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            quickUseItems[index] = null;
            quickUseSlots[index].clearItem();
            publishQuickUseSlot(index, null);
            activeGrid().place(item, pos.row(), pos.col());
        }
    }

    // ==================== Render ====================

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        super.render(context, mouseX, mouseY, delta);
        drawMultiCellItems(context);
        updateTooltipFromHover(mouseX, mouseY);

        // Body inspect tooltip — drawn here to escape owo-lib component clipping
        if (activeTab == 1 && bodyInspect != null) {
            var matrices = context.getMatrices();
            matrices.push();
            matrices.translate(0, 0, 400);
            bodyInspect.drawTooltip(context, mouseX, mouseY);
            matrices.pop();
        }

        if (dragState.isDragging() && dragState.draggedItem() != null) {
            InventoryItem item = dragState.draggedItem();
            int cs = GridSlotComponent.CELL_SIZE;
            int gw = item.gridWidth() * cs, gh = item.gridHeight() * cs;

            Identifier tex = new Identifier("bong-client", "textures/gui/items/" + item.itemId() + ".png");
            var matrices = context.getMatrices();
            matrices.push();
            matrices.translate(0, 0, 200);

            int fitSize = Math.min(gw, gh);
            int fitX = mouseX - fitSize / 2, fitY = mouseY - fitSize / 2;

            RenderSystem.enableBlend();
            RenderSystem.defaultBlendFunc();
            RenderSystem.setShaderColor(1f, 1f, 1f, 0.75f);
            matrices.push();
            matrices.translate(fitX, fitY, 0);
            matrices.scale((float) fitSize / ICON_SIZE, (float) fitSize / ICON_SIZE, 1f);
            context.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
            matrices.pop();

            RenderSystem.setShaderColor(1f, 1f, 1f, 1f);
            RenderSystem.disableBlend();
            matrices.pop();
        }
    }

    private void drawMultiCellItems(DrawContext context) {
        BackpackGridPanel grid = activeGrid();
        for (var entry : grid.toGridEntries()) {
            InventoryItem item = entry.item();
            if (item.gridWidth() == 1 && item.gridHeight() == 1) continue;
            if (dragState.isDragging() && dragState.draggedItem() == item) continue;

            GridSlotComponent anchor = grid.slotAt(entry.row(), entry.col());
            if (anchor == null) continue;

            int px = anchor.x() + 2, py = anchor.y() + 2;
            int pw = item.gridWidth() * GridSlotComponent.CELL_SIZE - 4;
            int ph = item.gridHeight() * GridSlotComponent.CELL_SIZE - 4;
            drawItemTextureRaw(context, item, px, py, pw, ph);
            GridSlotComponent.drawItemOverlays(
                context, item,
                anchor.x(), anchor.y(),
                item.gridWidth() * GridSlotComponent.CELL_SIZE,
                item.gridHeight() * GridSlotComponent.CELL_SIZE
            );
        }
    }

    private static void drawItemTextureRaw(DrawContext ctx, InventoryItem item, int dx, int dy, int dw, int dh) {
        if (item == null || item.isEmpty()) return;
        Identifier tex = new Identifier("bong-client", "textures/gui/items/" + item.itemId() + ".png");
        int fitSize = Math.min(dw, dh);
        int ox = (dw - fitSize) / 2, oy = (dh - fitSize) / 2;

        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.enableDepthTest();
        var m = ctx.getMatrices();
        m.push();
        m.translate(dx + ox, dy + oy, 100);
        m.scale((float) fitSize / ICON_SIZE, (float) fitSize / ICON_SIZE, 1f);
        ctx.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
        m.pop();
        RenderSystem.disableBlend();
    }

    private void updateTooltipFromHover(double mx, double my) {
        if (dragState.isDragging()) { tooltipPanel.setHoveredItem(dragState.draggedItem()); return; }
        InventoryItem hovered = null;
        BackpackGridPanel grid = activeGrid();
        if (grid.containsPoint(mx, my)) {
            var pos = grid.screenToGrid(mx, my);
            if (pos != null) hovered = grid.itemAt(pos.row(), pos.col());
        }
        if (hovered == null && activeTab == 0) {
            var eq = equipPanel.slotAtScreen(mx, my);
            if (eq != null) hovered = eq.item();
        }
        if (hovered == null && activeTab == 1 && bodyInspect != null) {
            if (bodyInspect.activeLayer() == BodyInspectComponent.Layer.PHYSICAL) {
                BodyPart bp = bodyInspect.bodyPartAtScreen(mx, my);
                if (bp != null) hovered = bodyInspect.physicalItemAt(bp);
            } else {
                MeridianChannel ch = bodyInspect.channelAtScreen(mx, my);
                if (ch != null) hovered = bodyInspect.meridianItemAt(ch);
            }
        }
        if (hovered == null) {
            int idx = hotbarSlotAtScreen(mx, my);
            if (idx >= 0) hovered = hotbarItems[idx];
        }
        if (hovered == null) {
            int idx = quickUseSlotAtScreen(mx, my);
            if (idx >= 0) hovered = quickUseItems[idx];
        }
        tooltipPanel.setHoveredItem(hovered);
    }
}
