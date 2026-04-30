package com.bong.client.inventory;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
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
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;

import java.util.Locale;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
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
    // plan-skill-v1 §5.1 第三个 tab "技艺"
    private final LabelComponent[] tabLabels = new LabelComponent[4];
    private FlowLayout equipTabContent;
    private FlowLayout cultivationTabContent;
    private FlowLayout skillTabContent;
    private com.bong.client.combat.inspect.CombatTrainingPanel combatTrainingPanel;
    private FlowLayout combatTrainingTabContent;
    private FlowLayout skillScrollDropZone;
    private LabelComponent skillScrollDropTitle;
    private LabelComponent skillScrollDropHint;
    private String skillScrollDropFeedback = "仅 skill 残卷可悟";
    // plan-skill-v1 §5.1 三行固定（herbalism / alchemy / forging）
    private com.bong.client.skill.SkillRowComponent[] skillRows;
    private com.bong.client.skill.SkillId selectedSkill = com.bong.client.skill.SkillId.HERBALISM;
    private LabelComponent skillDetailTitle;
    private LabelComponent skillDetailLevel;
    private LabelComponent skillDetailProgress;
    private LabelComponent skillDetailCurrent;
    private LabelComponent skillDetailNext;
    private LabelComponent skillDetailHint;
    private LabelComponent skillRecentHeader;
    private LabelComponent[] skillRecentLines;
    private LabelComponent skillMilestoneHeader;
    private LabelComponent[] skillMilestoneLines;
    /** Screen 存活期间持有的 SkillSetStore 订阅，close 时解绑避免泄漏。 */
    private java.util.function.Consumer<com.bong.client.skill.SkillSetSnapshot> skillListener;

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

    record PillMenuAction(String label, ActionKind kind) {}
    enum ActionKind { SELF_USE, MERIDIAN_TARGET, PLACE_FORGE_STATION, PLACE_SPIRIT_NICHE }
    record PillContextMenuState(InventoryItem item, int x, int y, List<PillMenuAction> actions) {}
    record PendingMeridianUse(InventoryItem item) {}
    record WeaponMenuAction(String label, WeaponActionKind kind) {}
    enum WeaponActionKind { REPAIR, DROP }
    record WeaponContextMenuState(InventoryItem item, EquipSlotType slotType, int x, int y, List<WeaponMenuAction> actions) {}

    private PillContextMenuState pillContextMenu;
    private PendingMeridianUse pendingMeridianUse;
    private WeaponContextMenuState weaponContextMenu;

    private static final int PILL_MENU_WIDTH = 112;
    private static final int PILL_MENU_ROW_HEIGHT = 16;
    private static final int PILL_MENU_PADDING = 4;
    private static final int PILL_MENU_BG = 0xEE151515;
    private static final int PILL_MENU_BORDER = 0xFF777777;
    private static final int PILL_MENU_TEXT = 0xFFE8E8E8;
    private static final int PILL_MENU_HOVER = 0xFF2A2A2A;
    private static final int PILL_TARGET_HINT = 0xFFFFD060;

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
        if (skillListener != null) {
            com.bong.client.skill.SkillSetStore.removeListener(skillListener);
            skillListener = null;
        }
        if (combatTrainingPanel != null) {
            combatTrainingPanel.close();
            combatTrainingPanel = null;
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
        String[] tabNames = {"装备", "修仙", "技艺", "战斗·修炼"};
        for (int i = 0; i < tabNames.length; i++) {
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
            if (b.hasLifespanPreview()) {
                if (sb.length() > 0) sb.append("  §8·  ");
                sb.append(String.format("§7寿元 §f%.1f/%d", b.yearsLived(), b.lifespanCapByRealm()));
                sb.append(String.format(" §8(余%.1f 扣%d ×%.1f)",
                    b.remainingYears(), b.deathPenaltyYears(), b.lifespanTickRateMultiplier()));
                if (b.isWindCandle()) sb.append(" §c风烛");
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

        // Tab 2: 技艺 (plan-skill-v1 §5.1)
        //   现阶段做最小闭环：左列固定三行 + 选中高亮；中列展示当前生效等级、cap 压制与效果说明。
        //   曲线 canvas / 里程碑 / 残卷拖入槽留 P4/P5/P6。
        skillTabContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        skillTabContent.gap(2);
        skillTabContent.padding(Insets.of(2));

        skillRows = new com.bong.client.skill.SkillRowComponent[3];
        com.bong.client.skill.SkillId[] order = {
            com.bong.client.skill.SkillId.HERBALISM,
            com.bong.client.skill.SkillId.ALCHEMY,
            com.bong.client.skill.SkillId.FORGING,
        };
        for (int i = 0; i < order.length; i++) {
            skillRows[i] = new com.bong.client.skill.SkillRowComponent(order[i]);
            final com.bong.client.skill.SkillId skillId = order[i];
            skillRows[i].component().cursorStyle(CursorStyle.HAND);
            skillRows[i].component().mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) {
                    selectedSkill = skillId;
                    refreshSkillRows(com.bong.client.skill.SkillSetStore.snapshot());
                    return true;
                }
                return false;
            });
            skillTabContent.child(skillRows[i].component());
        }
        skillTabContent.child(buildSkillScrollDropZone());
        skillTabContent.child(buildSkillDetailPanel());

        // 初次填充
        refreshSkillRows(com.bong.client.skill.SkillSetStore.snapshot());
        // 订阅更新 —— 回主线程再刷 UI，避免网络线程 mutate owo-lib 组件。
        skillListener = next -> {
            if (next == null) return;
            MinecraftClient.getInstance().execute(() -> refreshSkillRows(next));
        };
        com.bong.client.skill.SkillSetStore.addListener(skillListener);

        leftCol.child(skillTabContent);
        skillTabContent.positioning(Positioning.absolute(-9999, -9999));

        // Tab 3: 战斗·修炼（plan-hotbar-modify-v1 §4）
        combatTrainingPanel = new com.bong.client.combat.inspect.CombatTrainingPanel();
        combatTrainingTabContent = combatTrainingPanel.component();
        leftCol.child(combatTrainingTabContent);
        combatTrainingTabContent.positioning(Positioning.absolute(-9999, -9999));

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

    private void hydrateSkillBarFromStore() {
        var config = SkillBarStore.snapshot();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            SkillBarEntry entry = config.slot(i);
            if (entry == null || entry.kind() != SkillBarEntry.Kind.SKILL) {
                if (hotbarSlots[i] != null) {
                    if (hotbarItems[i] != null) hotbarSlots[i].setItem(hotbarItems[i], true);
                    else hotbarSlots[i].clearItem();
                }
                continue;
            }
            if (hotbarSlots[i] != null) {
                hotbarSlots[i].setItem(InventoryItem.simple("skill_scroll_" + safeSkillIconId(entry.id()), entry.displayName()), true);
            }
        }
    }

    private static String safeSkillIconId(String skillId) {
        if (skillId == null || skillId.isBlank()) return "unknown";
        return skillId.replace('.', '_').replace(':', '_').replace('/', '_');
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
        FlowLayout[] tabs = {equipTabContent, cultivationTabContent, skillTabContent, combatTrainingTabContent};
        for (int i = 0; i < tabs.length; i++) {
            tabLabels[i].color(Color.ofArgb(i == idx ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            if (tabs[i] != null) {
                tabs[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
            }
        }
        // 切到技艺 tab 时刷一次最新快照（离开其他 tab 时可能积攒了若干事件）。
        if (idx == 2) {
            refreshSkillRows(com.bong.client.skill.SkillSetStore.snapshot());
        } else if (idx == 3 && combatTrainingPanel != null) {
            combatTrainingPanel.refreshFromStores();
            hydrateSkillBarFromStore();
        }
    }

    /** plan-skill-v1 §5.1 三行固定刷新；listener / switchTab 共用此入口。 */
    private void refreshSkillRows(com.bong.client.skill.SkillSetSnapshot snapshot) {
        if (skillRows == null || snapshot == null) return;
        long now = System.currentTimeMillis();
        for (com.bong.client.skill.SkillRowComponent row : skillRows) {
            if (row == null) continue;
            row.update(snapshot.get(row.skill()), now);
            row.setSelected(row.skill() == selectedSkill);
        }
        refreshSkillDetail(snapshot.get(selectedSkill));
    }

    private FlowLayout buildSkillDetailPanel() {
        FlowLayout panel = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        panel.surface(Surface.flat(0xFF121712).and(Surface.outline(0xFF4A5C46)));
        panel.padding(Insets.of(4));
        panel.gap(2);

        skillDetailTitle = Components.label(Text.literal("采药"));
        skillDetailTitle.color(Color.ofArgb(0xFFD8E4D0));
        panel.child(skillDetailTitle);

        skillDetailLevel = Components.label(Text.literal("Lv.0 / effective 0 / cap 10"));
        skillDetailLevel.color(Color.ofArgb(0xFFE0B060));
        panel.child(skillDetailLevel);

        skillDetailProgress = Components.label(Text.literal("当前 XP 0 / 100 · 累计 0"));
        skillDetailProgress.color(Color.ofArgb(0xFFAAAAAA));
        panel.child(skillDetailProgress);

        skillDetailCurrent = Components.label(Text.literal("当前效果：尚未入门。"));
        skillDetailCurrent.color(Color.ofArgb(0xFFCCCCCC));
        skillDetailCurrent.maxWidth(160);
        panel.child(skillDetailCurrent);

        skillDetailNext = Components.label(Text.literal("下一阶：继续修习可见首层变化。"));
        skillDetailNext.color(Color.ofArgb(0xFF88B090));
        skillDetailNext.maxWidth(160);
        panel.child(skillDetailNext);

        skillDetailHint = Components.label(Text.literal("点左侧条目切换；若境界不足，超出 cap 的等级只按 effective 生效。"));
        skillDetailHint.color(Color.ofArgb(0xFF666666));
        skillDetailHint.maxWidth(160);
        panel.child(skillDetailHint);

        skillRecentHeader = Components.label(Text.literal("近期流水"));
        skillRecentHeader.color(Color.ofArgb(0xFF88B090));
        panel.child(skillRecentHeader);

        skillRecentLines = new LabelComponent[3];
        for (int i = 0; i < skillRecentLines.length; i++) {
            LabelComponent line = Components.label(Text.literal("§8（暂无）"));
            line.color(Color.ofArgb(0xFF888888));
            line.maxWidth(160);
            skillRecentLines[i] = line;
            panel.child(line);
        }

        skillMilestoneHeader = Components.label(Text.literal("最近里程碑"));
        skillMilestoneHeader.color(Color.ofArgb(0xFF88B090));
        panel.child(skillMilestoneHeader);

        skillMilestoneLines = new LabelComponent[3];
        for (int i = 0; i < skillMilestoneLines.length; i++) {
            LabelComponent line = Components.label(Text.literal("§8（暂无）"));
            line.color(Color.ofArgb(0xFF888888));
            line.maxWidth(160);
            skillMilestoneLines[i] = line;
            panel.child(line);
        }

        return panel;
    }

    private FlowLayout buildSkillScrollDropZone() {
        FlowLayout zone = Containers.verticalFlow(Sizing.fixed(56), Sizing.fixed(68));
        zone.surface(Surface.flat(0xFF201A14).and(Surface.outline(0xFF6A5030)));
        zone.padding(Insets.of(3));
        zone.gap(2);
        zone.horizontalAlignment(HorizontalAlignment.CENTER);
        zone.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout slot = Containers.verticalFlow(Sizing.fixed(28), Sizing.fixed(56));
        slot.surface(Surface.flat(0xFF15110E).and(Surface.outline(0xFF8A6A40)));
        zone.child(slot);

        skillScrollDropTitle = Components.label(Text.literal("技能残卷"));
        skillScrollDropTitle.color(Color.ofArgb(0xFFB89A68));
        zone.child(skillScrollDropTitle);

        skillScrollDropHint = Components.label(Text.literal(skillScrollDropFeedback));
        skillScrollDropHint.color(Color.ofArgb(0xFF888888));
        zone.child(skillScrollDropHint);
        skillScrollDropZone = zone;
        return zone;
    }

    private void refreshSkillDetail(com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skillDetailTitle == null || entry == null) return;
        skillDetailTitle.text(Text.literal(selectedSkill.displayName()));
        skillDetailLevel.text(Text.literal(formatSkillLevelLine(selectedSkill, entry)));
        skillDetailLevel.color(Color.ofArgb(entry.lv() > entry.cap() ? 0xFF907050 : 0xFFE0B060));
        skillDetailProgress.text(Text.literal(formatSkillProgressLine(entry)));
        skillDetailCurrent.text(Text.literal(formatSkillCurrentEffect(selectedSkill, entry)));
        skillDetailNext.text(Text.literal(formatSkillNextEffect(selectedSkill, entry)));
        skillDetailHint.text(Text.literal(formatSkillHint(entry)));
        refreshSkillRecentEvents();
        refreshSkillMilestones();
    }

    private void refreshSkillRecentEvents() {
        if (skillRecentHeader == null || skillRecentLines == null) return;
        java.util.List<com.bong.client.skill.SkillRecentEventStore.Entry> lines = recentEventsForSkill(selectedSkill);
        skillRecentHeader.text(Text.literal("近期流水" + (lines.isEmpty() ? "" : " · " + selectedSkill.displayName())));
        for (int i = 0; i < skillRecentLines.length; i++) {
            LabelComponent line = skillRecentLines[i];
            if (line == null) continue;
            if (i >= lines.size()) {
                line.text(Text.literal(i == 0 ? "§8（暂无）" : ""));
                line.color(Color.ofArgb(0xFF888888));
                continue;
            }
            line.text(Text.literal(formatSkillRecentEventLine(lines.get(i))));
            line.color(Color.ofArgb(0xFFAAAAAA));
        }
    }

    private void refreshSkillMilestones() {
        if (skillMilestoneHeader == null || skillMilestoneLines == null) return;
        java.util.List<com.bong.client.skill.SkillMilestoneSnapshot> lines = recentMilestonesForSkill(selectedSkill);
        skillMilestoneHeader.text(Text.literal("最近里程碑" + (lines.isEmpty() ? "" : " · " + selectedSkill.displayName())));
        for (int i = 0; i < skillMilestoneLines.length; i++) {
            LabelComponent line = skillMilestoneLines[i];
            if (line == null) continue;
            if (i >= lines.size()) {
                line.text(Text.literal(i == 0 ? "§8（暂无）" : ""));
                line.color(Color.ofArgb(0xFF888888));
                continue;
            }
            line.text(Text.literal(formatSkillMilestoneLine(lines.get(i))));
            line.color(Color.ofArgb(0xFFAAAAAA));
        }
    }

    private static java.util.List<com.bong.client.skill.SkillMilestoneSnapshot> recentMilestonesForSkill(
        com.bong.client.skill.SkillId skill
    ) {
        java.util.List<com.bong.client.skill.SkillMilestoneSnapshot> all =
            com.bong.client.skill.SkillMilestoneStore.snapshot();
        java.util.ArrayList<com.bong.client.skill.SkillMilestoneSnapshot> filtered = new java.util.ArrayList<>();
        for (int i = all.size() - 1; i >= 0 && filtered.size() < 3; i--) {
            com.bong.client.skill.SkillMilestoneSnapshot snapshot = all.get(i);
            if (snapshot != null && snapshot.skill() == skill) {
                filtered.add(snapshot);
            }
        }
        return java.util.List.copyOf(filtered);
    }

    private static java.util.List<com.bong.client.skill.SkillRecentEventStore.Entry> recentEventsForSkill(
        com.bong.client.skill.SkillId skill
    ) {
        java.util.List<com.bong.client.skill.SkillRecentEventStore.Entry> all =
            com.bong.client.skill.SkillRecentEventStore.snapshot();
        java.util.ArrayList<com.bong.client.skill.SkillRecentEventStore.Entry> filtered = new java.util.ArrayList<>();
        for (com.bong.client.skill.SkillRecentEventStore.Entry entry : all) {
            if (entry != null && entry.skill() == skill) {
                filtered.add(entry);
                if (filtered.size() >= 3) break;
            }
        }
        return java.util.List.copyOf(filtered);
    }

    static String formatSkillRecentEventLine(com.bong.client.skill.SkillRecentEventStore.Entry entry) {
        if (entry == null) return "（暂无）";
        return switch (entry.kind()) {
            case "xp_gain" -> entry.text();
            case "lv_up" -> entry.text();
            case "cap_changed" -> entry.text();
            case "scroll_used" -> entry.text();
            default -> entry.text();
        };
    }

    static String formatSkillMilestoneLine(com.bong.client.skill.SkillMilestoneSnapshot milestone) {
        if (milestone == null) return "（暂无）";
        String narration = milestone.narration();
        if (narration != null && !narration.isBlank()) {
            return "Lv." + milestone.newLv() + " · " + narration;
        }
        return "Lv." + milestone.newLv() + " · t" + milestone.achievedAt() + " · 累计 " + milestone.totalXpAt() + " XP";
    }

    static String formatSkillLevelLine(com.bong.client.skill.SkillId skill, com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skill == null || entry == null) return "Lv.0 / effective 0 / cap 10";
        String line = "Lv." + entry.lv() + " / effective " + entry.effectiveLv() + " / cap " + entry.cap();
        if (entry.lv() > entry.cap()) {
            line += " · 境界压制";
        }
        return line;
    }

    static String formatSkillProgressLine(com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (entry == null) return "当前 XP 0 / 100 · 累计 0";
        if (entry.lv() >= 10) {
            return "Lv.10 已满 · 累计 " + entry.totalXp() + " XP";
        }
        return "当前 XP " + entry.xp() + " / " + entry.xpToNext() + " · 累计 " + entry.totalXp();
    }

    static String formatSkillCurrentEffect(com.bong.client.skill.SkillId skill, com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skill == null || entry == null) return "当前效果：尚未入门。";
        int lv = entry.effectiveLv();
        return "当前效果：" + switch (skill) {
            case HERBALISM -> herbalismCurrentEffect(lv);
            case ALCHEMY -> alchemyCurrentEffect(lv);
            case FORGING -> forgingCurrentEffect(lv);
        };
    }

    static String formatSkillNextEffect(com.bong.client.skill.SkillId skill, com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skill == null || entry == null) return "下一阶：继续修习可见首层变化。";
        int effective = entry.effectiveLv();
        if (entry.lv() >= 10) {
            return "下一阶：已至极限，后续只看境界能否完全承住这门手艺。";
        }
        if (entry.lv() > entry.cap()) {
            return "下一阶：真实等级已高于境界上限；待突破后，压住的效果会直接放开。";
        }
        int nextLv = Math.min(10, effective + 1);
        return "下一阶：effective 提到 " + nextLv + " 时，"
            + switch (skill) {
                case HERBALISM -> herbalismNextEffect(nextLv);
                case ALCHEMY -> alchemyNextEffect(nextLv);
                case FORGING -> forgingNextEffect(nextLv);
            };
    }

    static String formatSkillHint(com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (entry == null) return "点左侧条目切换；若境界不足，超出 cap 的等级只按 effective 生效。";
        if (entry.lv() > entry.cap()) {
            return "你已练到更高层次，但经脉未承住，只能按 effective_lv 发挥。";
        }
        if (entry.cap() < 10) {
            return "当前境界最多承到 cap " + entry.cap() + "；继续突破后，高等级效果会自然放开。";
        }
        return "当前境界已不再压制这门技艺，条目显示的 real_lv 就是实际生效等级。";
    }

    private static String herbalismCurrentEffect(int effectiveLv) {
        return String.format(
            Locale.ROOT,
            "手动采集 %.1fs，加成种子掉率 +%s%%，品质偏移 +%s%%。%s",
            herbalismManualDurationDelta(effectiveLv),
            formatPercent1(herbalismSeedBonus(effectiveLv)),
            formatInt(herbalismQualityBias(effectiveLv)),
            herbalismAutoText(effectiveLv)
        );
    }

    private static String herbalismNextEffect(int nextLv) {
        return String.format(
            Locale.ROOT,
            "手动采集 %.1fs，种子掉率 +%s%%，品质偏移 +%s%%。%s",
            herbalismManualDurationDelta(nextLv),
            formatPercent1(herbalismSeedBonus(nextLv)),
            formatInt(herbalismQualityBias(nextLv)),
            herbalismAutoText(nextLv)
        );
    }

    private static String alchemyCurrentEffect(int effectiveLv) {
        return String.format(
            Locale.ROOT,
            "火候容差 ×%s，坏副作用权重 ×%s，丹毒排异 +%s%%。",
            formatPercent2(alchemyToleranceScale(effectiveLv)),
            formatPercent2(alchemyBadWeightScale(effectiveLv)),
            formatPercent1(alchemyPurgeBonus(effectiveLv) * 100.0)
        );
    }

    private static String alchemyNextEffect(int nextLv) {
        return String.format(
            Locale.ROOT,
            "火候容差 ×%s，坏副作用权重 ×%s，丹毒排异 +%s%%。",
            formatPercent2(alchemyToleranceScale(nextLv)),
            formatPercent2(alchemyBadWeightScale(nextLv)),
            formatPercent1(alchemyPurgeBonus(nextLv) * 100.0)
        );
    }

    private static String forgingCurrentEffect(int effectiveLv) {
        return String.format(
            Locale.ROOT,
            "淬火命中窗 +%s tick，允许失误 +%s，铭文失败率 -%s%%。",
            formatInt(forgingWindowBonus(effectiveLv)),
            formatInt(forgingAllowedMiss(effectiveLv)),
            formatPercent1(forgingFailureReduction(effectiveLv) * 100.0)
        );
    }

    private static String forgingNextEffect(int nextLv) {
        return String.format(
            Locale.ROOT,
            "淬火命中窗 +%s tick，允许失误 +%s，铭文失败率 -%s%%。",
            formatInt(forgingWindowBonus(nextLv)),
            formatInt(forgingAllowedMiss(nextLv)),
            formatPercent1(forgingFailureReduction(nextLv) * 100.0)
        );
    }

    private static String herbalismAutoText(int effectiveLv) {
        if (effectiveLv < 3) return "自动采集未开。";
        return String.format(Locale.ROOT, "自动采集已开，时长 %.1fs。", herbalismAutoDuration(effectiveLv));
    }

    private static double herbalismManualDurationDelta(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, -0.2}, {3, -0.5}, {5, -1.0}, {7, -1.2}, {10, -1.5}
        });
    }

    private static double herbalismSeedBonus(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 2.0}, {3, 5.0}, {5, 10.0}, {7, 15.0}, {10, 25.0}
        });
    }

    private static double herbalismQualityBias(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 5.0}, {3, 10.0}, {5, 15.0}, {7, 20.0}, {10, 30.0}
        });
    }

    private static double herbalismAutoDuration(int effectiveLv) {
        if (effectiveLv < 3) return 0.0;
        return interpolate(effectiveLv, new double[][] {
            {3, 8.0}, {5, 6.0}, {7, 5.0}, {10, 5.0}
        });
    }

    private static double alchemyToleranceScale(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 1.00}, {1, 1.05}, {3, 1.15}, {5, 1.25}, {7, 1.35}, {10, 1.50}
        });
    }

    private static double alchemyBadWeightScale(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 1.00}, {1, 0.95}, {3, 0.85}, {5, 0.75}, {7, 0.60}, {10, 0.40}
        });
    }

    private static double alchemyPurgeBonus(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.00}, {1, 0.02}, {3, 0.05}, {5, 0.10}, {7, 0.15}, {10, 0.25}
        });
    }

    private static double forgingWindowBonus(int effectiveLv) {
        return Math.round(interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 1.0}, {3, 3.0}, {5, 5.0}, {7, 6.0}, {10, 8.0}
        }));
    }

    private static double forgingAllowedMiss(int effectiveLv) {
        return Math.round(interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 0.0}, {3, 1.0}, {5, 1.0}, {7, 2.0}, {10, 3.0}
        }));
    }

    private static double forgingFailureReduction(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.00}, {1, 0.03}, {3, 0.10}, {5, 0.15}, {7, 0.22}, {10, 0.30}
        });
    }

    private static double interpolate(int lv, double[][] points) {
        if (points == null || points.length == 0) return 0.0;
        if (lv <= points[0][0]) return points[0][1];
        for (int i = 0; i < points.length - 1; i++) {
            double l0 = points[i][0];
            double v0 = points[i][1];
            double l1 = points[i + 1][0];
            double v1 = points[i + 1][1];
            if (lv <= l1) {
                double t = (lv - l0) / (l1 - l0);
                return v0 + (v1 - v0) * t;
            }
        }
        return points[points.length - 1][1];
    }

    private static String formatPercent1(double value) {
        return String.format(Locale.ROOT, "%.1f", value);
    }

    private static String formatPercent2(double value) {
        return String.format(Locale.ROOT, "%.2f", value);
    }

    private static String formatInt(double value) {
        return Integer.toString((int) Math.round(value));
    }

    private void switchBodyLayer(BodyInspectComponent.Layer layer) {
        if (bodyInspect == null) return;
        bodyInspect.setActiveLayer(layer);
        boolean isPhys = layer == BodyInspectComponent.Layer.PHYSICAL;
        if (physicalLayerLabel != null) {
            physicalLayerLabel.color(Color.ofArgb(isPhys ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
        }
        if (meridianLayerLabel != null) {
            meridianLayerLabel.color(Color.ofArgb(isPhys ? TAB_INACTIVE_COLOR : TAB_ACTIVE_COLOR));
        }
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

        hydrateSkillBarFromStore();

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

    void setBodyInspectForTests(BodyInspectComponent bodyInspect) {
        this.bodyInspect = bodyInspect;
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
        if (button == 0 && pendingMeridianUse != null && confirmPendingMeridianUse()) {
            return true;
        }

        if (button == 1) {
            if (pillContextMenu != null) {
                int actionIdx = pillMenuActionIndexAt(mouseX, mouseY);
                if (actionIdx >= 0) {
                    triggerPillMenuAction(pillContextMenu.actions().get(actionIdx).kind());
                } else {
                    pillContextMenu = null;
                    pendingMeridianUse = null;
                }
                return true;
            }

            if (weaponContextMenu != null) {
                int actionIdx = weaponMenuActionIndexAt(mouseX, mouseY);
                if (actionIdx >= 0) {
                    triggerWeaponMenuAction(weaponContextMenu.actions().get(actionIdx).kind());
                } else {
                    weaponContextMenu = null;
                }
                return true;
            }

            if (activeTab == 0) {
                var eq = equipPanel.slotAtScreen(mouseX, mouseY);
                if (eq != null && eq.item() != null && openWeaponContextMenu(eq.slotType(), eq.item(), (int) mouseX, (int) mouseY)) {
                    return true;
                }
            }

            BackpackGridPanel grid = activeGrid();
            if (grid.containsPoint(mouseX, mouseY)) {
                var pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = grid.itemAt(pos.row(), pos.col());
                    if (item != null && openPillContextMenu(item, (int) mouseX, (int) mouseY)) {
                        return true;
                    }
                }
            }

            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && openPillContextMenu(hotbarItems[hIdx], (int) mouseX, (int) mouseY)) {
                return true;
            }

            if (pendingMeridianUse != null) {
                pendingMeridianUse = null;
                return true;
            }
        }

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
            if (button == 0 && activeTab == 3 && hIdx >= 0 && combatTrainingPanel != null
                    && combatTrainingPanel.selectedTechnique() != null) {
                if (combatTrainingPanel.bindSelectedTechniqueToSlot(hIdx)) {
                    hydrateSkillBarFromStore();
                    return true;
                }
            }
            if (button == 1 && activeTab == 3 && hIdx >= 0 && SkillBarStore.snapshot().slot(hIdx) != null) {
                if (combatTrainingPanel != null && combatTrainingPanel.clearSkillSlot(hIdx)) {
                    hydrateSkillBarFromStore();
                    return true;
                }
            }
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

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (keyCode == GLFW.GLFW_KEY_Q && activeTab == 0 && !dragState.isDragging()) {
            var eq = equipPanel.slotAtScreen(mouseX(), mouseY());
            if (eq != null && eq.item() != null && InventoryEquipRules.isWeapon(eq.item())) {
                if (dispatchDropWeaponFromEquip(eq.slotType(), eq.item())) {
                    eq.clearItem();
                    clearAllHighlights();
                    return true;
                }
            }
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    // ==================== Drag ====================

    private void attemptDrop(double mouseX, double mouseY) {
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) { dragState.cancel(); clearAllHighlights(); return; }

        // Capture source before drop() resets dragState; needed for the C2S move intent.
        com.bong.client.network.ClientRequestProtocol.InvLocation fromLoc = snapshotSourceLocation();

        // Discard
        if (isOverDiscard(mouseX, mouseY)) {
            if (dispatchDiscardIntent(dragged, fromLoc)) {
                dragState.drop();
            } else {
                returnDragToSource();
            }
            clearAllHighlights();
            return;
        }

        if (activeTab == 2 && isOverSkillScrollDropZone(mouseX, mouseY)) {
            if (tryLearnSkillScroll(dragged)) {
                dragState.drop();
            } else {
                returnDragToSource();
            }
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
                if (!isEquipSlotDropValid(dragged, eq.slotType())) {
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
        if (hIdx >= 0 && InventoryEquipRules.canPlaceIntoHotbar(dragged)) {
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
        if (qIdx >= 0 && InventoryEquipRules.canPlaceIntoQuickUse(dragged)) {
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

    boolean dispatchApplyPillSelf(InventoryItem item) {
        if (item == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchApplyPillSelf skipped: item is null");
            return false;
        }
        if (item.instanceId() == 0L) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchApplyPillSelf skipped: item {} has instanceId=0",
                item.itemId());
            return false;
        }
        if (!"guyuan_pill".equals(item.itemId()) && !"huiyuan_pill_forbidden".equals(item.itemId())) {
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchApplyPillSelf instance={} item={}",
            item.instanceId(), item.itemId());
        com.bong.client.network.ClientRequestSender.sendApplyPillSelf(item.instanceId());
        return true;
    }

    boolean dispatchApplyPillMeridian(InventoryItem item) {
        if (item == null || bodyInspect == null) {
            return false;
        }
        MeridianChannel selected = bodyInspect.selectedChannel();
        if (selected == null) {
            return false;
        }
        return dispatchApplyPillMeridianToChannel(item, selected);
    }

    boolean dispatchApplyPillMeridianToChannel(InventoryItem item, MeridianChannel target) {
        if (item == null || target == null) {
            return false;
        }
        if (item.instanceId() == 0L) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchApplyPillMeridian skipped: item {} has instanceId=0",
                item.itemId());
            return false;
        }
        if (!"ningmai_powder".equals(item.itemId())) {
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchApplyPillMeridian instance={} item={} meridian={}",
            item.instanceId(), item.itemId(), target);
        com.bong.client.network.ClientRequestSender.sendApplyPill(
            item.instanceId(),
            new com.bong.client.network.ClientRequestProtocol.MeridianTarget(
                com.bong.client.network.ClientRequestProtocol.toMeridianId(target)
            )
        );
        return true;
    }

    List<PillMenuAction> availablePillMenuActions(InventoryItem item) {
        List<PillMenuAction> actions = new ArrayList<>();
        if (item == null || item.instanceId() == 0L) {
            return actions;
        }
        if ("guyuan_pill".equals(item.itemId()) || "huiyuan_pill_forbidden".equals(item.itemId())) {
            actions.add(new PillMenuAction("服用", ActionKind.SELF_USE));
        }
        if ("ningmai_powder".equals(item.itemId())) {
            actions.add(new PillMenuAction("外敷（选经脉）", ActionKind.MERIDIAN_TARGET));
        }
        if (forgeStationTier(item) > 0) {
            actions.add(new PillMenuAction("放置炼器砧", ActionKind.PLACE_FORGE_STATION));
        }
        if (isSpiritNicheStone(item)) {
            actions.add(new PillMenuAction("放置灵龛", ActionKind.PLACE_SPIRIT_NICHE));
        }
        return actions;
    }

    boolean openPillContextMenu(InventoryItem item, int x, int y) {
        List<PillMenuAction> actions = availablePillMenuActions(item);
        if (actions.isEmpty()) {
            pillContextMenu = null;
            return false;
        }
        pillContextMenu = new PillContextMenuState(item, x, y, List.copyOf(actions));
        pendingMeridianUse = null;
        return true;
    }

    boolean hasOpenPillContextMenu() {
        return pillContextMenu != null;
    }

    boolean hasPendingMeridianUse() {
        return pendingMeridianUse != null;
    }

    void triggerPillMenuAction(ActionKind kind) {
        if (pillContextMenu == null || kind == null) {
            return;
        }
        InventoryItem item = pillContextMenu.item();
        pillContextMenu = null;
        switch (kind) {
            case SELF_USE -> {
                pendingMeridianUse = null;
                dispatchApplyPillSelf(item);
            }
            case MERIDIAN_TARGET -> {
                if (tabLabels[0] != null && tabLabels[1] != null) {
                    switchTab(1);
                } else {
                    activeTab = 1;
                }
                if (bodyInspect != null) {
                    switchBodyLayer(BodyInspectComponent.Layer.MERIDIAN);
                }
                pendingMeridianUse = new PendingMeridianUse(item);
            }
            case PLACE_FORGE_STATION -> {
                pendingMeridianUse = null;
                dispatchPlaceForgeStation(item);
            }
            case PLACE_SPIRIT_NICHE -> {
                pendingMeridianUse = null;
                dispatchPlaceSpiritNiche(item);
            }
        }
    }

    boolean dispatchPlaceSpiritNiche(InventoryItem item) {
        BlockPos pos = targetPlacementPos();
        return dispatchPlaceSpiritNicheAt(item, pos.getX(), pos.getY(), pos.getZ());
    }

    boolean dispatchPlaceSpiritNicheAt(InventoryItem item, int x, int y, int z) {
        if (item == null || item.instanceId() == 0L || !isSpiritNicheStone(item)) {
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchPlaceSpiritNiche instance={} item={} pos=[{},{},{}]",
            item.instanceId(), item.itemId(), x, y, z);
        com.bong.client.network.ClientRequestSender.sendSpiritNichePlace(
            x,
            y,
            z,
            item.instanceId()
        );
        return true;
    }

    boolean dispatchPlaceForgeStation(InventoryItem item) {
        BlockPos pos = targetPlacementPos();
        return dispatchPlaceForgeStationAt(item, pos.getX(), pos.getY(), pos.getZ());
    }

    boolean dispatchPlaceForgeStationAt(InventoryItem item, int x, int y, int z) {
        if (item == null || item.instanceId() == 0L) {
            return false;
        }
        int tier = forgeStationTier(item);
        if (tier <= 0) {
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchPlaceForgeStation instance={} item={} tier={} pos=[{},{},{}]",
            item.instanceId(), item.itemId(), tier, x, y, z);
        com.bong.client.network.ClientRequestSender.sendForgeStationPlace(
            x,
            y,
            z,
            item.instanceId(),
            tier
        );
        return true;
    }

    static int forgeStationTier(InventoryItem item) {
        if (item == null || item.itemId() == null) {
            return 0;
        }
        return switch (item.itemId()) {
            case "fan_iron_anvil" -> 1;
            case "ling_iron_anvil" -> 2;
            case "xuan_iron_anvil" -> 3;
            case "dao_anvil" -> 4;
            default -> 0;
        };
    }

    static boolean isSpiritNicheStone(InventoryItem item) {
        return item != null && "spirit_niche_stone".equals(item.itemId());
    }

    private static BlockPos targetPlacementPos() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.crosshairTarget instanceof BlockHitResult hit
            && hit.getType() == HitResult.Type.BLOCK) {
            return hit.getBlockPos().offset(hit.getSide());
        }
        if (client.player != null) {
            return new BlockPos(
                (int) Math.floor(client.player.getX()),
                (int) Math.floor(client.player.getY()),
                (int) Math.floor(client.player.getZ())
            );
        }
        return new BlockPos(0, 64, 0);
    }

    boolean confirmPendingMeridianUse() {
        if (pendingMeridianUse == null || bodyInspect == null) {
            return false;
        }
        MeridianChannel target = bodyInspect.focusedChannel();
        if (!dispatchApplyPillMeridianToChannel(pendingMeridianUse.item(), target)) {
            return false;
        }
        pendingMeridianUse = null;
        return true;
    }

    void dispatchMoveIntent(
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

    boolean dispatchDiscardIntent(
        InventoryItem item,
        com.bong.client.network.ClientRequestProtocol.InvLocation from
    ) {
        if (item == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchDiscardIntent skipped: item is null");
            return false;
        }
        if (from == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchDiscardIntent skipped: from={} item={}",
                from, item.itemId());
            return false;
        }
        if (item.instanceId() == 0L) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchDiscardIntent skipped: item {} has instanceId=0",
                item.itemId());
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchDiscardIntent instance={} from={} item={}",
            item.instanceId(), from, item.itemId());
        com.bong.client.network.ClientRequestSender.sendInventoryDiscardItem(item.instanceId(), from);
        return true;
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
        if (InventoryEquipRules.canPlaceIntoHotbar(item)) {
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

        if (activeTab == 2 && skillScrollDropZone != null && isOverSkillScrollDropZone(mouseX, mouseY)) {
            setSkillScrollDropZoneState(skillScrollDropState(dragged));
        }

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
                boolean valid = isEquipSlotDropValid(dragged, eq.slotType());
                eq.setHighlightState(valid
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
            boolean valid = InventoryEquipRules.canPlaceIntoHotbar(dragged);
            hotbarSlots[hIdx].setHighlightState(
                valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
        }

        int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
        if (qIdx >= 0) {
            boolean valid = InventoryEquipRules.canPlaceIntoQuickUse(dragged);
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
        setSkillScrollDropZoneState(SkillScrollDropState.IDLE);
    }

    private boolean isOverSkillScrollDropZone(double sx, double sy) {
        if (skillScrollDropZone == null) return false;
        return sx >= skillScrollDropZone.x() && sx < skillScrollDropZone.x() + skillScrollDropZone.width()
            && sy >= skillScrollDropZone.y() && sy < skillScrollDropZone.y() + skillScrollDropZone.height();
    }

    private enum SkillScrollDropState {
        IDLE, VALID, INVALID
    }

    private SkillScrollDropState skillScrollDropState(InventoryItem item) {
        if (item == null) return SkillScrollDropState.IDLE;
        if (item.isSkillScroll() && isKnownSkillScroll(item) && !isConsumedSkillScroll(item)) {
            return SkillScrollDropState.VALID;
        }
        return SkillScrollDropState.INVALID;
    }

    private void setSkillScrollDropZoneState(SkillScrollDropState state) {
        if (skillScrollDropZone == null || skillScrollDropTitle == null || skillScrollDropHint == null) return;
        switch (state) {
            case VALID -> {
                skillScrollDropZone.surface(Surface.flat(0xFF1A2418).and(Surface.outline(0xFF5C8A50)));
                skillScrollDropTitle.text(Text.literal("技能残卷"));
                skillScrollDropHint.text(Text.literal("拖入即可顿悟"));
                skillScrollDropHint.color(Color.ofArgb(0xFF88CC88));
            }
            case INVALID -> {
                skillScrollDropZone.surface(Surface.flat(0xFF241616).and(Surface.outline(0xFFAA5050)));
                skillScrollDropTitle.text(Text.literal("不可投入"));
                skillScrollDropHint.text(Text.literal(skillScrollDropFeedback));
                skillScrollDropHint.color(Color.ofArgb(0xFFCC8888));
            }
            case IDLE -> {
                skillScrollDropZone.surface(Surface.flat(0xFF201A14).and(Surface.outline(0xFF6A5030)));
                skillScrollDropTitle.text(Text.literal("技能残卷"));
                skillScrollDropHint.text(Text.literal(skillScrollDropFeedback));
                skillScrollDropHint.color(Color.ofArgb(0xFF888888));
            }
        }
    }

    boolean tryLearnSkillScroll(InventoryItem item) {
        if (item == null || item.instanceId() == 0L) {
            skillScrollDropFeedback = "残卷无效";
            return false;
        }
        if (!item.isSkillScroll()) {
            skillScrollDropFeedback = "此物非 skill，不可入";
            return false;
        }
        if (!isKnownSkillScroll(item)) {
            skillScrollDropFeedback = "不识此技，暂不能悟";
            return false;
        }
        if (isConsumedSkillScroll(item)) {
            skillScrollDropFeedback = "此卷已悟";
            return false;
        }
        skillScrollDropFeedback = "已送出顿悟请求";
        com.bong.client.network.ClientRequestSender.sendLearnSkillScroll(item.instanceId());
        return true;
    }

    private boolean isKnownSkillScroll(InventoryItem item) {
        return com.bong.client.skill.SkillId.fromWire(item.scrollSkillId()) != null;
    }

    private boolean isConsumedSkillScroll(InventoryItem item) {
        return com.bong.client.skill.SkillSetStore.snapshot().hasConsumedScroll(item.itemId());
    }

    String debugSkillScrollDropFeedback() {
        return skillScrollDropFeedback;
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

    private boolean isEquipSlotDropValid(InventoryItem item, EquipSlotType targetSlot) {
        if (!isEquipSlotUsable(targetSlot)) return false;
        EquipSlotType sourceSlot = dragState.sourceKind() == DragState.SourceKind.EQUIP
            ? dragState.sourceEquipSlot()
            : null;
        return InventoryEquipRules.canEquip(
            item,
            targetSlot,
            sourceSlot,
            equippedStateForValidation(item, sourceSlot)
        );
    }

    private EnumMap<EquipSlotType, InventoryItem> equippedStateForValidation(
        InventoryItem dragged,
        EquipSlotType sourceSlot
    ) {
        EnumMap<EquipSlotType, InventoryItem> equipped = new EnumMap<>(EquipSlotType.class);
        for (EquipSlotType type : EquipSlotType.values()) {
            var slot = equipPanel.slotFor(type);
            if (slot == null || slot.item() == null || slot.item().isEmpty()) continue;
            equipped.put(type, slot.item());
        }
        if (sourceSlot != null && dragged != null && !dragged.isEmpty()) {
            equipped.put(sourceSlot, dragged);
        }
        return equipped;
    }

    // ==================== Quick operations ====================

    private void quickEquipFromGrid(InventoryItem item) {
        if (!InventoryEquipRules.isWeapon(item)
            && !InventoryEquipRules.isHoe(item)
            && !InventoryEquipRules.isTool(item)
            && !InventoryEquipRules.isTreasure(item)) {
            return;
        }
        var anchor = activeGrid().anchorOf(item);
        if (anchor == null) return;

        var equipped = equippedStateForValidation(null, null);
        EquipSlotType targetSlot = InventoryEquipRules.isTreasure(item)
            ? firstEmptyTreasureBeltSlot(equipped)
            : InventoryEquipRules.preferredWeaponQuickEquipSlot(
                item,
                equipped,
                this::isEquipSlotUsable
            );
        if (targetSlot == null) return;

        activeGrid().remove(item);
        equipPanel.slotFor(targetSlot).setItem(item);
        dispatchMoveIntent(
            item,
            new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                activeGrid().containerId(),
                anchor.row(),
                anchor.col()
            ),
            new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                targetSlot.name().toLowerCase()
            )
        );
    }

    private void quickUnequipToGrid(EquipSlotType slotType, InventoryItem item) {
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            equipPanel.slotFor(slotType).clearItem();
            activeGrid().place(item, pos.row(), pos.col());
            dispatchMoveIntent(
                item,
                new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                    slotType.name().toLowerCase()
                ),
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    activeGrid().containerId(),
                    pos.row(),
                    pos.col()
                )
            );
        }
    }

    private EquipSlotType firstEmptyTreasureBeltSlot(EnumMap<EquipSlotType, InventoryItem> equipped) {
        EquipSlotType[] order = {
            EquipSlotType.TREASURE_BELT_0,
            EquipSlotType.TREASURE_BELT_1,
            EquipSlotType.TREASURE_BELT_2,
            EquipSlotType.TREASURE_BELT_3
        };
        for (EquipSlotType slot : order) {
            InventoryItem item = equipped.get(slot);
            if (item == null || item.isEmpty()) return slot;
        }
        return null;
    }

    private void quickMoveHotbarToGrid(int index) {
        InventoryItem item = hotbarItems[index];
        if (item == null) return;
        var pos = activeGrid().findFreeSpace(item);
        if (pos != null) {
            hotbarItems[index] = null;
            hotbarSlots[index].clearItem();
            activeGrid().place(item, pos.row(), pos.col());
            dispatchMoveIntent(
                item,
                new com.bong.client.network.ClientRequestProtocol.HotbarLoc(index),
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    activeGrid().containerId(),
                    pos.row(),
                    pos.col()
                )
            );
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
        drawPillMenuOverlay(context, mouseX, mouseY);
        drawWeaponMenuOverlay(context, mouseX, mouseY);
        drawPendingMeridianPrompt(context);

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

            Identifier tex = GridSlotComponent.textureIdForItem(item);
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
        Identifier tex = GridSlotComponent.textureIdForItem(item);
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

    private int pillMenuHeight() {
        return pillContextMenu == null ? 0 : PILL_MENU_PADDING * 2 + pillContextMenu.actions().size() * PILL_MENU_ROW_HEIGHT;
    }

    private int pillMenuActionIndexAt(double mouseX, double mouseY) {
        if (pillContextMenu == null) return -1;
        int left = pillContextMenu.x();
        int top = pillContextMenu.y();
        int height = pillMenuHeight();
        if (mouseX < left || mouseX >= left + PILL_MENU_WIDTH || mouseY < top || mouseY >= top + height) {
            return -1;
        }
        int row = ((int) mouseY - top - PILL_MENU_PADDING) / PILL_MENU_ROW_HEIGHT;
        return row >= 0 && row < pillContextMenu.actions().size() ? row : -1;
    }

    private int weaponMenuHeight() {
        return weaponContextMenu == null ? 0 : PILL_MENU_PADDING * 2 + weaponContextMenu.actions().size() * PILL_MENU_ROW_HEIGHT;
    }

    private int weaponMenuActionIndexAt(double mouseX, double mouseY) {
        if (weaponContextMenu == null) return -1;
        int left = weaponContextMenu.x();
        int top = weaponContextMenu.y();
        int height = weaponMenuHeight();
        if (mouseX < left || mouseX >= left + PILL_MENU_WIDTH || mouseY < top || mouseY >= top + height) {
            return -1;
        }
        int row = ((int) mouseY - top - PILL_MENU_PADDING) / PILL_MENU_ROW_HEIGHT;
        return row >= 0 && row < weaponContextMenu.actions().size() ? row : -1;
    }

    private void drawPillMenuOverlay(DrawContext context, int mouseX, int mouseY) {
        if (pillContextMenu == null) return;
        int left = pillContextMenu.x();
        int top = pillContextMenu.y();
        int height = pillMenuHeight();
        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 450);
        context.fill(left, top, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BG);
        context.fill(left, top, left + PILL_MENU_WIDTH, top + 1, PILL_MENU_BORDER);
        context.fill(left, top + height - 1, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BORDER);
        context.fill(left, top, left + 1, top + height, PILL_MENU_BORDER);
        context.fill(left + PILL_MENU_WIDTH - 1, top, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BORDER);
        int hovered = pillMenuActionIndexAt(mouseX, mouseY);
        var textRenderer = MinecraftClient.getInstance().textRenderer;
        for (int i = 0; i < pillContextMenu.actions().size(); i++) {
            int rowTop = top + PILL_MENU_PADDING + i * PILL_MENU_ROW_HEIGHT;
            if (i == hovered) {
                context.fill(left + 1, rowTop, left + PILL_MENU_WIDTH - 1, rowTop + PILL_MENU_ROW_HEIGHT, PILL_MENU_HOVER);
            }
            context.drawTextWithShadow(
                textRenderer,
                Text.literal(pillContextMenu.actions().get(i).label()),
                left + 6,
                rowTop + 4,
                PILL_MENU_TEXT
            );
        }
        matrices.pop();
    }

    private void drawWeaponMenuOverlay(DrawContext context, int mouseX, int mouseY) {
        if (weaponContextMenu == null) return;
        int left = weaponContextMenu.x();
        int top = weaponContextMenu.y();
        int height = weaponMenuHeight();
        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 450);
        context.fill(left, top, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BG);
        context.fill(left, top, left + PILL_MENU_WIDTH, top + 1, PILL_MENU_BORDER);
        context.fill(left, top + height - 1, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BORDER);
        context.fill(left, top, left + 1, top + height, PILL_MENU_BORDER);
        context.fill(left + PILL_MENU_WIDTH - 1, top, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BORDER);
        int hovered = weaponMenuActionIndexAt(mouseX, mouseY);
        var textRenderer = MinecraftClient.getInstance().textRenderer;
        for (int i = 0; i < weaponContextMenu.actions().size(); i++) {
            int rowTop = top + PILL_MENU_PADDING + i * PILL_MENU_ROW_HEIGHT;
            if (i == hovered) {
                context.fill(left + 1, rowTop, left + PILL_MENU_WIDTH - 1, rowTop + PILL_MENU_ROW_HEIGHT, PILL_MENU_HOVER);
            }
            context.drawTextWithShadow(
                textRenderer,
                Text.literal(weaponContextMenu.actions().get(i).label()),
                left + 6,
                rowTop + 4,
                PILL_MENU_TEXT
            );
        }
        matrices.pop();
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

    private void drawPendingMeridianPrompt(DrawContext context) {
        if (pendingMeridianUse == null || bodyInspect == null) return;
        MeridianChannel focus = bodyInspect.focusedChannel();
        String text = focus == null
            ? "外敷：请先选择经脉（左键确认 / 右键取消）"
            : "外敷：左键确认至 " + focus.name();
        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 430);
        context.drawTextWithShadow(
            MinecraftClient.getInstance().textRenderer,
            Text.literal(text),
            12,
            26,
            PILL_TARGET_HINT
        );
        matrices.pop();
    }

    private boolean openWeaponContextMenu(EquipSlotType slotType, InventoryItem item, int x, int y) {
        if (slotType == null || item == null || item.isEmpty() || !InventoryEquipRules.isWeapon(item)) {
            weaponContextMenu = null;
            return false;
        }
        weaponContextMenu = new WeaponContextMenuState(
            item,
            slotType,
            x,
            y,
            List.of(
                new WeaponMenuAction("修复", WeaponActionKind.REPAIR),
                new WeaponMenuAction("丢弃", WeaponActionKind.DROP)
            )
        );
        pillContextMenu = null;
        pendingMeridianUse = null;
        return true;
    }

    private void triggerWeaponMenuAction(WeaponActionKind kind) {
        if (weaponContextMenu == null) return;
        WeaponContextMenuState menu = weaponContextMenu;
        weaponContextMenu = null;
        switch (kind) {
            case REPAIR -> openRepairScreen(menu.item());
            case DROP -> {
                if (dispatchDropWeaponFromEquip(menu.slotType(), menu.item())) {
                    var slot = equipPanel.slotFor(menu.slotType());
                    if (slot != null) slot.clearItem();
                }
            }
        }
    }

    private boolean dispatchDropWeaponFromEquip(EquipSlotType slotType, InventoryItem item) {
        if (slotType == null || item == null || item.instanceId() == 0L || !InventoryEquipRules.isWeapon(item)) {
            return false;
        }
        com.bong.client.network.ClientRequestSender.sendDropWeapon(
            item.instanceId(),
            new com.bong.client.network.ClientRequestProtocol.EquipLoc(slotType.name().toLowerCase())
        );
        return true;
    }

    private void openRepairScreen(InventoryItem item) {
        if (item == null || item.instanceId() == 0L) return;
        MinecraftClient client = MinecraftClient.getInstance();
        int sx = 0;
        int sy = 64;
        int sz = 0;
        if (client.player != null) {
            sx = (int) Math.floor(client.player.getX());
            sy = (int) Math.floor(client.player.getY());
            sz = (int) Math.floor(client.player.getZ());
        }
        client.setScreen(new com.bong.client.combat.screen.RepairScreen(
            item.displayName(),
            (float) item.durability(),
            item.instanceId(),
            sx,
            sy,
            sz
        ));
    }

    private double mouseX() {
        MinecraftClient client = MinecraftClient.getInstance();
        return client.mouse.getX() * width / (double) client.getWindow().getWidth();
    }

    private double mouseY() {
        MinecraftClient client = MinecraftClient.getInstance();
        return client.mouse.getY() * height / (double) client.getWindow().getHeight();
    }
}
