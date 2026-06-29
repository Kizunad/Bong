package com.bong.client.inventory;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.inspect.TechniqueDragDecision;
import com.bong.client.block.BlockVanillaIconMap;
import com.bong.client.combat.inspect.StatusPanelExtension;
import com.bong.client.craft.CraftScreen;
import com.bong.client.combat.store.AscensionQuotaStore;
import com.bong.client.cultivation.ColorKind;
import com.bong.client.cultivation.QiColorVectorHud;
import com.bong.client.cultivation.QiColorObservedState;
import com.bong.client.cultivation.QiColorObservedStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.hud.SwordBondHudState;
import com.bong.client.hud.SwordBondHudStateStore;
import com.bong.client.inspect.ItemInspectLongPressTracker;
import com.bong.client.inspect.ItemInspectScreen;
import com.bong.client.inventory.component.*;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.DragState;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.util.RealmLabel;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.processing.state.FreshnessStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvents;
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
    private static final int TAB_EQUIP = 0;
    private static final int TAB_CULTIVATION = 1;
    private static final int TAB_SKILL = 2;
    private static final int TAB_TECHNIQUES = 3;
    private static final int TAB_CRAFT = 4;
    private static final String[] TAB_NAMES = {"装备", "修仙", "技艺", "功法", "手搓"};
    private static final int ACTION_TOAST_OK = 0xFFA8E6CF;
    private static final int ACTION_TOAST_WARN = 0xFFFFAA55;
    private static final long ACTION_TOAST_MS = 2_200L;

    private InventoryModel model;
    private final DragState dragState = new DragState();
    // 功法拖拽落槽 —— 与物品 dragState 平行的独立轻量态（功法非 InventoryItem，不复用 DragState）。
    // 非 null 即「正在拖某条功法」；按下功法行起手、松手落 1-9 槽绑定，渲染期画跟手 ghost。
    private com.bong.client.combat.inspect.TechniquesListPanel.Technique draggedTechnique;
    private boolean draggedTechniqueLocked;
    // 拖拽来源槽：>=0 表示从已绑定的 1-9 槽拖出（松手落槽外=解绑）；-1 表示从功法列表拖出。
    private int draggedFromSlot = -1;
    private double techniqueDragX;
    private double techniqueDragY;
    private final ItemInspectLongPressTracker itemInspectLongPress = new ItemInspectLongPressTracker();
    /** Screen 存活期间持有的 InventoryStateStore 订阅，close 时解绑避免泄漏。 */
    private Consumer<InventoryModel> inventoryListener;

    // --- Container grids (driven by model.containers()) ---
    private BackpackGridPanel[] containerGrids;
    private FlowLayout[] containerWrappers;
    private LabelComponent[] containerLabels;
    private int containerCount;
    // plan-layered-equip-v1 P4（决议 #13/#18）：行囊面板 + containerCount 哨兵已删除。
    // -1 = 尚未激活任一页（初始）；之后 activeContainer 落 0..containerCount-1（body_pocket=index 0，默认激活）。
    private int activeContainer = -1;
    // 容器 tab 列表 = model.containers()，body_pocket 置 index 0（决议 #18）。
    // switchContainer 用这份列表索引 containerLabels[]/containerGrids[]。
    private java.util.List<InventoryModel.ContainerDef> filteredContainerDefs = java.util.List.of();
    // plan-tarkov-backpack-v1 后续修复：容器 tab 栏 + grids 的 holder。
    // snapshot 结构变化（卸/穿背包→容器增删）时 clearChildren + rebuildContainerSection 重建，
    // 无需重开界面（populateFromModel 只重填已有格子，不会增删 tab）。
    private FlowLayout containerSection;

    private EquipmentPanel equipPanel;
    private StatusBarsPanel statusBars;
    private ItemTooltipPanel tooltipPanel;
    private BottomInfoBar bottomBar;

    // Tabs (left panel)
    private int activeTab = TAB_EQUIP;
        // plan-skill-v1 §5.1 第三个 tab "技艺"
    private final LabelComponent[] tabLabels = new LabelComponent[TAB_NAMES.length];
    private FlowLayout equipTabContent;
    private FlowLayout cultivationTabContent;
    private FlowLayout skillTabContent;
    private com.bong.client.combat.inspect.TechniquesTabPanel techniquesTabPanel;
    private FlowLayout techniquesTabContent;
    private FlowLayout craftTabContent;
    private FlowLayout skillScrollDropZone;
    private LabelComponent skillScrollDropTitle;
    private LabelComponent skillScrollDropHint;
    private String skillScrollDropFeedback = "仅 skill 残卷可悟";
    // plan-cross-system-patch-v1 P1：按 SkillId.values() 展示所有技艺。
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

    // Loot panel (supply coffin — mounted into outerRow when session active)
    private FlowLayout outerRow;
    private LootContainerPanel lootPanel;
    private FlowLayout lootPanelLayout;
    private LootContainerStateStore.Listener lootStoreListener;

    // plan-tarkov-floating-windows — 套包内含物悬浮窗口系统（双击/右键背包件打开，多开 + 可拖动 +
    // z-order）。挂在 owo root（absolute 定位），不再占容器 tab，也不挂 outerRow（脱离 vertical flow）。
    private final PackWindowManager packWindows = new PackWindowManager();
    // build() 接收的 owo root FlowLayout（悬浮窗挂载点 + bringToFront 重挂目标）。
    private FlowLayout root;
    // 双击计时（owo 无 clickCount，在 Screen 级 mouseClicked() 手算）：同槽两击 ≤ 窗口 → 双击。
    private static final long DOUBLE_CLICK_WINDOW_MS = 400L;
    private long lastEquipClickTimeMs = 0L;
    private EquipSlotType lastEquipClickSlot = null;
    // fix/tarkov-nest-persistence — grid / hotbar 格双击开包（背包卸到身上任意位置后仍可开）。
    // 用 instanceId（比坐标稳）区分「同一物品的两连击」。
    private long lastGridClickTimeMs = 0L;
    private long lastGridClickInstanceId = -1L;
    private long lastHotbarClickTimeMs = 0L;
    private long lastHotbarClickInstanceId = -1L;

    // Block picker panel (plan-worldgen-v4 P5 §8.1#5 — dev-only 方块审阅浮窗)
    private BlockPickerPanel blockPickerPanel;
    private FlowLayout blockPickerPanelLayout;

    // Body inspect (cultivation tab) — dual-layer: physical + meridian
    private BodyInspectComponent bodyInspect;
    private QiColorVectorHud qiColorVectorHud;
    private LabelComponent physicalLayerLabel;
    private LabelComponent meridianLayerLabel;
    private FlowLayout meridianFilterBar;
    private io.wispforest.owo.ui.container.ScrollContainer<?> cultivationActionScroll;
    /** Screen 存活期间持有的 MeridianStateStore 订阅，close 时移除避免泄漏。 */
    private Consumer<MeridianBody> meridianBodyListener;
    private Consumer<QiColorObservedState> qiColorObservedListener;
    private Consumer<AscensionQuotaStore.State> ascensionQuotaListener;
    private final LabelComponent[] filterLabels = new LabelComponent[4];

    record PillMenuAction(String label, ActionKind kind) {}
    enum ActionKind { SELF_USE, MERIDIAN_TARGET, PLACE_FORGE_STATION, PLACE_SPIRIT_NICHE, REPAIR_SPIRIT_NICHE, TECHNIQUE_SCROLL_USE }
    record PillContextMenuState(InventoryItem item, int x, int y, List<PillMenuAction> actions) {}
    record PendingMeridianUse(InventoryItem item) {}
    record WeaponMenuAction(String label, WeaponActionKind kind) {}
    enum WeaponActionKind { REPAIR, DROP }
    record WeaponContextMenuState(InventoryItem item, EquipSlotType slotType, int x, int y, List<WeaponMenuAction> actions) {}
    record SkillBarMenuAction(String label, int slot) {}
    record SkillBarContextMenuState(InventoryItem item, int x, int y, List<SkillBarMenuAction> actions) {}

    private PillContextMenuState pillContextMenu;
    private PendingMeridianUse pendingMeridianUse;
    private WeaponContextMenuState weaponContextMenu;
    private SkillBarContextMenuState skillBarContextMenu;

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
        if (qiColorObservedListener != null) {
            QiColorObservedStore.removeListener(qiColorObservedListener);
            qiColorObservedListener = null;
        }
        if (ascensionQuotaListener != null) {
            AscensionQuotaStore.removeListener(ascensionQuotaListener);
            ascensionQuotaListener = null;
        }
        if (inventoryListener != null) {
            InventoryStateStore.removeListener(inventoryListener);
            inventoryListener = null;
        }
        if (skillListener != null) {
            com.bong.client.skill.SkillSetStore.removeListener(skillListener);
            skillListener = null;
        }
        if (techniquesTabPanel != null) {
            techniquesTabPanel.close();
            techniquesTabPanel = null;
        }
        // Loot panel cleanup — send close to server if still active
        if (lootPanel != null && !lootPanel.isClosed()) {
            lootPanel.sendClose();
        }
        unmountLootPanel();
        if (lootStoreListener != null) {
            LootContainerStateStore.removeListener(lootStoreListener);
            lootStoreListener = null;
        }
        // plan-tarkov-floating-windows —— 全关悬浮窗（dispose 内含面板的 InventoryStateStore 订阅）。
        packWindows.closeAll();
        unmountBlockPickerPanel();
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
        outerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
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
        for (int i = 0; i < TAB_NAMES.length; i++) {
            final int idx = i;
            var label = Components.label(Text.literal(TAB_NAMES[i]));
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
        // 经脉必须以服务端 cultivation_detail 为准；无快照时宁可显示加载态，不能用 mock 误导目标选择。
        bodyInspect.setMeridianBody(meridianData);

        // Cultivation action bar: [设为目标] [突破] [淬炼·流速] [淬炼·容量]
        // 「突破」常开；其余三项需选中某条经脉 — 灰态表示禁用。
        FlowLayout actionBar = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        actionBar.gap(4);
        actionBar.padding(Insets.of(2, 2, 2, 2));
        actionBar.verticalAlignment(VerticalAlignment.CENTER);
        Object[] setTargetBtn = buildActionButton("设为目标", this::dispatchSetMeridianTarget);
        Object[] breakthroughBtn = buildActionButton("突破",
            this::dispatchBreakthroughRequest);
        Object[] duXuBtn = buildActionButton("渡虚劫", this::dispatchStartDuXuIfEligible);
        Object[] forgeRateBtn = buildActionButton("淬炼·流速", () -> {
            var sel = bodyInspect.selectedChannel();
            if (sel == null) {
                showActionToast("先选中一条经脉", ACTION_TOAST_WARN);
                return;
            }
            com.bong.client.network.ClientRequestSender.sendForgeRequest(
                com.bong.client.network.ClientRequestProtocol.toMeridianId(sel),
                com.bong.client.network.ClientRequestProtocol.ForgeAxis.Rate);
        });
        Object[] forgeCapBtn = buildActionButton("淬炼·容量", () -> {
            var sel = bodyInspect.selectedChannel();
            if (sel == null) {
                showActionToast("先选中一条经脉", ACTION_TOAST_WARN);
                return;
            }
            com.bong.client.network.ClientRequestSender.sendForgeRequest(
                com.bong.client.network.ClientRequestProtocol.toMeridianId(sel),
                com.bong.client.network.ClientRequestProtocol.ForgeAxis.Capacity);
        });
        var setTargetLabel = (LabelComponent) setTargetBtn[1];
        var duXuLabel = (LabelComponent) duXuBtn[1];
        var forgeRateLabel = (LabelComponent) forgeRateBtn[1];
        var forgeCapLabel = (LabelComponent) forgeCapBtn[1];
        actionBar.child((io.wispforest.owo.ui.core.Component) setTargetBtn[0]);
        actionBar.child((io.wispforest.owo.ui.core.Component) breakthroughBtn[0]);
        actionBar.child((io.wispforest.owo.ui.core.Component) duXuBtn[0]);
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
                sb.append("§7境界 §f").append(RealmLabel.displayName(b.realm()));
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
            ColorKind ownColor = b.qiColorMain();
            if (ownColor != null) {
                if (sb.length() > 0) sb.append("  §8·  ");
                sb.append("§7真元 §f").append(ownColor.label());
                if (b.qiColorSecondary() != null) sb.append("/").append(b.qiColorSecondary().label());
                if (b.qiColorChaotic()) sb.append(" §c杂");
                if (b.qiColorHunyuan()) sb.append(" §b混");
            }
            QiColorObservedState observedColor = QiColorObservedStore.snapshot();
            if (observedColor != null && !observedColor.displayText().isEmpty()) {
                if (sb.length() > 0) sb.append("  §8·  ");
                sb.append("§7").append(observedColor.displayText());
            }
            String quotaLine = StatusPanelExtension.ascensionQuotaLine(b.realm());
            if (!quotaLine.isEmpty()) {
                if (sb.length() > 0) sb.append("  §8·  ");
                sb.append("§7").append(quotaLine);
            }
            bodyStatusLabel.text(Text.literal(sb.toString()));
        };
        refreshBodyStatus.run();
        qiColorVectorHud = new QiColorVectorHud();
        qiColorVectorHud.setBody(bodyInspect.meridianBody());
        cultivationTabContent.child(qiColorVectorHud);
        // 网络新快照到达时推到 UI（BodyInspect 内部不会自动感知 MeridianStateStore.replace）。
        // 存成字段以便 removed() 回调里解绑 —— 否则每开一次 InspectScreen 都累积一个悬挂监听。
        meridianBodyListener = body -> MinecraftClient.getInstance().execute(() -> {
            if (body != null) bodyInspect.setMeridianBody(body);
            if (qiColorVectorHud != null) qiColorVectorHud.setBody(body);
            refreshBodyStatus.run();
        });
        MeridianStateStore.addListener(meridianBodyListener);
        qiColorObservedListener = ignored -> MinecraftClient.getInstance().execute(refreshBodyStatus);
        QiColorObservedStore.addListener(qiColorObservedListener);
        ascensionQuotaListener = ignored -> MinecraftClient.getInstance().execute(refreshBodyStatus);
        AscensionQuotaStore.addListener(ascensionQuotaListener);
        bodyInspect.addSelectionListener(ch -> refreshBodyStatus.run());

        // 根据当前选择应用灰态，并订阅变化
        Runnable refreshActionColors = () -> {
            boolean hasSel = bodyInspect.selectedChannel() != null;
            int c = hasSel ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR;
            setTargetLabel.color(Color.ofArgb(c));
            duXuLabel.color(Color.ofArgb(isDuXuEligible(bodyInspect.meridianBody()) ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            forgeRateLabel.color(Color.ofArgb(c));
            forgeCapLabel.color(Color.ofArgb(c));
        };
        refreshActionColors.run();
        bodyInspect.addSelectionListener(ch -> refreshActionColors.run());

        cultivationTabContent.child(bodyInspect);

        // 灵剑信息区块（F.3 §sword-path-complete 契约）
        // 数据在打开面板时读 store 快照（store 由 SwordBondHudStateHandler S2C 写入）。
        // active=false 时整个区块不添加，以免留白影响布局。
        List<String> bondLines = swordBondInfoLines(SwordBondHudStateStore.snapshot());
        if (!bondLines.isEmpty()) {
            FlowLayout bondSection = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
            bondSection.gap(1);
            bondSection.padding(Insets.of(4, 0, 0, 0));
            for (String line : bondLines) {
                LabelComponent bondLbl = Components.label(Text.literal(line));
                bondLbl.color(Color.ofArgb(0xFFCCCCCC));
                bondSection.child(bondLbl);
            }
            cultivationTabContent.child(bondSection);
        }

        leftCol.child(cultivationTabContent);
        cultivationTabContent.positioning(Positioning.absolute(-9999, -9999));

        // Tab 2: 技艺 (plan-skill-v1 §5.1)
        //   现阶段做最小闭环：左列固定三行 + 选中高亮；中列展示当前生效等级、cap 压制与效果说明。
        //   曲线 canvas / 里程碑 / 残卷拖入槽留 P4/P5/P6。
        skillTabContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        skillTabContent.gap(2);
        skillTabContent.padding(Insets.of(2));

        com.bong.client.skill.SkillId[] order = com.bong.client.skill.SkillId.values();
        skillRows = new com.bong.client.skill.SkillRowComponent[order.length];
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

        // Tab 3: 功法（plan-hotbar-modify-v2 §1）
        techniquesTabPanel = new com.bong.client.combat.inspect.TechniquesTabPanel(channels -> {
            if (bodyInspect != null) bodyInspect.setTechniqueMeridianHighlights(channels);
        });
        techniquesTabContent = techniquesTabPanel.component();
        leftCol.child(techniquesTabContent);
        techniquesTabContent.positioning(Positioning.absolute(-9999, -9999));

        // Tab 4: 手搓入口。完整三栏布局在独立 CraftScreen，避免挤进 172px 左栏。
        craftTabContent = buildCraftTabEntryContent();
        leftCol.child(craftTabContent);
        craftTabContent.positioning(Positioning.absolute(-9999, -9999));

        middle.child(leftCol);

        // 经脉详情直接绘制在 body 画布内部（见 BodyInspectComponent.drawMeridianDetailInline）
        // 不再作为独立组件，以免增加列宽/列高

        // -- Right column --
        FlowLayout rightCol = Containers.verticalFlow(Sizing.content(), Sizing.content());
        rightCol.gap(2);

        // Container tabs + grids（plan-tarkov-backpack-v1 后续修复）：放进 containerSection holder，
        // 由 rebuildContainerSection() 从 model.containers() 重建；snapshot 结构变化时（卸/穿背包→
        // 容器增删）listener 整体重建，无需重开界面。
        containerSection = Containers.verticalFlow(Sizing.content(), Sizing.content());
        containerSection.gap(2);
        rebuildContainerSection();
        rightCol.child(containerSection);

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

        // Mount loot panel if a coffin session is already active
        mountLootPanelIfActive();

        // Listen for loot container state changes (open/close)
        lootStoreListener = session -> {
            MinecraftClient.getInstance().execute(() -> {
                if (session instanceof LootContainerStateStore.OpenSession) {
                    mountLootPanelIfActive();
                } else if (session instanceof LootContainerStateStore.Closed) {
                    unmountLootPanel();
                }
            });
        };
        LootContainerStateStore.addListener(lootStoreListener);

        // Mount dev-only block picker panel (隐藏而非灰态：非 dev/creative 完全不挂载)
        mountBlockPickerPanelIfDev();

        root.child(outerRow);
        // plan-tarkov-floating-windows：悬浮窗挂在 root（absolute 定位），不进 outerRow 的 horizontal flow。
        this.root = root;
        packWindows.attach(root);
        populateFromModel();

        // Server 增量到达时（InventoryEventHandler 写入或新 snapshot 落地）刷新 UI。
        // listener 在网络线程触发，UI mutation 必须回主线程。
        inventoryListener = next -> {
            if (next == null) return;
            MinecraftClient.getInstance().execute(() -> {
                // 容器集合变化（卸/穿背包→pack_<id> 增删）时重建 tab 栏 + grids，免重开界面；
                // 仅内含物变化时 populateFromModel 重填已有格子即可。
                boolean structuralChange = containerStructureChanged(next);
                this.model = next;
                if (structuralChange) {
                    rebuildContainerSection();
                }
                // plan-tarkov-floating-windows / fix/tarkov-nest-persistence §C4 —— 悬浮窗不 stale：
                // 背包真离开（丢地/转移走）后其 pack_<id> 从 snapshot 消失，已开的悬浮窗须关闭，否则空壳
                // 窗仍可点 → 产生 stale move_intent。卸到 body_pocket/手持/快捷栏等仍在携带面的情形 pack_<id>
                // 仍在 → 窗口不关（用户可继续看包），这正是预期。
                // 注意：A 准则后 worn pack 会进容器 tab，穿/卸 pack 经 structuralChange→rebuildContainerSection
                // 增删该 tab；但 reconcile 仍必须每个 snapshot 跑（不依赖 structuralChange），否则丢地后窗口不关。
                // 二者是独立通道：tab 增删看 worn 态（owner 在不在 worn 层），悬浮窗开关看 pack_<id> 在不在
                // containers()——worn↔非worn 切换不改变 containers() 成员，故悬浮窗不被误关，互不冲突。
                for (var win : packWindows.ordered()) {
                    boolean stillExists = next.containers().stream()
                            .anyMatch(d -> win.containerId().equals(d.id()));
                    if (!stillExists) {
                        // 若正从该被销毁容器拖拽，先取消拖拽避免 stale from-location。
                        if (dragState.isDragging()
                                && win.containerId().equals(dragState.sourceContainerId())) {
                            dragState.cancel();
                        }
                        packWindows.close(win.containerId());
                    }
                }
                populateFromModel();
            });
        };
        InventoryStateStore.addListener(inventoryListener);

        // plan-layered-equip-v1 P4（决议 #18）：body_pocket（容器 tab 第一个，index 0）是默认激活页。
        if (containerCount > 0) {
            switchContainer(0);
        }
    }

    // ==================== Build helpers ====================

    /**
     * 灵剑（剑道）信息行。仅当 {@code state.active()} 时返回 3 行：品阶 / 封存 / 人剑合一。
     * active=false 返回空列表（调用方不渲染该区块）。
     *
     * <p>数据源：{@link SwordBondHudStateStore}（F.3 契约锁定接口）。
     * 封存以百分比展示（storedQiRatio × 100），无独立 cap 字段。
     *
     * <p>设计为 {@code public static} 以便单测在不启动 MinecraftClient 的情况下验证行为。
     */
    public static List<String> swordBondInfoLines(SwordBondHudState state) {
        if (!state.active()) {
            return List.of();
        }
        String gradeLine   = "§7品阶：§f" + state.gradeName();
        String storedLine  = String.format(java.util.Locale.ROOT, "§7封存：§f%.0f%%", state.storedQiRatio() * 100f);
        String bondLine    = String.format(java.util.Locale.ROOT, "§7人剑合一：§f%.0f%%", state.bondStrength() * 100f);
        return List.of(gradeLine, storedLine, bondLine);
    }

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

    private FlowLayout buildCraftTabEntryContent() {
        FlowLayout panel = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        panel.gap(4);
        panel.padding(Insets.of(4));
        panel.surface(Surface.flat(0xFF12121C).and(Surface.outline(0xFF4A4050)));
        panel.cursorStyle(CursorStyle.HAND);

        LabelComponent title = Components.label(Text.literal("手搓台"));
        title.color(Color.ofArgb(0xFFE8DDC4));
        panel.child(title);

        LabelComponent hint = Components.label(Text.literal("C 打开手搓台"));
        hint.color(Color.ofArgb(0xFFA8A8B8));
        hint.maxWidth(160);
        panel.child(hint);

        LabelComponent status = Components.label(Text.literal(craftStatusLine()));
        status.color(Color.ofArgb(0xFF80FFCC));
        status.maxWidth(160);
        panel.child(status);

        panel.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) {
                openCraftScreen();
                return true;
            }
            return false;
        });

        return panel;
    }

    private static String craftStatusLine() {
        com.bong.client.craft.CraftSessionStateView session = com.bong.client.craft.CraftStore.sessionState();
        if (session != null && session.active()) {
            return "当前任务进行中";
        }
        int knownRecipes = com.bong.client.craft.CraftStore.recipes().size();
        return "已知配方 " + knownRecipes;
    }

    static String craftStatusLineForTests() {
        return craftStatusLine();
    }

    static boolean opensCraftScreenForTabForTests(int idx) {
        return idx == TAB_CRAFT;
    }

    private void openCraftScreen() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null) {
            client.setScreen(new CraftScreen());
        }
    }

    private void hydrateQuickUseFromStore() {
        QuickSlotConfig config = QuickUseSlotStore.snapshot();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            QuickSlotEntry entry = config.slot(i);
            if (entry == null) {
                quickUseItems[i] = null;
                setQuickUseSlotVisual(i, null);
                continue;
            }
            InventoryItem matched = findItemInModel(entry.itemId());
            quickUseItems[i] = matched;
            setQuickUseSlotVisual(i, matched);
        }
    }

    private void setQuickUseSlotVisual(int index, InventoryItem item) {
        setSlotVisual(quickUseSlots[index], item);
    }

    private void setSlotVisual(GridSlotComponent slot, InventoryItem item) {
        if (slot == null) return;
        if (item != null) slot.setItem(item, true);
        else slot.clearItem();
    }

    private void hydrateSkillBarFromStore() {
        var config = SkillBarStore.snapshot();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            SkillBarEntry entry = config.slot(i);
            if (entry == null) {
                if (hotbarSlots[i] != null) {
                    if (hotbarItems[i] != null) hotbarSlots[i].setItem(hotbarItems[i], true);
                    else hotbarSlots[i].clearItem();
                }
                continue;
            }
            if (entry.kind() == SkillBarEntry.Kind.ITEM) {
                InventoryItem matched = findItemInModel(entry.id());
                if (matched == null && !model.isEmpty()) {
                    // P1 — 槽绑定的物品在 authoritative 库存里已不存在（放置耗尽最后一个实例 →
                    // server 删实例 → inventory_snapshot 回写后 model 里再也找不到该 template_id）。
                    // 服务端 skillbar_config emit 用 Changed<SkillBarBindings> 触发，而放置消耗不
                    // 改 SkillBarBindings 组件（仍持悬空 instance_id）→ 服务端不会主动重推清空，
                    // 故由客户端依据 inventory_snapshot 自洽清槽。SkillBarStore.updateSlot(null) 同步
                    // 触发 selectedSlot 失效复位（clearInvalidSelectedSlot），避免热键仍指向空槽。
                    //
                    // 仅在 model 非空（已落地 authoritative snapshot）时清，防开屏首帧空 model
                    // 误清掉服务端刚下发的有效绑定。
                    SkillBarStore.updateSlot(i, null);
                    if (hotbarSlots[i] != null) {
                        if (hotbarItems[i] != null) hotbarSlots[i].setItem(hotbarItems[i], true);
                        else hotbarSlots[i].clearItem();
                    }
                    continue;
                }
                if (hotbarSlots[i] != null) {
                    // count>1 时由 GridSlotComponent.drawItemOverlays 在右下角绘数量；
                    // count 扣减/=0 全程由 inventory_snapshot → matched 自然刷新，无需本地扣减。
                    // model 为空（未加载）且暂时 matched==null 时，沿用占位名硬撑，待 snapshot 到达再校准。
                    hotbarSlots[i].setItem(
                        matched == null ? InventoryItem.simple(entry.id(), entry.displayName()) : matched,
                        true
                    );
                }
                continue;
            }
            if (hotbarSlots[i] != null) {
                hotbarSlots[i].setItem(InventoryItem.simple("skill_scroll_" + safeSkillIconId(entry.id()), entry.displayName()), true);
            }
        }
    }

    private static String safeSkillIconId(String skillId) {
        // 与 HUD 共用同一规约，避免两端解析到不同贴图路径。
        return com.bong.client.combat.SkillIconIds.safeSkillIconId(skillId);
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
        quickUseItems[index] = item;
        setQuickUseSlotVisual(index, item);
        com.bong.client.network.ClientRequestSender.sendQuickSlotBind(
            index, item == null ? null : item.itemId());
    }

    /**
     * 拖放落到快捷使用栏（F1-F9）槽位时的绑定收口。
     *
     * <p>按 plan-block-placement-ux-v1 §N.1 决议：quick_slot 栏（F 键 cast/use）与 SkillBarStore
     * 栏（1-9 选中放置）是两套独立的 server 语义（{@code QuickSlotBindings} vs {@code SkillBarBindings}），
     * 不合并成单条 C2S。对所有物品照常发 {@code quick_slot_bind}（{@link #publishQuickUseSlot}）；
     * 仅当落进的是可放置方块（{@link #isBlockQuickBarBindable}）时，<b>额外</b>同时写 SkillBarStore +
     * 发 {@code skill_bar_bind}（{@link #bindBlockItemToSkillBar}），使「拖放方块到快捷栏」与右键菜单
     * 「绑定到 N」同效，让选中该槽（1-9）后右键放置可读到该方块实例。
     *
     * <p>非方块物品只走 quick_slot 一条路径，行为不变。
     */
    void commitQuickUseDrop(int index, InventoryItem item) {
        publishQuickUseSlot(index, item);
        if (isBlockQuickBarBindable(item)) {
            bindBlockItemToSkillBar(index, item);
        }
    }

    /**
     * B 准则（快捷栏来源限制）：被拖物品的来源是否允许指派到快捷使用栏（F1-F9）。
     *
     * <ul>
     *   <li>{@code QUICK_USE}（快捷栏内重排）→ 放行。</li>
     *   <li>{@code GRID} 且来源容器为 body_pocket，或该容器 def 的 {@code quickAccess}（[快捷] 背包）→ 放行。</li>
     *   <li>其余 GRID / HOTBAR / EQUIP / MERIDIAN / BODY_PART / 来源容器未知 → 拒绝。</li>
     * </ul>
     *
     * <p>HOTBAR 决策：{@code DragState.pickupFromHotbar} 不记 sourceContainerId（主手栏不属任何容器），
     * 按规则 B 字面（仅 body_pocket / [快捷]容器物品可指派）严格解读 → 拒绝。若后续产品拍板放行
     * 主手栏物品，单独为 HOTBAR 放行即可。
     */
    boolean isQuickAccessSource(DragState ds) {
        if (ds == null || ds.sourceKind() == null) {
            return false;
        }
        switch (ds.sourceKind()) {
            case QUICK_USE:
                return true;
            case GRID: {
                String cid = ds.sourceContainerId();
                if (cid == null) {
                    return false; // GRID 异常：无来源容器 id，兜底拒绝。
                }
                if (InventoryModel.BODY_POCKET_CONTAINER_ID.equals(cid)) {
                    return true;
                }
                for (var def : model.containers()) {
                    if (cid.equals(def.id())) {
                        return def.quickAccess();
                    }
                }
                return false;
            }
            case HOTBAR:
            case EQUIP:
            case MERIDIAN:
            case BODY_PART:
            default:
                return false;
        }
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
        // plan-layered-equip-v1 P4（决议 #13）：行囊面板删除后，body_pocket 是普通容器 grid（index 0），
        // 与其他容器同走 containerGrids 路径，不再有 bodyPocketGrid 特例。
        if (activeContainer < 0 || activeContainer >= containerGrids.length) return null;
        return containerGrids[activeContainer];
    }

    // ==================== Tab / Container switching ====================

    private void switchTab(int idx) {
        if (idx == activeTab || idx < 0 || idx >= TAB_NAMES.length) return;
        activeTab = idx;
        FlowLayout[] tabs = {
            equipTabContent,
            cultivationTabContent,
            skillTabContent,
            techniquesTabContent,
            craftTabContent,
        };
        for (int i = 0; i < tabs.length; i++) {
            tabLabels[i].color(Color.ofArgb(i == idx ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            if (tabs[i] != null) {
                tabs[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
            }
        }
        // 切到技艺 tab 时刷一次最新快照（离开其他 tab 时可能积攒了若干事件）。
        if (idx == TAB_SKILL) {
            refreshSkillRows(com.bong.client.skill.SkillSetStore.snapshot());
        } else if (idx == TAB_TECHNIQUES && techniquesTabPanel != null) {
            techniquesTabPanel.refreshFromStores();
            hydrateSkillBarFromStore();
        } else if (idx == TAB_CRAFT) {
            openCraftScreen();
        }
    }

    static java.util.List<String> tabNamesForTests() {
        return java.util.List.of(TAB_NAMES);
    }
    // plan-layered-equip-v1 P4（决议 #19）：行囊重量条 backpackWeightBreakdown 删除——负重沿用整体
    // inventory 底部既有 BottomInfoBar（读 model.currentWeight()/maxWeight()，过载变红）。

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
            case COMBAT -> combatCurrentEffect(lv);
            case MINERAL -> mineralCurrentEffect(lv);
            case CULTIVATION -> cultivationCurrentEffect(lv);
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
                case COMBAT -> combatNextEffect(nextLv);
                case MINERAL -> mineralNextEffect(nextLv);
                case CULTIVATION -> cultivationNextEffect(nextLv);
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

    private static String combatCurrentEffect(int effectiveLv) {
        return "击杀、实战与截脉对练会增长此项；当前只作熟练度记录，细分武学待后续 plan。"
            + " 实效 Lv." + effectiveLv + "。";
    }

    private static String combatNextEffect(int nextLv) {
        return "实战熟练度将提到 Lv." + nextLv + "；细分剑术/拳法等仍由后续 combat plan 定义。";
    }

    private static String mineralCurrentEffect(int effectiveLv) {
        return "采矿、辨矿与落袋会增长此项；当前只作熟练度记录。实效 Lv." + effectiveLv + "。";
    }

    private static String mineralNextEffect(int nextLv) {
        return "采矿熟练度将提到 Lv." + nextLv + "；矿物品质/概率加成留给 mineral 后续表。";
    }

    private static String cultivationCurrentEffect(int effectiveLv) {
        return "开脉与突破会增长此项；境界仍是根本，本项只记行功熟练。实效 Lv." + effectiveLv + "。";
    }

    private static String cultivationNextEffect(int nextLv) {
        return "行功熟练度将提到 Lv." + nextLv + "；不替代境界，只辅助展示修行履历。";
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

    /**
     * A 准则（worn-tab）：owner 背包件是否当前穿戴在某身体槽的 worn 层（区别于「当货物塞在
     * body_pocket / 手持」）。worn → 显常驻 tab；否则只走悬浮窗。
     *
     * <p>判据走 {@link SlotContents#worn()} 全栈（而非 {@code wornTop()} / {@code held()}）：
     * {@code held} 是手持代表件不算「穿戴上身」；用全栈而非栈顶避免叠穿（多层穿戴）时非栈顶 pack 漏判。
     */
    static boolean isPackOwnerInWornSlot(InventoryModel m, long ownerInstanceId) {
        for (var contents : m.equippedSlots().values()) {
            if (contents == null) continue;
            for (var worn : contents.worn()) {
                if (worn != null && worn.instanceId() == ownerInstanceId) {
                    return true;
                }
            }
        }
        return false;
    }

    /**
     * plan-tarkov-backpack-v1 后续修复：从 model.containers() 计算容器 tab 列表，
     * body_pocket 恒置 index 0（默认/最左，决议 #18），server 未下发时补默认 2×3。
     *
     * <p>A 准则：穿戴态的 {@code pack_<id>} 容器（owner 在 worn 层）也进 tab；穿/卸背包经
     * {@code containerStructureChanged → rebuildContainerSection} 自动增删该 tab（列表成员集合变化触发
     * {@link #containerDefsDiffer}）。reconcile（每 snapshot 跑、不依赖 structuralChange）独立管理悬浮窗
     * 开关，与 tab 增删互不冲突——worn↔非worn 切换不改变 {@code containers()} 成员，悬浮窗不被误关。
     */
    static java.util.List<InventoryModel.ContainerDef> computeContainerDefs(InventoryModel m) {
        java.util.List<InventoryModel.ContainerDef> defs = new java.util.ArrayList<>();
        InventoryModel.ContainerDef bodyPocketDef = null;
        for (var def : m.containers()) {
            if (InventoryModel.BODY_POCKET_CONTAINER_ID.equals(def.id())) {
                bodyPocketDef = def;
            } else if (parseWornPackInstance(def.id()).isPresent()) {
                // A 准则（worn-tab）：owner 背包件当前**穿戴**在某身体槽 worn 层 → 保留常驻 tab
                // （双击/右键悬浮窗入口并存，不互斥）；非 worn（当货物躺 body_pocket / 手持 / 已落地）
                // → skip，仅经悬浮窗访问。收窄 #778「pack 一律不显 tab」为「仅穿戴态显 tab」。
                Long owner = def.ownerInstanceId();
                if (owner != null && isPackOwnerInWornSlot(m, owner)) {
                    defs.add(def);
                }
                // else（owner==null 旧 server 缺字段 / owner 不在 worn 层）：skip，不退化为显示。
            } else {
                defs.add(def);
            }
        }
        if (bodyPocketDef == null) {
            bodyPocketDef = new InventoryModel.ContainerDef(
                InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3);
        }
        defs.add(0, bodyPocketDef);
        return defs;
    }

    /**
     * 两个有序容器列表的结构（id + rows + cols）是否不同。
     * 仅内含物变化 → false（populateFromModel 即可重填）；容器增/删/换尺寸 → true（需 rebuildContainerSection）。
     */
    static boolean containerDefsDiffer(
            java.util.List<InventoryModel.ContainerDef> a,
            java.util.List<InventoryModel.ContainerDef> b) {
        if (a.size() != b.size()) return true;
        for (int i = 0; i < a.size(); i++) {
            var x = a.get(i);
            var y = b.get(i);
            if (!x.id().equals(y.id()) || x.rows() != y.rows() || x.cols() != y.cols()) {
                return true;
            }
        }
        return false;
    }

    /** 新 snapshot 的容器集合是否与当前 tab 栏结构不同 → 需重建 tab 栏 + grids。 */
    private boolean containerStructureChanged(InventoryModel next) {
        return containerDefsDiffer(computeContainerDefs(next), filteredContainerDefs);
    }

    /**
     * plan-tarkov-backpack-v1 后续修复：从当前 model 重建容器 tab 栏 + grids 到 containerSection。
     * build() 初次构建 + snapshot 结构变化（卸/穿背包→容器增删）时调用，免重开界面。
     * 保留当前选中容器（id 仍在则切回，否则落 body_pocket=0）。
     */
    private void rebuildContainerSection() {
        if (containerSection == null) return;
        String prevActiveId = (activeContainer >= 0 && activeContainer < filteredContainerDefs.size())
            ? filteredContainerDefs.get(activeContainer).id()
            : null;

        var containerDefs = computeContainerDefs(model);
        filteredContainerDefs = containerDefs;
        containerCount = containerDefs.size();
        containerGrids = new BackpackGridPanel[containerCount];
        containerWrappers = new FlowLayout[containerCount];
        containerLabels = new LabelComponent[containerCount];
        activeContainer = -1; // 强制下面 switchContainer 真正生效（不被 idx==activeContainer 短路）

        containerSection.clearChildren();

        FlowLayout containerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        containerRow.gap(2);
        int maxCols = 3; // floor at body_pocket(2×3) width 防容器面板宽度塌
        for (var def : containerDefs) maxCols = Math.max(maxCols, def.cols());

        // 全部容器 tab（body_pocket 第一个 + 身体槽 worn 背包件产生的容器）。
        for (int i = 0; i < containerCount; i++) {
            final int ci = i;
            var def = containerDefs.get(i);

            FlowLayout tab = Containers.horizontalFlow(Sizing.content(), Sizing.fixed(14));
            tab.surface(Surface.flat(0xFF1E1E1E));
            tab.padding(Insets.of(1, 4, 1, 4));
            tab.verticalAlignment(VerticalAlignment.CENTER);
            tab.cursorStyle(CursorStyle.HAND);

            var label = Components.label(Text.literal(
                "§7" + def.name() + " §8(" + def.rows() + "×" + def.cols() + ")"
            ));
            containerLabels[i] = label;
            tab.child(label);
            tab.mouseDown().subscribe((mx, my, btn) -> {
                if (btn == 0) { switchContainer(ci); return true; }
                return false;
            });
            tab.mouseEnter().subscribe(() -> {
                if (dragState.isDragging()) switchContainer(ci);
            });
            containerRow.child(tab);
        }
        containerSection.child(containerRow);

        // Build all grids, hidden by default（active 页由下方 switchContainer 切出）。
        int wrapperW = maxCols * GridSlotComponent.CELL_SIZE + 4;
        for (int i = 0; i < containerCount; i++) {
            var def = containerDefs.get(i);
            containerGrids[i] = new BackpackGridPanel(def.id(), def.rows(), def.cols());
            FlowLayout w = Containers.verticalFlow(Sizing.fixed(wrapperW), Sizing.content());
            w.surface(Surface.flat(0xFF111111));
            w.padding(Insets.of(2));
            w.child(containerGrids[i].container());
            containerWrappers[i] = w;
            containerSection.child(w);
            w.positioning(Positioning.absolute(-9999, -9999));
        }

        // 恢复选中：原选中容器仍在 → 切回；否则落 body_pocket(index 0)。
        int target = 0;
        if (prevActiveId != null) {
            for (int i = 0; i < containerCount; i++) {
                if (filteredContainerDefs.get(i).id().equals(prevActiveId)) { target = i; break; }
            }
        }
        if (containerCount > 0) switchContainer(target);
    }

    private void switchContainer(int idx) {
        if (idx == activeContainer || idx < 0 || idx >= containerCount) return;
        activeContainer = idx;
        // 用 filteredContainerDefs 索引 —— 与 containerLabels[]/containerGrids[] 同源同序（body_pocket=index 0）。
        var defs = filteredContainerDefs;
        for (int i = 0; i < containerCount; i++) {
            containerWrappers[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
            var def = defs.get(i);
            containerLabels[i].text(Text.literal(
                (i == idx ? "§f" : "§7") + def.name()
                + " §8(" + def.rows() + "×" + def.cols() + ")"
            ));
        }
    }

    /** 切换到承载指定 containerId 的页（body_pocket 与其余容器同走普通 switchContainer，决议 #13）。无匹配则不动。 */
    private void switchToGridContainer(String containerId) {
        if (containerId == null) return;
        for (int i = 0; i < containerCount; i++) {
            if (filteredContainerDefs.get(i).id().equals(containerId)) {
                switchContainer(i);
                return;
            }
        }
    }

    /**
     * plan-layered-equip-v1 P4（决议 #12）：弹出该槽栈顶/held（仅 LIFO 顶层可动）。
     * 手槽 → 清 held；身体槽 → pop worn 末尾（栈顶）；下层保持不动。乐观本地更新，server 快照为权威。
     */
    private void popSlotTop(EquipSlotComponent eq) {
        if (eq == null) return;
        com.bong.client.inventory.model.SlotContents c = eq.contents();
        if (eq.slotType().isHand() || c.held() != null) {
            // 手槽（held）或恰好持械：清 held，worn 不变。
            eq.setContents(new com.bong.client.inventory.model.SlotContents(c.worn(), null));
            return;
        }
        java.util.List<InventoryItem> stack = new java.util.ArrayList<>(c.worn());
        if (!stack.isEmpty()) {
            stack.remove(stack.size() - 1); // pop 栈顶
        }
        eq.setContents(new com.bong.client.inventory.model.SlotContents(stack, c.held()));
    }

    /** 把件推回槽：手槽 → held 单件；身体槽 → push worn 栈顶（决议 #12）。 */
    private void pushSlot(EquipSlotComponent eq, InventoryItem item) {
        if (eq == null || item == null) return;
        com.bong.client.inventory.model.SlotContents c = eq.contents();
        if (eq.slotType().isHand()) {
            eq.setContents(new com.bong.client.inventory.model.SlotContents(c.worn(), item));
        } else {
            java.util.List<InventoryItem> stack = new java.util.ArrayList<>(c.worn());
            stack.add(item);
            eq.setContents(new com.bong.client.inventory.model.SlotContents(stack, c.held()));
        }
    }

    // ==================== Populate ====================

    /**
     * 测试专用：以 authoritative inventory snapshot 设置 model 后，仅驱动 P1 的 SkillBar 自洽收口
     * （{@link #hydrateSkillBarFromStore}）。
     *
     * <p>生产路径是 {@code listener → MinecraftClient.execute(this.model = next; populateFromModel())}，
     * 而 {@code populateFromModel} 还会刷 owo UI 面板（equipPanel/statusBars/...），这些面板只在
     * owo {@code build()} 生命周期里实例化、headless 测试不可用。本 seam 只设 model + 调
     * {@code hydrateSkillBarFromStore}，精确锁住「实例消耗后清槽 / 仍在则保留 / count 刷新 /
     * selectedSlot 失效复位 / 空 model 不误清」这些<b>客户端状态机</b>契约（其余 UI 刷新与槽位视觉
     * 由 e2e 覆盖）。</p>
     */
    void reconcileSkillBarForTests(InventoryModel next) {
        if (next == null) return;
        this.model = next;
        hydrateSkillBarFromStore();
    }

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

    boolean dispatchSetMeridianTarget() {
        MeridianChannel selected = bodyInspect == null ? null : bodyInspect.selectedChannel();
        MeridianBody body = bodyInspect == null ? MeridianStateStore.snapshot() : bodyInspect.meridianBody();
        String blockedReason = meridianTargetBlockReason(body, selected);
        if (blockedReason != null) {
            showActionToast(blockedReason, ACTION_TOAST_WARN);
            com.bong.client.BongClient.LOGGER.warn("[bong][inspect] set_meridian_target skipped: {}", blockedReason);
            return false;
        }

        com.bong.client.network.ClientRequestSender.sendSetMeridianTarget(
            com.bong.client.network.ClientRequestProtocol.toMeridianId(selected));
        showActionToast("经脉目标：" + selected.displayName(), ACTION_TOAST_OK);
        return true;
    }

    boolean dispatchBreakthroughRequest() {
        com.bong.client.network.ClientRequestSender.sendBreakthroughRequest();
        showActionToast("已请求突破", ACTION_TOAST_OK);
        return true;
    }

    boolean dispatchStartDuXuIfEligible() {
        MeridianBody body = bodyInspect == null ? MeridianStateStore.snapshot() : bodyInspect.meridianBody();
        if (!isDuXuEligible(body)) {
            // 灰态按钮仍可点击（颜色仅 cosmetic），被拦时必须给玩家可见反馈，
            // 与 dispatchSetMeridianTarget 的 showActionToast 一致。按未满足的具体条件分流文案。
            String reason = (body == null || !"Spirit".equals(body.realm()))
                ? "渡虚劫需通灵境"
                : "渡虚劫需打通全部经脉";
            showActionToast(reason, ACTION_TOAST_WARN);
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] start_du_xu skipped: requires Spirit realm and all 20 meridians opened");
            return false;
        }
        com.bong.client.network.ClientRequestSender.sendStartDuXuRequest();
        return true;
    }

    static String meridianTargetBlockReason(MeridianBody body, MeridianChannel selected) {
        if (body == null) {
            return "经脉数据加载中";
        }
        if (selected == null) {
            return "先点击一条经脉";
        }
        ChannelState state = body.channel(selected);
        if (state != null && !state.blocked()) {
            return selected.displayName() + "已通，不需要再设为经脉目标";
        }
        if (openedMeridianCount(body) == 0
            && selected.family() == MeridianChannel.Family.EXTRAORDINARY) {
            return "首脉需先走十二正经";
        }
        return null;
    }

    private static int openedMeridianCount(MeridianBody body) {
        if (body == null) {
            return 0;
        }
        int count = 0;
        for (MeridianChannel channel : MeridianChannel.values()) {
            ChannelState state = body.channel(channel);
            if (state != null && !state.blocked()) {
                count++;
            }
        }
        return count;
    }

    private static void showActionToast(String text, int color) {
        BongToast.show(text, color, System.currentTimeMillis(), ACTION_TOAST_MS);
    }

    // plan-tarkov-backpack-v1 P2（交付物 #3，client 穿戴态门控）——
    // 镜像 server `worn_pack_instance_from_container_id`（inventory/mod.rs:3795）：
    // 仅 `pack_<数字>` 形态解析出 owner instance id；body_pocket / pack_grass_pouch（占位，
    // 非数字后缀）/ main_pack 等返回 empty（不视作可门控的套包容器，放行）。
    /**
     * plan-tarkov-backpack-v1 P3（交付物 #2）—— 装备槽双击判定（owo 无 clickCount，Screen 级手算）。
     *
     * <p>当前点击的槽与上次点击同一槽、且距上次点击 ≤ {@link #DOUBLE_CLICK_WINDOW_MS} 时视为双击。
     * 不同槽、超窗、首次点击（lastSlot 为 null）均非双击。纯计时逻辑，抽出供单测覆盖状态转换。</p>
     *
     * @param lastSlot     上次点击的装备槽（首次为 null）
     * @param lastTimeMs   上次点击时间戳（ms）
     * @param currentSlot  本次点击的装备槽
     * @param nowMs        本次点击时间戳（ms）
     */
    static boolean isEquipDoubleClick(
            EquipSlotType lastSlot, long lastTimeMs, EquipSlotType currentSlot, long nowMs) {
        if (currentSlot == null || lastSlot != currentSlot) {
            return false;
        }
        return (nowMs - lastTimeMs) <= DOUBLE_CLICK_WINDOW_MS;
    }

    /**
     * fix/tarkov-nest-persistence §C1/§C2 —— grid / hotbar 格双击判定（按 instanceId 区分物品）。
     *
     * <p>同一 instance 两连击、且距上次 ≤ {@link #DOUBLE_CLICK_WINDOW_MS} 视为双击；不同 instance、
     * 超窗、首次点击（lastInstanceId &lt; 0）均非双击。纯计时逻辑，抽出供单测覆盖状态转换。
     * 用 instanceId（而非坐标）区分，背包在 body_pocket / hotbar 间移动后仍稳定。</p>
     *
     * @param lastInstanceId 上次点击物品 instance（首次为负数，如 -1）
     * @param lastTimeMs     上次点击时间戳（ms）
     * @param instanceId     本次点击物品 instance
     * @param nowMs          本次点击时间戳（ms）
     */
    static boolean isInstanceDoubleClick(
            long lastInstanceId, long lastTimeMs, long instanceId, long nowMs) {
        if (lastInstanceId < 0 || lastInstanceId != instanceId) {
            return false;
        }
        return (nowMs - lastTimeMs) <= DOUBLE_CLICK_WINDOW_MS;
    }

    static java.util.OptionalLong parseWornPackInstance(String containerId) {
        if (containerId == null || !containerId.startsWith("pack_")) {
            return java.util.OptionalLong.empty();
        }
        String suffix = containerId.substring("pack_".length());
        if (suffix.isEmpty()) {
            return java.util.OptionalLong.empty();
        }
        for (int i = 0; i < suffix.length(); i++) {
            if (!Character.isDigit(suffix.charAt(i))) {
                return java.util.OptionalLong.empty();
            }
        }
        try {
            return java.util.OptionalLong.of(Long.parseLong(suffix));
        } catch (NumberFormatException e) {
            return java.util.OptionalLong.empty();
        }
    }

    // plan-tarkov-backpack-v1 P2（交付物 #3）——拖入目标容器的穿戴态门控谓词（client 侧）。
    // 仅 `pack_<数字>` 套包容器受门控：其 owner 背包件必须当前穿戴在某身体槽 worn 层才允许拖入
    // （塔科夫式语义：卸下的包是死容器）。非套包容器（body_pocket / main_pack / pack_grass_pouch 等）
    // 恒放行。与 server `validate_move_semantics` 的 Container 分支门控对齐，避免「拖进去瞬间弹回且无提示」。
    static boolean isWornPackContainerDroppable(
            com.bong.client.inventory.model.InventoryModel snapshot, String containerId) {
        java.util.OptionalLong ownerOpt = parseWornPackInstance(containerId);
        if (ownerOpt.isEmpty()) {
            return true; // 非 pack_<数字> 容器不受穿戴态门控。
        }
        if (snapshot == null) {
            return false; // 套包容器但无快照可校验穿戴态 → 保守拒绝（不乐观落位）。
        }
        long owner = ownerOpt.getAsLong();
        // plan-tarkov-floating-windows bug #2：放宽到「携带面任意位置」，精确镜像 server #777
        // find_pack_instances_anywhere（worn + held + hotbar + body_pocket）。此前只扫 worn 层，
        // 漏 held/hotbar/body_pocket，导致包卸到 body_pocket 当货物时 owner 找不到 → 误拒（拖进瞬间弹回）。
        for (com.bong.client.inventory.model.SlotContents contents :
                snapshot.equippedSlots().values()) {
            if (contents == null) {
                continue;
            }
            for (InventoryItem worn : contents.worn()) {
                if (worn != null && worn.instanceId() == owner) {
                    return true; // owner 背包件在某身体槽 worn 层。
                }
            }
            if (contents.held() != null && contents.held().instanceId() == owner) {
                return true; // owner 背包件被手持。
            }
        }
        for (InventoryItem h : snapshot.hotbar()) {
            if (h != null && h.instanceId() == owner) {
                return true; // owner 背包件在快捷栏。
            }
        }
        for (InventoryModel.GridEntry e : snapshot.gridItems()) {
            if (InventoryModel.BODY_POCKET_CONTAINER_ID.equals(e.containerId())
                    && e.item() != null && e.item().instanceId() == owner) {
                return true; // owner 背包件当货物塞在 body_pocket（卸下不丢，仍可作为容器拖入）。
            }
        }
        return false; // owner 背包件不在任何携带面 → 真丢地/转移走，拒绝拖入。
    }

    static boolean isDuXuEligible(MeridianBody body) {
        if (body == null || !"Spirit".equals(body.realm())) {
            return false;
        }
        for (MeridianChannel channel : MeridianChannel.values()) {
            ChannelState state = body.channel(channel);
            if (state == null || state.blocked()) {
                return false;
            }
        }
        return true;
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
    public void tick() {
        super.tick();
        InventoryItem ready = itemInspectLongPress.consumeReady(System.currentTimeMillis());
        if (ready != null && client != null) {
            client.setScreen(new ItemInspectScreen(ready));
        }
        // Loot panel timer tick
        if (lootPanel != null && !lootPanel.isClosed()) {
            if (lootPanel.tickTimer()) {
                unmountLootPanel();
            }
        }
    }

    /**
     * 处理已打开的 context menu（pill / skillBar / weapon）上的点击。
     *
     * <p>plan-block-placement-ux-v1 P2：菜单 action 行的命中-触发对左键(0)与右键(1)都生效；
     * 命中行外侧时只有右键关闭菜单，左键点空白不关（避免误操作）。返回 {@code true} 表示本次点击
     * 已被菜单消费，{@code mouseClicked} 应短路返回。
     *
     * <p>同一时刻至多一个菜单处于打开态（open* 方法互斥清空其它菜单），故按 pill→skillBar→weapon
     * 顺序检查并在第一个命中处返回。
     */
    boolean handleContextMenuClick(double mouseX, double mouseY, int button) {
        if (pillContextMenu != null) {
            int actionIdx = pillMenuActionIndexAt(mouseX, mouseY);
            if (actionIdx >= 0) {
                triggerPillMenuAction(pillContextMenu.actions().get(actionIdx).kind());
                itemInspectLongPress.cancel();
                return true;
            }
            if (button == 1) {
                pillContextMenu = null;
                pendingMeridianUse = null;
                itemInspectLongPress.cancel();
                return true;
            }
            return false;
        }

        if (skillBarContextMenu != null) {
            int actionIdx = skillBarMenuActionIndexAt(mouseX, mouseY);
            if (actionIdx >= 0) {
                triggerSkillBarMenuAction(skillBarContextMenu.actions().get(actionIdx).slot());
                itemInspectLongPress.cancel();
                return true;
            }
            if (button == 1) {
                skillBarContextMenu = null;
                itemInspectLongPress.cancel();
                return true;
            }
            return false;
        }

        if (weaponContextMenu != null) {
            int actionIdx = weaponMenuActionIndexAt(mouseX, mouseY);
            if (actionIdx >= 0) {
                triggerWeaponMenuAction(weaponContextMenu.actions().get(actionIdx).kind());
                itemInspectLongPress.cancel();
                return true;
            }
            if (button == 1) {
                weaponContextMenu = null;
                itemInspectLongPress.cancel();
                return true;
            }
            return false;
        }

        return false;
    }

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (button != 1) {
            itemInspectLongPress.cancel();
        }

        // plan-block-placement-ux-v1 P2 — context menu（pill/skillBar/weapon）的「命中-触发」对
        // 左键(0)和右键(1)都响应：一个菜单已经打开后，玩家用左键点 action 行也应触发，符合
        // 直觉。菜单的「打开」仍只在右键/长按（见下方 button==1 块的 open* 调用）。菜单的「关闭」
        // （点 action 行之外）只在右键触发，左键点空白不关菜单，避免误关。
        if (handleContextMenuClick(mouseX, mouseY, button)) {
            return true;
        }

        if (button == 0 && pendingMeridianUse != null && confirmPendingMeridianUse()) {
            return true;
        }

        // plan-tarkov-floating-windows：套包悬浮窗 z 最上，优先命中（点 ✕ 关闭 / grid 拾取 / 标题栏拖动 +
        // 点击置顶）。命中即不穿透到主 grid。RAISED = 标题栏/空白区 → 置顶后交回 owo（focusHandler →
        // DraggableContainer.childAt 返回窗口 → 后续 mouseDragged 路由 onMouseDrag 移位）。
        PackWindowClick pw = handlePackWindowMouseDown(mouseX, mouseY, button);
        if (pw == PackWindowClick.CONSUMED) {
            itemInspectLongPress.cancel();
            return true;
        }
        if (pw == PackWindowClick.RAISED) {
            itemInspectLongPress.cancel();
            return super.mouseClicked(mouseX, mouseY, button);
        }

        if (button == 1) {
            if (activeTab == TAB_EQUIP) {
                var eq = equipPanel.slotAtScreen(mouseX, mouseY);
                // 决议 #12：仅栈顶/held（representative）可操作。
                InventoryItem top = eq == null ? null : eq.representative();
                // fix/tarkov-nest-persistence §C3 — 右键背包件直接开包（容器件对 weapon/pill 菜单
                // 都返回 false，故前置拦截）。覆盖装备槽持有位。
                if (eq != null && top != null && InventoryEquipRules.isContainer(top)) {
                    openWornContainerPanel(top, eq.slotType());
                    itemInspectLongPress.cancel();
                    return true;
                }
                if (eq != null && top != null && openWeaponContextMenu(eq.slotType(), top, (int) mouseX, (int) mouseY)) {
                    itemInspectLongPress.cancel();
                    return true;
                }
            }

            // plan-exploration-probe-return-v1 P1 fix(M1-②): 先清理 pendingMeridianUse 状态，
            // 防止 Shift+右键触发保鲜探针时残留旧的「待选经脉外敷���状态导致后续误触发。
            if (pendingMeridianUse != null) {
                pendingMeridianUse = null;
                itemInspectLongPress.cancel();
                return true;
            }

            BackpackGridPanel grid = activeGrid();
            if (grid != null && grid.containsPoint(mouseX, mouseY)) {
                var pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = grid.itemAt(pos.row(), pos.col());
                    // fix/tarkov-nest-persistence §C3 — 右键 grid 内背包件直接开包（前置拦截）。
                    if (item != null && InventoryEquipRules.isContainer(item)) {
                        openWornContainerPanel(item, null);
                        itemInspectLongPress.cancel();
                        return true;
                    }
                    // plan-interaction-intent-cleanup-v1 P1 — 感保鲜触发已从「任意物品 Shift+右键」
                    // 收窄并迁出鼠标路径：改为焦点槽位 Shift+F（见 keyPressed），且仅对「已知有
                    // 保鲜数据」的物品发探针，杜绝通配误触与对普通物品的无效探针。
                    if (item != null && openPillContextMenu(item, (int) mouseX, (int) mouseY)) {
                        itemInspectLongPress.cancel();
                        return true;
                    }
                    if (item != null && openSkillBarContextMenu(item, (int) mouseX, (int) mouseY)) {
                        itemInspectLongPress.cancel();
                        return true;
                    }
                }
            }

            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            // fix/tarkov-nest-persistence §C3 — 右键快捷栏背包件直接开包（前置拦截）。
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && InventoryEquipRules.isContainer(hotbarItems[hIdx])) {
                openWornContainerPanel(hotbarItems[hIdx], null);
                itemInspectLongPress.cancel();
                return true;
            }
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && openPillContextMenu(hotbarItems[hIdx], (int) mouseX, (int) mouseY)) {
                itemInspectLongPress.cancel();
                return true;
            }
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && openSkillBarContextMenu(hotbarItems[hIdx], (int) mouseX, (int) mouseY)) {
                itemInspectLongPress.cancel();
                return true;
            }
            // 右键【已绑定功法】的 1-9 槽 → 清空解绑（绑定功法不写 hotbarItems[]，单独走 SkillBarStore）。
            if (activeTab == TAB_TECHNIQUES && hIdx >= 0 && techniquesTabPanel != null) {
                SkillBarEntry bound = SkillBarStore.snapshot().slot(hIdx);
                if (bound != null && bound.kind() == SkillBarEntry.Kind.SKILL
                        && techniquesTabPanel.clearSkillSlot(hIdx)) {
                    hydrateSkillBarFromStore();
                    itemInspectLongPress.cancel();
                    return true;
                }
            }
            itemInspectLongPress.start(itemAtScreen(mouseX, mouseY), mouseX, mouseY, System.currentTimeMillis());
        }

        if (button == 0) {
            boolean shift = hasShiftDown();
            BackpackGridPanel grid = activeGrid();

            // Grid
            if (grid != null && grid.containsPoint(mouseX, mouseY)) {
                var pos = grid.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = grid.itemAt(pos.row(), pos.col());
                    if (item != null) {
                        // fix/tarkov-nest-persistence §C1 — 背包卸入 body_pocket grid 后双击开包。
                        // 非 shift、同一 instance 两连击 ≤ 窗口、且为容器件 → 打开 WornContainerPanel。
                        long now = System.currentTimeMillis();
                        boolean dbl = !shift
                                && isInstanceDoubleClick(lastGridClickInstanceId, lastGridClickTimeMs,
                                        item.instanceId(), now)
                                && InventoryEquipRules.isContainer(item);
                        lastGridClickTimeMs = now;
                        lastGridClickInstanceId = item.instanceId();
                        if (dbl) {
                            lastGridClickTimeMs = 0L;
                            lastGridClickInstanceId = -1L; // 防三连
                            openWornContainerPanel(item, null);
                            return true;
                        }
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
            // 决议 #12：仅栈顶/held（representative）可被拖下/卸下；下层被压住不可动。
            if (activeTab == TAB_EQUIP) {
                var eq = equipPanel.slotAtScreen(mouseX, mouseY);
                InventoryItem item = eq == null ? null : eq.representative();
                if (eq != null && item != null) {
                    // plan-tarkov-backpack-v1 P3（交付物 #2）—— 双击穿戴的背包件 → 打开其内含物视图。
                    // owo 无 clickCount：在 Screen 级手算（同槽两击 ≤ DOUBLE_CLICK_WINDOW_MS 视为双击）。
                    // 非 shift、栈顶件是容器（isContainer）时拦截，打开 pack_<owner> 视图；否则按单击拾取。
                    long now = System.currentTimeMillis();
                    boolean doubleClick = !shift && isEquipDoubleClick(
                        lastEquipClickSlot, lastEquipClickTimeMs, eq.slotType(), now);
                    lastEquipClickTimeMs = now;
                    lastEquipClickSlot = eq.slotType();
                    if (doubleClick && InventoryEquipRules.isContainer(item)) {
                        // 重置计时，避免三连击再次触发；打开后吞掉本次点击（不拾取背包件）。
                        lastEquipClickTimeMs = 0L;
                        lastEquipClickSlot = null;
                        openWornContainerPanel(item, eq.slotType());
                        return true;
                    }
                    if (shift) quickUnequipToGrid(eq.slotType(), item);
                    else {
                        popSlotTop(eq); // 乐观弹出栈顶/held（server 快照为权威）
                        dragState.pickupFromEquip(item, eq.slotType());
                    }
                    return true;
                }
            }

            // Body inspect applied items (physical or meridian layer)
            if (activeTab == TAB_CULTIVATION && bodyInspect != null) {
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

            // 功法拖拽起手：在功法列表行上按下 → 选中 + 进入拖拽态（释放时落槽绑定）。
            // 两步点击（点行选中、点槽绑定）仍由下方 Hotbar 分支兜底。
            if (activeTab == TAB_TECHNIQUES && techniquesTabPanel != null && !dragState.isDragging()) {
                com.bong.client.combat.inspect.TechniquesListPanel.Technique tech =
                    techniquesTabPanel.techniqueAtScreen(mouseX, mouseY);
                if (tech != null) {
                    techniquesTabPanel.selectTechnique(tech.id());
                    draggedTechnique = tech;
                    draggedTechniqueLocked = !techniquesTabPanel.lockReasonFor(tech).isBlank();
                    techniqueDragX = mouseX;
                    techniqueDragY = mouseY;
                    updateTechniqueHotbarHighlight(mouseX, mouseY);
                    return true;
                }
            }

            // Hotbar
            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            // 功法拖出起手：按下一个【已绑定功法】的 1-9 槽 → 拾起拖拽。
            // 松手落到某槽=移动/换绑、落到槽外(背包/空白)=解绑消失（见 mouseReleased + TechniqueDragDecision）。
            if (button == 0 && activeTab == TAB_TECHNIQUES && hIdx >= 0 && techniquesTabPanel != null
                    && draggedTechnique == null && !dragState.isDragging()) {
                SkillBarEntry bound = SkillBarStore.snapshot().slot(hIdx);
                if (bound != null && bound.kind() == SkillBarEntry.Kind.SKILL) {
                    com.bong.client.combat.inspect.TechniquesListPanel.Technique tech =
                        techniquesTabPanel.techniqueById(bound.id());
                    if (tech != null) {
                        techniquesTabPanel.selectTechnique(tech.id());
                        draggedTechnique = tech;
                        draggedFromSlot = hIdx;
                        draggedTechniqueLocked = !techniquesTabPanel.lockReasonFor(tech).isBlank();
                        techniqueDragX = mouseX;
                        techniqueDragY = mouseY;
                        if (hotbarSlots[hIdx] != null) {
                            // 拾起：仅清源槽视觉，store 暂不动；任何取消/失败由 hydrate 还原。
                            hotbarSlots[hIdx].clearItem();
                        }
                        updateTechniqueHotbarHighlight(mouseX, mouseY);
                        return true;
                    }
                }
            }
            if (button == 0 && activeTab == TAB_TECHNIQUES && hIdx >= 0 && techniquesTabPanel != null
                    && techniquesTabPanel.selectedTechnique() != null) {
                if (techniquesTabPanel.bindSelectedTechniqueToSlot(hIdx)) {
                    hydrateSkillBarFromStore();
                    return true;
                }
            }
            if (hIdx >= 0 && hotbarItems[hIdx] != null) {
                InventoryItem item = hotbarItems[hIdx];
                // fix/tarkov-nest-persistence §C2 — 背包件在快捷栏槽双击开包。
                long nowH = System.currentTimeMillis();
                boolean dblH = !shift
                        && isInstanceDoubleClick(lastHotbarClickInstanceId, lastHotbarClickTimeMs,
                                item.instanceId(), nowH)
                        && InventoryEquipRules.isContainer(item);
                lastHotbarClickTimeMs = nowH;
                lastHotbarClickInstanceId = item.instanceId();
                if (dblH) {
                    lastHotbarClickTimeMs = 0L;
                    lastHotbarClickInstanceId = -1L;
                    openWornContainerPanel(item, null);
                    return true;
                }
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
                    setQuickUseSlotVisual(qIdx, null);
                    publishQuickUseSlot(qIdx, null);
                    dragState.pickupFromQuickUse(item, qIdx);
                }
                return true;
            }

            // Loot grid (supply coffin)
            if (lootPanel != null && !lootPanel.isClosed()) {
                BackpackGridPanel lg = lootPanel.lootGrid();
                if (lg.containsPoint(mouseX, mouseY)) {
                    var pos = lg.screenToGrid(mouseX, mouseY);
                    if (pos != null) {
                        InventoryItem item = lg.itemAt(pos.row(), pos.col());
                        if (item != null && dragState.phase() == DragState.Phase.IDLE) {
                            var anchor = lg.anchorOf(item);
                            if (anchor != null) {
                                dragState.pickup(item, lootPanel.extContainerId(),
                                    anchor.row(), anchor.col());
                                lg.remove(item);
                                return true;
                            }
                        }
                    }
                }
            }

            // 套包悬浮窗 grid 的拾取（拖出）已在 mouseClicked 顶部 handlePackWindowMouseDown 处理
            // （窗口 z 最上，先于主 grid/equip 命中）。
        }

        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double deltaX, double deltaY) {
        itemInspectLongPress.move(mouseX, mouseY);
        // 功法拖拽中：吞掉 drag 事件（不下传给 ScrollContainer，否则列表会跟着滚动），
        // 更新 ghost 位置与落点槽位高亮。
        if (draggedTechnique != null) {
            techniqueDragX = mouseX;
            techniqueDragY = mouseY;
            updateTechniqueHotbarHighlight(mouseX, mouseY);
            return true;
        }
        if (dragState.isDragging()) {
            dragState.updateMouse(mouseX, mouseY);
            updateHighlights(mouseX, mouseY);
            return true;
        }
        return super.mouseDragged(mouseX, mouseY, button, deltaX, deltaY);
    }

    @Override
    public boolean mouseReleased(double mouseX, double mouseY, int button) {
        if (button == 1) {
            itemInspectLongPress.cancel();
        }
        // 功法拖拽落槽：松手时若落在某个 1-9 槽上则绑定（锁定功法由 bindTechniqueToSlot 拒绝并给原因）；
        // 落在槽位外则仅取消拖拽，选中态保留（兼容两步点击）。
        if (button == 0 && draggedTechnique != null) {
            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            if (techniquesTabPanel != null) {
                switch (TechniqueDragDecision.decide(hIdx, draggedFromSlot)) {
                    case BIND ->
                        // 从功法列表拖入、或拖回同一槽：绑定（锁定则失败并在状态栏给原因）。
                        techniquesTabPanel.bindTechniqueToSlot(draggedTechnique.id(), hIdx);
                    case MOVE -> {
                        // 从已绑槽移到不同槽：绑新槽成功后清空源槽（失败则不动源槽，hydrate 还原）。
                        if (techniquesTabPanel.bindTechniqueToSlot(draggedTechnique.id(), hIdx)) {
                            techniquesTabPanel.clearSkillSlot(draggedFromSlot);
                        }
                    }
                    case UNBIND ->
                        // 从已绑槽拖出、落在 1-9 之外（背包/空白）：解绑消失。
                        techniquesTabPanel.clearSkillSlot(draggedFromSlot);
                    case CANCEL -> {
                        // 从功法列表拖出、落空：不改绑定，保留选中（两步点击兜底）。
                    }
                }
            }
            hydrateSkillBarFromStore();
            endTechniqueDrag();
            return true;
        }
        if (button == 0 && dragState.isDragging()) {
            attemptDrop(mouseX, mouseY);
            return true;
        }
        return super.mouseReleased(mouseX, mouseY, button);
    }

    /** 拖拽中：清掉所有 hotbar 槽高亮，再把光标下的槽标成绿(可绑)/红(锁定)。 */
    private void updateTechniqueHotbarHighlight(double mouseX, double mouseY) {
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            if (hotbarSlots[i] != null) {
                hotbarSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
            }
        }
        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0 && hotbarSlots[hIdx] != null) {
            hotbarSlots[hIdx].setHighlightState(draggedTechniqueLocked
                ? GridSlotComponent.HighlightState.INVALID
                : GridSlotComponent.HighlightState.VALID);
        }
    }

    /** 结束功法拖拽：清状态 + 清槽位高亮。 */
    private void endTechniqueDrag() {
        draggedTechnique = null;
        draggedTechniqueLocked = false;
        draggedFromSlot = -1;
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            if (hotbarSlots[i] != null) {
                hotbarSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
            }
        }
    }

    private InventoryItem itemAtScreen(double mouseX, double mouseY) {
        BackpackGridPanel grid = activeGrid();
        if (grid != null && grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                InventoryItem item = grid.itemAt(pos.row(), pos.col());
                if (item != null) return item;
            }
        }
        if (activeTab == TAB_EQUIP) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null && eq.representative() != null) return eq.representative();
        }
        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0 && hotbarItems[hIdx] != null) return hotbarItems[hIdx];
        int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
        if (qIdx >= 0 && quickUseItems[qIdx] != null) return quickUseItems[qIdx];
        if (lootPanel != null && !lootPanel.isClosed()) {
            BackpackGridPanel lg = lootPanel.lootGrid();
            if (lg.containsPoint(mouseX, mouseY)) {
                var pos = lg.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    InventoryItem item = lg.itemAt(pos.row(), pos.col());
                    if (item != null) return item;
                }
            }
        }
        return null;
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (keyCode == GLFW.GLFW_KEY_Q && activeTab == TAB_EQUIP && !dragState.isDragging()) {
            var eq = equipPanel.slotAtScreen(mouseX(), mouseY());
            InventoryItem top = eq == null ? null : eq.representative();
            if (eq != null && top != null && InventoryEquipRules.isWeapon(top)) {
                if (dispatchDropWeaponFromEquip(eq.slotType(), top)) {
                    popSlotTop(eq);
                    clearAllHighlights();
                    return true;
                }
            }
        }
        // plan-interaction-intent-cleanup-v1 P1 — 焦点槽位 Shift+F 触发神识感知保鲜，
        // 仅对「已知有保鲜数据」的物品发探针（见 maybeProbeFreshness）。
        if (keyCode == GLFW.GLFW_KEY_F && hasShiftDown() && !dragState.isDragging()) {
            InventoryItem focused = focusedGridItem();
            if (focused != null && maybeProbeFreshness(focused)) {
                itemInspectLongPress.cancel();
                return true;
            }
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    /** 当前鼠标焦点所在 backpack 网格槽位的物品；无则返回 null。 */
    private InventoryItem focusedGridItem() {
        BackpackGridPanel grid = activeGrid();
        if (grid == null) {
            return null;
        }
        double mx = mouseX();
        double my = mouseY();
        if (!grid.containsPoint(mx, my)) {
            return null;
        }
        var pos = grid.screenToGrid(mx, my);
        if (pos == null) {
            return null;
        }
        return grid.itemAt(pos.row(), pos.col());
    }

    /**
     * plan-interaction-intent-cleanup-v1 P1 — 仅对「已知有保鲜数据」的物品发感保鲜探针。
     *
     * <p>「已知有保鲜数据」= {@link FreshnessStore} 里有该 instance_id 的记录（server 曾
     * 主动推过 freshness_update）。普通物品没有保鲜数据，直接返回 false 不发包，杜绝对
     * 凡物的无效探针 + server 端「无时气流转」灰字刷屏。
     *
     * <p>package-private 以便测试断言「有记录→发包+音效」「无记录→不发包」。
     *
     * @return true 表示已发出探针（调用方据此 consume 按键）
     */
    boolean maybeProbeFreshness(InventoryItem item) {
        if (item == null) {
            return false;
        }
        if (FreshnessStore.get(Long.toString(item.instanceId())) == null) {
            return false;
        }
        com.bong.client.network.ClientRequestSender.sendFreshnessProbe(item.instanceId());
        // S2: plan §P1 触发音效 block.amethyst_block.chime pitch=1.2 / vol=0.25
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.player != null) {
            mc.player.playSound(
                SoundEvents.BLOCK_AMETHYST_BLOCK_CHIME,
                SoundCategory.PLAYERS,
                0.25f,
                1.2f
            );
        }
        return true;
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

        if (activeTab == TAB_SKILL && isOverSkillScrollDropZone(mouseX, mouseY)) {
            if (tryLearnSkillScroll(dragged)) {
                dragState.drop();
            } else {
                returnDragToSource();
            }
            clearAllHighlights();
            return;
        }

        // Loot grid drop (supply coffin) — handle both directions
        if (lootPanel != null && !lootPanel.isClosed()) {
            boolean fromLoot = lootPanel.extContainerId().equals(dragState.sourceContainerId());

            // Drop onto loot grid (from player container)
            BackpackGridPanel lg = lootPanel.lootGrid();
            if (lg.containsPoint(mouseX, mouseY)) {
                var pos = lg.screenToGrid(mouseX, mouseY);
                if (pos != null && lg.canPlace(dragged, pos.row(), pos.col())) {
                    lg.place(dragged, pos.row(), pos.col());
                    String srcCid = dragState.sourceContainerId();
                    int srcRow = dragState.sourceRow();
                    int srcCol = dragState.sourceCol();
                    dragState.drop();
                    lootPanel.sendMove(dragged.instanceId(),
                        srcCid != null ? srcCid : "", srcRow, srcCol,
                        lootPanel.extContainerId(), pos.row(), pos.col());
                    clearAllHighlights();
                    return;
                }
            }

            // Drop from loot grid onto active player grid
            if (fromLoot) {
                BackpackGridPanel destGrid = activeGrid();
                if (destGrid != null && destGrid.containsPoint(mouseX, mouseY)) {
                    var pos = destGrid.screenToGrid(mouseX, mouseY);
                    if (pos != null && destGrid.canPlace(dragged, pos.row(), pos.col())) {
                        destGrid.place(dragged, pos.row(), pos.col());
                        String srcCid = dragState.sourceContainerId();
                        int srcRow = dragState.sourceRow();
                        int srcCol = dragState.sourceCol();
                        dragState.drop();
                        lootPanel.sendMove(dragged.instanceId(),
                            srcCid, srcRow, srcCol,
                            destGrid.containerId(), pos.row(), pos.col());
                        clearAllHighlights();
                        return;
                    }
                }
            }
        }

        // plan-tarkov-floating-windows（交付物 #4）—— 拖入套包悬浮窗内含物（含跨窗拖移：两窗 grid 的
        // containerId 不同，dispatchMoveIntent 用各自 containerId 构 ContainerLoc）。
        // 发包硬约束：走 dispatchMoveIntent → sendInventoryMove，严禁 sendExternalContainerMove（loot 专用）。
        // 落位前过 isWornPackContainerDroppable 门控（owner 背包件须在携带面任意位置）。逆序遍历（z 最上优先）。
        var packWins = packWindows.ordered();
        for (int i = packWins.size() - 1; i >= 0; i--) {
            var win = packWins.get(i);
            if (win.isClosed()) {
                continue;
            }
            BackpackGridPanel wg = win.grid();
            if (wg == null || !wg.containsPoint(mouseX, mouseY)) {
                continue;
            }
            var pos = wg.screenToGrid(mouseX, mouseY);
            if (pos == null || !wg.canPlace(dragged, pos.row(), pos.col())) {
                continue;
            }
            if (!isWornPackContainerDroppable(
                    InventoryStateStore.snapshot(), wg.containerId())) {
                showActionToast("背包未在携带中，无法放入", ACTION_TOAST_WARN);
                returnDragToSource();
                clearAllHighlights();
                return;
            }
            wg.place(dragged, pos.row(), pos.col());
            dragState.drop();
            dispatchMoveIntent(dragged, fromLoc,
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    wg.containerId(), pos.row(), pos.col()));
            clearAllHighlights();
            return;
        }

        // Active grid
        BackpackGridPanel grid = activeGrid();
        if (grid != null && grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null && grid.canPlace(dragged, pos.row(), pos.col())) {
                // plan-tarkov-backpack-v1 P2（交付物 #3，穿戴态门控 client 侧）——
                // 拖入 `pack_<数字>` 套包容器前，校验其 owner 背包件仍穿戴在某身体槽 worn 层。
                // 非穿戴（已卸下的套包，server snapshot 仍含其容器但拒绝拖入）→ 不乐观落位 + 给 toast，
                // 避免「拖进去瞬间弹回且无提示」。与 server validate_move_semantics 的 Container 门控对齐。
                if (!isWornPackContainerDroppable(
                        InventoryStateStore.snapshot(), grid.containerId())) {
                    showActionToast("背包未在携带中，无法放入", ACTION_TOAST_WARN);
                    returnDragToSource();
                    clearAllHighlights();
                    return;
                }
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
        // plan-layered-equip-v1 P4（决议 #3/#12）：删除旧 swap 分支——满/占=飘红退回不顶替；
        // worn 合法则 push 栈顶（乐观更新，server 快照为权威）。canEquip 已含「worn 满 / held 占 / 锁手」拒绝。
        if (activeTab == TAB_EQUIP) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null) {
                if (eq.isDisabledByTwoHand() || !isEquipSlotDropValid(dragged, eq.slotType())) {
                    returnDragToSource();
                    clearAllHighlights();
                    return;
                }
                // 乐观本地落位：手槽 → held 单件；身体槽 → push worn 栈顶。
                EquipSlotType slot = eq.slotType();
                if (slot.isHand()) {
                    eq.setContents(com.bong.client.inventory.model.SlotContents.ofHeld(dragged));
                } else {
                    java.util.List<InventoryItem> stack =
                        new java.util.ArrayList<>(eq.contents().worn());
                    stack.add(dragged); // push 到栈顶（尾）
                    eq.setContents(new com.bong.client.inventory.model.SlotContents(stack, eq.held()));
                }
                dragState.drop();
                dispatchMoveIntent(dragged, fromLoc,
                    new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                        slot.name().toLowerCase(java.util.Locale.ROOT), slot.wireState()));
                clearAllHighlights();
                return;
            }
        }

        // Body inspect drop (physical or meridian layer) — only 1×1 items
        if (activeTab == TAB_CULTIVATION && bodyInspect != null
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
            // B 准则（快捷栏来源限制）：只有贴身口袋 / [快捷] 容器内物品（或快捷栏内重排）能指派到快捷栏。
            // 来源门控与物品形态规则（canPlaceIntoQuickUse，验形态/排盾）正交——此处单独把关来源。
            if (!isQuickAccessSource(dragState)) {
                showActionToast("只有贴身口袋或快捷背包内物品能指派到快捷栏", ACTION_TOAST_WARN);
                returnDragToSource();
                clearAllHighlights();
                return;
            }
            InventoryItem old = quickUseItems[qIdx];
            quickUseItems[qIdx] = dragged;
            setQuickUseSlotVisual(qIdx, dragged);
            commitQuickUseDrop(qIdx, dragged);
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
                    dragState.sourceEquipSlot().name().toLowerCase(java.util.Locale.ROOT),
                    dragState.sourceEquipSlot().wireState());
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
        if (!"guyuan_pill".equals(item.itemId())
            && !"huiyuan_pill".equals(item.itemId())
            && !"huiyuan_pill_forbidden".equals(item.itemId())) {
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
        if ("guyuan_pill".equals(item.itemId())
            || "huiyuan_pill".equals(item.itemId())
            || "huiyuan_pill_forbidden".equals(item.itemId())) {
            actions.add(new PillMenuAction("服用", ActionKind.SELF_USE));
        }
        if ("ningmai_powder".equals(item.itemId())) {
            actions.add(new PillMenuAction("外敷（选经脉）", ActionKind.MERIDIAN_TARGET));
        }
        if (forgeStationTier(item) > 0) {
            actions.add(new PillMenuAction("放置炼器砧", ActionKind.PLACE_FORGE_STATION));
        }
        if (isSpiritNicheBase(item)) {
            actions.add(new PillMenuAction("放置灵龛", ActionKind.PLACE_SPIRIT_NICHE));
        }
        if (isSpiritNicheRepairKit(item)) {
            actions.add(new PillMenuAction("修补灵龛", ActionKind.REPAIR_SPIRIT_NICHE));
        }
        if (item.isTechniqueScroll() && isKnownTechniqueScroll(item) && !isKnownTechnique(item)) {
            actions.add(new PillMenuAction("研读功法", ActionKind.TECHNIQUE_SCROLL_USE));
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
        skillBarContextMenu = null;
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
                if (tabLabels[TAB_EQUIP] != null && tabLabels[TAB_CULTIVATION] != null) {
                    switchTab(TAB_CULTIVATION);
                } else {
                    activeTab = TAB_CULTIVATION;
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
            case REPAIR_SPIRIT_NICHE -> {
                pendingMeridianUse = null;
                dispatchRepairSpiritNiche(item);
            }
            case TECHNIQUE_SCROLL_USE -> {
                pendingMeridianUse = null;
                dispatchTechniqueScrollUse(item);
            }
        }
    }

    boolean dispatchPlaceSpiritNiche(InventoryItem item) {
        BlockPos pos = targetPlacementPos();
        return dispatchPlaceSpiritNicheAt(item, pos.getX(), pos.getY(), pos.getZ());
    }

    boolean dispatchPlaceSpiritNicheAt(InventoryItem item, int x, int y, int z) {
        if (item == null || item.instanceId() == 0L || !isSpiritNicheBase(item)) {
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

    boolean dispatchRepairSpiritNiche(InventoryItem item) {
        BlockPos pos = targetPlacementPos();
        return dispatchRepairSpiritNicheAt(item, pos.getX(), pos.getY(), pos.getZ());
    }

    boolean dispatchRepairSpiritNicheAt(InventoryItem item, int x, int y, int z) {
        if (item == null || item.instanceId() == 0L || !isSpiritNicheRepairKit(item)) {
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchRepairSpiritNiche instance={} item={} pos=[{},{},{}]",
            item.instanceId(), item.itemId(), x, y, z);
        com.bong.client.network.ClientRequestSender.sendSpiritNicheRepair(
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

    static boolean isSpiritNicheBase(InventoryItem item) {
        return item != null && "niche_base".equals(item.itemId());
    }

    static boolean isSpiritNicheRepairKit(InventoryItem item) {
        return item != null && "niche_repair_kit".equals(item.itemId());
    }

    static boolean isBlockQuickBarBindable(InventoryItem item) {
        return item != null && !item.isEmpty() && BlockVanillaIconMap.isKnownBlockItem(item.itemId());
    }

    boolean openSkillBarContextMenu(InventoryItem item, int x, int y) {
        if (!isBlockQuickBarBindable(item)) {
            skillBarContextMenu = null;
            return false;
        }
        List<SkillBarMenuAction> actions = new ArrayList<>();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            actions.add(new SkillBarMenuAction("绑定到 " + (i + 1), i));
        }
        skillBarContextMenu = new SkillBarContextMenuState(item, x, y, List.copyOf(actions));
        pillContextMenu = null;
        pendingMeridianUse = null;
        return true;
    }

    boolean hasOpenSkillBarContextMenu() {
        return skillBarContextMenu != null;
    }

    void triggerSkillBarMenuAction(int slot) {
        if (skillBarContextMenu == null) return;
        InventoryItem item = skillBarContextMenu.item();
        skillBarContextMenu = null;
        bindBlockItemToSkillBar(slot, item);
    }

    boolean bindBlockItemToSkillBar(int slot, InventoryItem item) {
        if (slot < 0 || slot >= HOTBAR_SLOTS || !isBlockQuickBarBindable(item)) {
            return false;
        }
        // P3 — vanilla 方块物品（HOST_ITEMS 映射 / vanilla:<short>）的 iconTexture 留空，
        // 让 HUD 走 itemTexture 命令 → BongHud.drawItemTexture 内的 vanilla 原生方块图标分支，
        // 而非扁平 bong-client:textures/gui/items/<id>.png（对 vanilla:<short> 是非法路径 → 占位图）。
        // 非 vanilla 的 Bong 方块（若有）仍用扁平贴图路径。
        String iconTexture = BlockVanillaIconMap.usesVanillaItemIcon(item.itemId())
            ? null
            : ItemIconRegistry.itemTexturePath(item.itemId());
        com.bong.client.network.ClientRequestSender.sendSkillBarBindItem(slot, item.itemId());
        SkillBarStore.updateSlot(slot, SkillBarEntry.item(
            item.itemId(),
            item.displayName(),
            0,
            0,
            iconTexture
        ));
        if (hotbarSlots[slot] != null) {
            hotbarSlots[slot].setItem(item, true);
        }
        return true;
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
                // If source was the loot grid, return there
                if (lootPanel != null && !lootPanel.isClosed()
                        && lootPanel.extContainerId().equals(r.sourceContainerId())) {
                    BackpackGridPanel lg = lootPanel.lootGrid();
                    if (lg.canPlace(item, r.sourceRow(), r.sourceCol())) {
                        lg.place(item, r.sourceRow(), r.sourceCol());
                    } else {
                        // loot grid position taken — find free space
                        var freePos = lg.findFreeSpace(item);
                        if (freePos != null) lg.place(item, freePos.row(), freePos.col());
                        else placeItemAnywhere(item);
                    }
                } else {
                    // 先切回来源容器页（用户可能已手动/drag-hover 切走），再放回原位 —— 否则会落到
                    // 当前隐藏的网格，物品视觉上凭空消失（数据不丢、下次 snapshot 重同步，但体验差）。
                    switchToGridContainer(r.sourceContainerId());
                    BackpackGridPanel grid = activeGrid();
                    if (grid != null && grid.canPlace(item, r.sourceRow(), r.sourceCol()))
                        grid.place(item, r.sourceRow(), r.sourceCol());
                    else placeItemAnywhere(item);
                }
            }
            case EQUIP -> {
                if (r.sourceEquipSlot() != null) {
                    var slot = equipPanel.slotFor(r.sourceEquipSlot());
                    // 拖拽取消 → 把件还回来源槽（手槽 held / 身体槽 re-push 栈顶）。
                    if (slot != null) pushSlot(slot, item);
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
                    setQuickUseSlotVisual(idx, item);
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
        BackpackGridPanel cur = activeGrid();
        if (cur != null) {
            var pos = cur.findFreeSpace(item);
            if (pos != null) { cur.place(item, pos.row(), pos.col()); return; }
        }
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
        // 实在放不下 — 强制放进第一个容器第一格（body_pocket=index 0，覆盖避免数据丢失）。
        // 决议 #13：body_pocket 现为普通容器（containerGrids[0]），不再有独立 bodyPocketGrid 回落分支。
        if (containerGrids.length > 0) {
            containerGrids[0].place(item, 0, 0);
        }
    }

    // ==================== Highlights ====================

    private void updateHighlights(double mouseX, double mouseY) {
        clearAllHighlights();
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) return;

        if (activeTab == TAB_SKILL && skillScrollDropZone != null && isOverSkillScrollDropZone(mouseX, mouseY)) {
            setSkillScrollDropZoneState(skillScrollDropState(dragged));
        }

        BackpackGridPanel grid = activeGrid();
        if (grid != null && grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                boolean valid = grid.canPlace(dragged, pos.row(), pos.col());
                grid.highlightArea(pos.row(), pos.col(), dragged.gridWidth(), dragged.gridHeight(),
                    valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
            }
        }

        if (activeTab == TAB_EQUIP) {
            var eq = equipPanel.slotAtScreen(mouseX, mouseY);
            if (eq != null) {
                boolean valid = isEquipSlotDropValid(dragged, eq.slotType());
                eq.setHighlightState(valid
                    ? GridSlotComponent.HighlightState.VALID
                    : GridSlotComponent.HighlightState.INVALID);
            }
        }

        // Body inspect highlight
        if (activeTab == TAB_CULTIVATION && bodyInspect != null) {
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
            GridSlotComponent.HighlightState state = valid
                ? GridSlotComponent.HighlightState.VALID
                : GridSlotComponent.HighlightState.INVALID;
            if (quickUseSlots[qIdx] != null) quickUseSlots[qIdx].setHighlightState(state);
        }

        discardStrip.surface(Surface.flat(isOverDiscard(mouseX, mouseY) ? 0xFF331111 : 0xFF201010));

        // Loot grid highlight
        if (lootPanel != null && !lootPanel.isClosed()) {
            BackpackGridPanel lg = lootPanel.lootGrid();
            if (lg.containsPoint(mouseX, mouseY)) {
                var pos = lg.screenToGrid(mouseX, mouseY);
                if (pos != null) {
                    boolean valid = lg.canPlace(dragged, pos.row(), pos.col());
                    lg.highlightArea(pos.row(), pos.col(), dragged.gridWidth(), dragged.gridHeight(),
                        valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
                }
            }
        }
    }

    private void clearAllHighlights() {
        for (BackpackGridPanel g : containerGrids) g.clearHighlights();
        equipPanel.clearHighlights();
        for (int i = 0; i < HOTBAR_SLOTS; i++) {
            if (hotbarSlots[i] != null) hotbarSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
            if (quickUseSlots[i] != null) quickUseSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
        }
        if (bodyInspect != null) bodyInspect.clearHighlight();
        if (lootPanel != null && lootPanel.lootGrid() != null) lootPanel.lootGrid().clearHighlights();
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
        if ((item.isSkillScroll() && isKnownSkillScroll(item) && !isConsumedSkillScroll(item))
            || (item.isTechniqueScroll() && isKnownTechniqueScroll(item) && !isKnownTechnique(item))) {
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
        if (item.isTechniqueScroll()) {
            return tryReadTechniqueScroll(item);
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

    private boolean tryReadTechniqueScroll(InventoryItem item) {
        if (!isKnownTechniqueScroll(item)) {
            skillScrollDropFeedback = "不识此法，暂不能悟";
            return false;
        }
        if (isKnownTechnique(item)) {
            skillScrollDropFeedback = "你已通晓此法";
            return false;
        }
        skillScrollDropFeedback = "已送出研读请求";
        dispatchTechniqueScrollUse(item);
        return true;
    }

    boolean dispatchTechniqueScrollUse(InventoryItem item) {
        if (item == null || item.instanceId() == 0L || !item.isTechniqueScroll() || !isKnownTechniqueScroll(item)) {
            return false;
        }
        com.bong.client.cultivation.TechniqueScrollReadScreen.showReadRequested(
            item.displayName(),
            System.currentTimeMillis()
        );
        com.bong.client.network.ClientRequestSender.sendTechniqueScrollUse(item.instanceId());
        return true;
    }

    private boolean isKnownSkillScroll(InventoryItem item) {
        return com.bong.client.skill.SkillId.fromWire(item.scrollSkillId()) != null;
    }

    private boolean isKnownTechniqueScroll(InventoryItem item) {
        return com.bong.client.cultivation.TechniqueScrollReadScreen.isWoliuTechniqueId(item.scrollSkillId());
    }

    private boolean isKnownTechnique(InventoryItem item) {
        String techniqueId = item == null ? "" : item.scrollSkillId();
        return com.bong.client.combat.inspect.TechniquesListPanel.snapshot().stream()
            .anyMatch(technique -> technique.id().equals(techniqueId));
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
            // 决议 #17：TWO_HAND 专槽删除，双手武器走 MAIN_HAND（右手可用性）。
            case MAIN_HAND -> pb.canUseHand(PhysicalBody.Side.RIGHT);
            case OFF_HAND -> pb.canUseHand(PhysicalBody.Side.LEFT);
            default -> true; // EXTRA_HAND_0/1 多臂槽暂不受体表断臂限制
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

    /**
     * plan-layered-equip-v1 P4：从当前面板各槽收集分层装备态（SlotContents）供 canEquip 校验。
     * 拖拽来源槽若来自装备槽，把 dragged 件计回该槽（同槽重排放宽，与 canEquip sourceSlot 约定一致）。
     */
    private EnumMap<EquipSlotType, com.bong.client.inventory.model.SlotContents> equippedStateForValidation(
        InventoryItem dragged,
        EquipSlotType sourceSlot
    ) {
        EnumMap<EquipSlotType, com.bong.client.inventory.model.SlotContents> equipped =
            new EnumMap<>(EquipSlotType.class);
        for (EquipSlotType type : EquipSlotType.values()) {
            var slot = equipPanel.slotFor(type);
            if (slot == null || slot.isEmpty()) continue;
            equipped.put(type, slot.contents());
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
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
        var anchor = grid.anchorOf(item);
        if (anchor == null) return;

        var equipped = equippedStateForValidation(null, null);
        // 决议 #8/#17：treasure 不再有专属 belt 槽，与武器/工具同走手槽（off_hand 优先）；激活态另由灵宝 UI 触发位承载。
        EquipSlotType targetSlot = InventoryEquipRules.preferredWeaponQuickEquipSlot(
            item,
            equipped,
            this::isEquipSlotUsable
        );
        if (targetSlot == null) return;

        grid.remove(item);
        // 手槽 quick-equip → held 单件。
        equipPanel.slotFor(targetSlot).setContents(
            com.bong.client.inventory.model.SlotContents.ofHeld(item));
        dispatchMoveIntent(
            item,
            new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                grid.containerId(),
                anchor.row(),
                anchor.col()
            ),
            new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                targetSlot.name().toLowerCase(java.util.Locale.ROOT), targetSlot.wireState()
            )
        );
    }

    private void quickUnequipToGrid(EquipSlotType slotType, InventoryItem item) {
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
        var pos = grid.findFreeSpace(item);
        if (pos != null) {
            // 决议 #12：卸下仅弹出栈顶/held（被压住下层不动）。
            popSlotTop(equipPanel.slotFor(slotType));
            grid.place(item, pos.row(), pos.col());
            dispatchMoveIntent(
                item,
                new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                    slotType.name().toLowerCase(java.util.Locale.ROOT), slotType.wireState()
                ),
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    grid.containerId(),
                    pos.row(),
                    pos.col()
                )
            );
        }
    }

    private void quickMoveHotbarToGrid(int index) {
        InventoryItem item = hotbarItems[index];
        if (item == null) return;
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
        var pos = grid.findFreeSpace(item);
        if (pos != null) {
            hotbarItems[index] = null;
            hotbarSlots[index].clearItem();
            grid.place(item, pos.row(), pos.col());
            dispatchMoveIntent(
                item,
                new com.bong.client.network.ClientRequestProtocol.HotbarLoc(index),
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    grid.containerId(),
                    pos.row(),
                    pos.col()
                )
            );
        }
    }

    private void quickMoveQuickUseToGrid(int index) {
        InventoryItem item = quickUseItems[index];
        if (item == null) return;
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
        var pos = grid.findFreeSpace(item);
        if (pos != null) {
            quickUseItems[index] = null;
            setQuickUseSlotVisual(index, null);
            publishQuickUseSlot(index, null);
            grid.place(item, pos.row(), pos.col());
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
        drawSkillBarMenuOverlay(context, mouseX, mouseY);
        drawPendingMeridianPrompt(context);

        // Body inspect tooltip — drawn here to escape owo-lib component clipping
        if (activeTab == TAB_CULTIVATION && bodyInspect != null) {
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

            var matrices = context.getMatrices();
            matrices.push();
            matrices.translate(0, 0, 200);

            int fitSize = Math.min(gw, gh);
            int fitX = mouseX - fitSize / 2, fitY = mouseY - fitSize / 2;

            // P3 — 拖拽中的 vanilla 方块物品用原生方块图标跟随光标（与落点槽位图标一致）；
            // 非 vanilla 物品走原扁平贴图 + 0.75 幽灵透明。
            if (!BlockVanillaIconMap.drawVanillaIcon(context, item.itemId(), fitX, fitY, fitSize)) {
                Identifier tex = GridSlotComponent.textureIdForItem(item);
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
            }
            matrices.pop();
        }

        if (draggedTechnique != null) {
            drawTechniqueDragGhost(context, (int) techniqueDragX, (int) techniqueDragY);
        }

        // 左下角功法名：绑定功法图标都一样（缺专属贴图，全 fallback 同一张残卷），
        // 拖拽中 / hover 已绑槽时在左下角标出名字以便分辨。
        String cornerName = cornerTechniqueName(mouseX, mouseY);
        if (cornerName != null) {
            drawCornerTechniqueName(context, cornerName);
        }
    }

    /** 左下角要显示的功法名：优先拖拽中的功法，其次 hover 到的已绑定 1-9 槽；都没有则 null。 */
    private String cornerTechniqueName(int mouseX, int mouseY) {
        if (draggedTechnique != null) {
            return draggedTechnique.displayName();
        }
        if (activeTab != TAB_TECHNIQUES) {
            return null;
        }
        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0) {
            SkillBarEntry entry = SkillBarStore.snapshot().slot(hIdx);
            if (entry != null && entry.kind() == SkillBarEntry.Kind.SKILL && !entry.displayName().isBlank()) {
                return entry.displayName();
            }
        }
        return null;
    }

    /** 在屏幕左下角画一条带半透明底的功法名。 */
    private void drawCornerTechniqueName(DrawContext context, String name) {
        int x = 6;
        int y = this.height - 14;
        int boxW = textRenderer.getWidth(name) + 8;
        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 400);
        context.fill(x - 3, y - 3, x - 3 + boxW, y + textRenderer.fontHeight + 1, 0xC8101010);
        context.drawText(textRenderer, Text.literal(name), x, y, 0xFFE0B060, true);
        matrices.pop();
    }

    /** 功法拖拽跟手 ghost：跟随光标的功法名小标签；锁定态用红框 + ✕ 提示不可绑定。 */
    private void drawTechniqueDragGhost(DrawContext context, int mouseX, int mouseY) {
        String label = draggedTechnique.displayName();
        int padX = 4;
        int padY = 3;
        int boxW = textRenderer.getWidth(label) + padX * 2;
        int boxH = textRenderer.fontHeight + padY * 2;
        int x = mouseX + 10;
        int y = mouseY - boxH / 2;
        int border = draggedTechniqueLocked ? 0xFFCC5555 : 0xFFE0B060;
        int textColor = draggedTechniqueLocked ? 0xFFCC8888 : 0xFFE8E0D0;

        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 400);
        context.fill(x - 1, y - 1, x + boxW + 1, y + boxH + 1, border);
        context.fill(x, y, x + boxW, y + boxH, 0xE8181410);
        context.drawText(textRenderer, Text.literal(label), x + padX, y + padY, textColor, false);
        if (draggedTechniqueLocked) {
            context.drawText(textRenderer, Text.literal("✕"), x + boxW + 3, y + padY, 0xFFFF5555, false);
        }
        matrices.pop();
    }

    private void drawMultiCellItems(DrawContext context) {
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
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
        int fitSize = Math.min(dw, dh);
        int ox = (dw - fitSize) / 2, oy = (dh - fitSize) / 2;

        // P3 — vanilla 方块物品走原生方块图标，与单格槽/HUD 一致；非 vanilla 回退扁平贴图。
        if (BlockVanillaIconMap.drawVanillaIcon(ctx, item.itemId(), dx + ox, dy + oy, fitSize)) {
            return;
        }

        Identifier tex = GridSlotComponent.textureIdForItem(item);

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

    private int skillBarMenuHeight() {
        return skillBarContextMenu == null ? 0 : PILL_MENU_PADDING * 2 + skillBarContextMenu.actions().size() * PILL_MENU_ROW_HEIGHT;
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

    private int skillBarMenuActionIndexAt(double mouseX, double mouseY) {
        if (skillBarContextMenu == null) return -1;
        int left = skillBarContextMenu.x();
        int top = skillBarContextMenu.y();
        int height = skillBarMenuHeight();
        if (mouseX < left || mouseX >= left + PILL_MENU_WIDTH || mouseY < top || mouseY >= top + height) {
            return -1;
        }
        int row = ((int) mouseY - top - PILL_MENU_PADDING) / PILL_MENU_ROW_HEIGHT;
        return row >= 0 && row < skillBarContextMenu.actions().size() ? row : -1;
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

    private void drawSkillBarMenuOverlay(DrawContext context, int mouseX, int mouseY) {
        if (skillBarContextMenu == null) return;
        int left = skillBarContextMenu.x();
        int top = skillBarContextMenu.y();
        int height = skillBarMenuHeight();
        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 450);
        context.fill(left, top, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BG);
        context.fill(left, top, left + PILL_MENU_WIDTH, top + 1, PILL_MENU_BORDER);
        context.fill(left, top + height - 1, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BORDER);
        context.fill(left, top, left + 1, top + height, PILL_MENU_BORDER);
        context.fill(left + PILL_MENU_WIDTH - 1, top, left + PILL_MENU_WIDTH, top + height, PILL_MENU_BORDER);
        int hovered = skillBarMenuActionIndexAt(mouseX, mouseY);
        var textRenderer = MinecraftClient.getInstance().textRenderer;
        for (int i = 0; i < skillBarContextMenu.actions().size(); i++) {
            int rowTop = top + PILL_MENU_PADDING + i * PILL_MENU_ROW_HEIGHT;
            if (i == hovered) {
                context.fill(left + 1, rowTop, left + PILL_MENU_WIDTH - 1, rowTop + PILL_MENU_ROW_HEIGHT, PILL_MENU_HOVER);
            }
            context.drawTextWithShadow(
                textRenderer,
                Text.literal(skillBarContextMenu.actions().get(i).label()),
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
        if (grid != null && grid.containsPoint(mx, my)) {
            var pos = grid.screenToGrid(mx, my);
            if (pos != null) hovered = grid.itemAt(pos.row(), pos.col());
        }
        if (hovered == null && activeTab == TAB_EQUIP) {
            var eq = equipPanel.slotAtScreen(mx, my);
            if (eq != null) hovered = eq.representative();
        }
        if (hovered == null && activeTab == TAB_CULTIVATION && bodyInspect != null) {
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
        skillBarContextMenu = null;
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
                    popSlotTop(equipPanel.slotFor(menu.slotType()));
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
            new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                slotType.name().toLowerCase(java.util.Locale.ROOT), slotType.wireState())
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

    // ==================== Loot panel mount/unmount ====================

    private void mountLootPanelIfActive() {
        LootContainerStateStore.Session session = LootContainerStateStore.current();
        if (!(session instanceof LootContainerStateStore.OpenSession open)) return;
        if (lootPanel != null && !lootPanel.isClosed()) return; // already mounted

        unmountLootPanel(); // clean up any stale panel
        lootPanel = new LootContainerPanel(open);
        lootPanelLayout = lootPanel.build();
        // Insert before discardStrip
        int discardIdx = outerRow.children().indexOf(discardStrip);
        if (discardIdx >= 0) {
            outerRow.child(discardIdx, lootPanelLayout);
        } else {
            outerRow.child(lootPanelLayout);
        }
    }

    private void unmountLootPanel() {
        if (lootPanel != null) {
            lootPanel.dispose();
            lootPanel = null;
        }
        if (lootPanelLayout != null && outerRow != null) {
            outerRow.removeChild(lootPanelLayout);
            lootPanelLayout = null;
        }
    }

    // ==================== Worn container panel mount/unmount (P3) ============

    /**
     * plan-tarkov-backpack-v1 P3（交付物 #2/#3）—— 打开穿戴背包件的内含物视图。
     *
     * <p>由双击装备槽内的容器件触发。owner instance_id 来自被双击件，容器 id 约定为
     * {@code pack_<instance_id>}（与 server {@code container_id_for_worn_pack} 一致）。已挂同一容器
     * 则忽略；切换到另一个背包件则先卸旧再挂新。挂在 outerRow 的 discardStrip 之前（同 loot）。</p>
     */
    private void openWornContainerPanel(InventoryItem packItem, EquipSlotType slotType) {
        if (packItem == null || root == null) return;
        String containerId = "pack_" + packItem.instanceId();
        // 已开则置顶（去重）；否则新建悬浮窗，初始坐标错开堆叠（占位策略，后续真机再调）。
        int n = packWindows.size();
        int anchorX = 8 + 16 * n;
        int anchorY = 8 + 16 * n;
        packWindows.open(containerId, this.model, anchorX, anchorY);
    }

    /** mouseClicked 顶部命中悬浮窗的三态结果。 */
    private enum PackWindowClick {
        /** 未命中任何悬浮窗 → 继续主 grid/equip 流程。 */
        NONE,
        /** 命中并已处理（关闭 / grid 拾取 / 空白置顶）→ 吞掉本次点击。 */
        CONSUMED,
        /** 命中标题栏（拖动起手）→ 已置顶，交回 owo 做 focus + 后续 onMouseDrag 移位。 */
        RAISED
    }

    /**
     * plan-tarkov-floating-windows：mouseClicked 顶部对悬浮窗的命中分流。逆序遍历（map 末尾=z 最上）：
     * <ul>
     *   <li>命中 ✕ → 关闭该窗（若正从它拖拽则先取消）→ CONSUMED；</li>
     *   <li>左键命中 grid → 从该窗拾取物品（拖出）+ 置顶 → CONSUMED；</li>
     *   <li>命中标题栏 / 非 grid 空白 → 置顶 → RAISED（交回 owo 处理拖动）。</li>
     * </ul>
     * 命中即停（不穿透到主 grid）。
     */
    private PackWindowClick handlePackWindowMouseDown(double mouseX, double mouseY, int button) {
        var wins = packWindows.ordered();
        for (int i = wins.size() - 1; i >= 0; i--) {
            var win = wins.get(i);
            if (win.isClosed() || !win.isInBoundingBox(mouseX, mouseY)) {
                continue;
            }
            // ✕ 关闭按钮（任意键）。
            if (win.isOverCloseButton(mouseX, mouseY)) {
                if (dragState.isDragging()
                        && win.containerId().equals(dragState.sourceContainerId())) {
                    dragState.cancel();
                    clearAllHighlights();
                }
                packWindows.close(win.containerId());
                return PackWindowClick.CONSUMED;
            }
            // 左键命中内含物 grid → 拾取（拖出），随后置顶。
            if (button == 0) {
                BackpackGridPanel wg = win.grid();
                if (wg != null && wg.containsPoint(mouseX, mouseY)) {
                    var pos = wg.screenToGrid(mouseX, mouseY);
                    if (pos != null) {
                        InventoryItem item = wg.itemAt(pos.row(), pos.col());
                        if (item != null && dragState.phase() == DragState.Phase.IDLE) {
                            var anchor = wg.anchorOf(item);
                            if (anchor != null) {
                                dragState.pickup(item, wg.containerId(),
                                    anchor.row(), anchor.col());
                                wg.remove(item);
                            }
                        }
                    }
                    packWindows.raise(win);
                    return PackWindowClick.CONSUMED; // grid 区命中即吞，不穿透主 grid。
                }
            }
            // 标题栏 / 非 grid 空白 → 置顶后交回 owo（focus → 后续 onMouseDrag 拖动窗口）。
            packWindows.raise(win);
            return PackWindowClick.RAISED;
        }
        return PackWindowClick.NONE;
    }

    // ==================== Block picker panel mount/unmount (P5 §8.1#5) ====

    /**
     * plan-worldgen-v4 P5 §8.1#5 — 仅 dev/creative 时挂载方块审阅浮窗。
     *
     * <p>对齐 HUD conditional display：非 dev/creative 完全不挂载（隐藏而非灰态）。
     * 挂在 outerRow 末尾（discardStrip 之后）。</p>
     */
    private void mountBlockPickerPanelIfDev() {
        if (outerRow == null) return;
        if (!com.bong.client.block.BlockPickerGate.isDevOrCreative()) return;
        if (blockPickerPanel != null) return; // already mounted
        blockPickerPanel = BlockPickerPanel.fromVanillaRegistry();
        blockPickerPanelLayout = blockPickerPanel.build();
        outerRow.child(blockPickerPanelLayout);
    }

    private void unmountBlockPickerPanel() {
        if (blockPickerPanelLayout != null && outerRow != null) {
            outerRow.removeChild(blockPickerPanelLayout);
        }
        blockPickerPanelLayout = null;
        blockPickerPanel = null;
    }

    /**
     * 测试 seam：mount 决策与 {@link com.bong.client.block.BlockPickerGate#isDevOrCreative()}
     * 等价（隐藏而非灰态）。headless 单测据此锁定 dev-gate，不必跑 owo build()。
     */
    static boolean shouldMountBlockPickerForTests() {
        return com.bong.client.block.BlockPickerGate.isDevOrCreative();
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
