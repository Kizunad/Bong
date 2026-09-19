package com.bong.client.inventory;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.block.BlockVanillaIconMap;
import com.bong.client.craft.CraftContext;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.hud.SwordBondHudState;
import com.bong.client.hud.SwordBondHudStateStore;
import com.bong.client.inspect.ItemInspectClickTracker;
import com.bong.client.ui.window.UiWindowRuntime;
import com.bong.client.ui.adapter.owo.OwoXmlTemplateRegistry;
import io.wispforest.owo.ui.component.ButtonComponent;
import com.bong.client.inventory.component.*;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.DragState;
import com.bong.client.inventory.state.InventoryStateStore;
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
    private static final int HOTBAR_SLOTS = SkillBarConfig.SLOT_COUNT;

    private static final int TAB_ACTIVE_COLOR = 0xFFCCCCCC;
    private static final int TAB_INACTIVE_COLOR = 0xFF555555;
    private static final int TAB_EQUIP = 0;
    private static final int TAB_CULTIVATION = 1;
    private static final int TAB_PRACTICE = 2;
    private static final int TAB_CRAFT = 3;
    private static final String[] TAB_NAMES = {"随身", "修仙", "修习", "制作"};
    private static final int ACTION_TOAST_OK = 0xFFA8E6CF;
    private static final int ACTION_TOAST_WARN = 0xFFFFAA55;
    private static final long ACTION_TOAST_MS = 2_200L;

    private InventoryModel model;
    private final DragState dragState = new DragState();
    private final ItemInspectClickTracker itemInspectClicks = new ItemInspectClickTracker();
    private boolean startingItemDrag;
    /** Screen 存活期间持有的 InventoryStateStore 订阅，close 时解绑避免泄漏。 */
    private Consumer<InventoryModel> inventoryListener;

    // --- Container grids (driven by model.containers()) ---
    private BackpackGridPanel[] containerGrids = new BackpackGridPanel[0];
    private int containerCount;
    // 保存最近操作的容器，供快捷转移选择目标；无容器时为 -1。
    private int activeContainer = -1;
    // 口袋和穿戴容器的常驻入口；套包可由容器物品右键打开。
    private java.util.List<InventoryModel.ContainerDef> filteredContainerDefs = java.util.List.of();
    private FlowLayout containerSection;

    private InventoryLoadoutWindows loadout = new InventoryLoadoutWindows();
    private EquipmentPanel equipPanel;
    private BottomInfoBar bottomBar;
    // buff/状态效果条 —— 所有 tab 常驻（挂在 mainPanel，非某个 tab 专属内容），
    // 无 buff 时自行收起为 0 高度不占位。
    private BuffBarPanel buffBarPanel;

    // Tabs (left panel)
    private int activeTab = TAB_EQUIP;
    private final LabelComponent[] tabLabels = new LabelComponent[TAB_NAMES.length];
    private FlowLayout equipTabContent;
    private FlowLayout cultivationTabContent;
    private FlowLayout craftTabContent;
    private String skillScrollDropFeedback = "仅 skill 残卷可悟";

    // Hotbar
    private GridSlotComponent[] hotbarSlots = loadout.hotbarSlots;
    private InventoryItem[] hotbarItems = loadout.hotbarItems;

    // 物品快捷栏当前开放 F1/F2，与下方技能栏容量独立。
    private GridSlotComponent[] quickUseSlots = loadout.quickUseSlots;
    private InventoryItem[] quickUseItems = loadout.quickUseItems;
    private Consumer<QuickUseSlotStore.Update> quickUseStoreListener;
    private Consumer<SkillBarConfig> skillBarStoreListener;
    private long lastQuickUseUpdateSequence = -1L;
    private PendingQuickUseIntent pendingQuickUseIntent;

    private record PendingQuickUseIntent(
        String requestId,
        int slot,
        Long expectedInstanceId,
        Runnable onAccepted,
        Runnable onRejected
    ) {}

    // Discard
    private FlowLayout discardStrip;

    // Loot panel (supply coffin — mounted into outerRow when session active)
    private FlowLayout outerRow;
    private LootContainerPanel lootPanel;
    private FlowLayout lootPanelLayout;
    private LootContainerStateStore.Listener lootStoreListener;


    // Block picker panel (plan-worldgen-v4 P5 §8.1#5 — dev-only 方块审阅浮窗)
    private BlockPickerPanel blockPickerPanel;
    private FlowLayout blockPickerPanelLayout;

    // Body inspect (cultivation tab) — dual-layer: physical + meridian
    private BodyInspectComponent bodyInspect;
    private BodyInspectComponent.Layer initialBodyLayer;

    /** 远端语义入口使用 Inspect 工作台，窗口在工作台初始化后打开。 */
    public InspectScreen withBodyWindow(BodyInspectComponent.Layer layer) {
        initialBodyLayer = layer;
        return this;
    }

    public void openBodyWindow(BodyInspectComponent.Layer layer) {
        UiWindowRuntime.openBody(layer);
        bodyInspect = UiWindowRuntime.body(layer);
    }

    record PillMenuAction(String label, ActionKind kind) {}
    enum ActionKind { SELF_USE, MERIDIAN_TARGET, PLACE_FORGE_STATION, PLACE_SPIRIT_NICHE, REPAIR_SPIRIT_NICHE, TECHNIQUE_SCROLL_USE, CRAFT_RECIPE_SCROLL_USE, READ_SCROLL }
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
        loadout.populate(this.model);
    }

    @Override
    public void removed() {
        itemInspectClicks.cancel();
        UiWindowRuntime.cancelInput();
        // Screen 被关闭时解绑背包和技艺订阅；模型内容由窗口 scope 持有。
        if (inventoryListener != null) {
            InventoryStateStore.removeListener(inventoryListener);
            inventoryListener = null;
        }
        unregisterAuthoritativeBarListeners();
        // Loot panel cleanup — send close to server if still active
        if (lootPanel != null && !lootPanel.isClosed()) {
            lootPanel.sendClose();
        }
        unmountLootPanel();
        if (lootStoreListener != null) {
            LootContainerStateStore.removeListener(lootStoreListener);
            lootStoreListener = null;
        }
        if (dragState.isDragging()) returnDragToSource();
        unmountBlockPickerPanel();
        super.removed();
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface((context, component) -> UiWindowRuntime.renderBackground(context, component.width(), component.height()));
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        // Outermost: [hotbar] [main] [discard]
        outerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        outerRow.gap(2);
        outerRow.verticalAlignment(VerticalAlignment.CENTER);

        loadout = UiWindowRuntime.loadout();
        equipPanel = loadout.equipment();
        hotbarSlots = loadout.hotbarSlots;
        hotbarItems = loadout.hotbarItems;
        quickUseSlots = loadout.quickUseSlots;
        quickUseItems = loadout.quickUseItems;
        registerAuthoritativeBarListeners(task -> MinecraftClient.getInstance().execute(task));

        // === CENTER: Main panel ===
        FlowLayout mainPanel = Containers.verticalFlow(Sizing.content(), Sizing.content());
        mainPanel.surface(Surface.flat(0xFF1A1A1A));
        mainPanel.padding(Insets.of(4));
        mainPanel.gap(2);

        // buff/状态效果条 —— 挂在所有 tab 内容之前，横条常驻不随 tab 切换消失；
        // 无 buff 时面板自行收起为 0 高度（BuffBarPanel 内部 sizing），不留灰色空壳。
        buffBarPanel = new BuffBarPanel();
        mainPanel.child(buffBarPanel);

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

        // 装备和快捷槽只保留统一窗口入口。
        equipTabContent = OwoXmlTemplateRegistry.production().require("inventory-equipment")
            .expandTemplate(FlowLayout.class, "launcher", java.util.Map.of());
        equipTabContent.childById(ButtonComponent.class, "open-equipment")
            .onPress(ignored -> UiWindowRuntime.openLoadout(InventoryLoadoutWindows.EQUIPMENT));
        equipTabContent.childById(ButtonComponent.class, "open-shortcuts")
            .onPress(ignored -> UiWindowRuntime.openLoadout(InventoryLoadoutWindows.SHORTCUTS));
        leftCol.child(equipTabContent);

        // 心·身·境只保留入口，模型、详情和修炼操作由各自窗口持有。
        cultivationTabContent = OwoXmlTemplateRegistry.production().require("body-inspect")
            .expandTemplate(FlowLayout.class, "launcher", java.util.Map.of());
        cultivationTabContent.childById(LabelComponent.class, "open-physical-body").mouseDown().subscribe((mx, my, btn) -> {
            if (btn != 0) return false;
            openBodyWindow(BodyInspectComponent.Layer.PHYSICAL);
            return true;
        });
        cultivationTabContent.childById(LabelComponent.class, "open-meridians").mouseDown().subscribe((mx, my, btn) -> {
            if (btn != 0) return false;
            openBodyWindow(BodyInspectComponent.Layer.MERIDIAN);
            return true;
        });

        leftCol.child(cultivationTabContent);
        cultivationTabContent.positioning(Positioning.absolute(-9999, -9999));

        // 手搓标签只负责打开窗口，业务会话不属于 Inspect Screen。
        craftTabContent = buildCraftTabEntryContent();
        leftCol.child(craftTabContent);
        craftTabContent.positioning(Positioning.absolute(-9999, -9999));

        middle.child(leftCol);

        // 经脉详情直接绘制在 body 画布内部（见 BodyInspectComponent.drawMeridianDetailInline）
        // 不再作为独立组件，以免增加列宽/列高

        // -- Right column --
        FlowLayout rightCol = Containers.verticalFlow(Sizing.content(), Sizing.content());
        rightCol.gap(2);

        containerSection = OwoXmlTemplateRegistry.production().require("inventory-container")
            .expandTemplate(FlowLayout.class, "launcher", java.util.Map.of());
        rebuildContainerSection();
        rightCol.child(containerSection);

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
        UiWindowRuntime.openInventory(filteredContainerDefs);
        if (!UiWindowRuntime.manager().contains(UiWindowRuntime.manager().key(InventoryLoadoutWindows.EQUIPMENT.windowType(), "player"))) {
            UiWindowRuntime.openLoadout(InventoryLoadoutWindows.EQUIPMENT);
        }
        if (!UiWindowRuntime.manager().contains(UiWindowRuntime.manager().key(InventoryLoadoutWindows.SHORTCUTS.windowType(), "player"))) {
            UiWindowRuntime.openLoadout(InventoryLoadoutWindows.SHORTCUTS);
        }
        populateFromModel();

        // Server 增量到达时（InventoryEventHandler 写入或新 snapshot 落地）刷新 UI。
        // listener 在网络线程触发，UI mutation 必须回主线程。
        inventoryListener = next -> {
            if (next == null) return;
            MinecraftClient.getInstance().execute(() -> {
                if (inventoryListener == null) return;
                // 穿戴或容量变化时刷新入口；容器网格和窗口生命周期由统一 runtime 更新。
                boolean structuralChange = containerStructureChanged(next);
                this.model = next;
                if (structuralChange) {
                    rebuildContainerSection();
                }
                if (dragState.isDragging() && dragState.sourceKind() == DragState.SourceKind.GRID
                    && next.containers().stream().noneMatch(def -> def.id().equals(dragState.sourceContainerId()))
                    && (lootPanel == null || !lootPanel.extContainerId().equals(dragState.sourceContainerId()))) {
                    dragState.cancel();
                    itemInspectClicks.cancel();
                }
                populateFromModel();
            });
        };
        InventoryStateStore.addListener(inventoryListener);
        if (initialBodyLayer != null) {
            switchTab(TAB_CULTIVATION);
            openBodyWindow(initialBodyLayer);
            initialBodyLayer = null;
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


    private FlowLayout buildCraftTabEntryContent() {
        FlowLayout panel = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        panel.gap(4);
        panel.padding(Insets.of(4));
        panel.surface(Surface.flat(0xFF12121C).and(Surface.outline(0xFF4A4050)));
        panel.cursorStyle(CursorStyle.HAND);

        LabelComponent title = Components.label(Text.literal("制作"));
        title.color(Color.ofArgb(0xFFE8DDC4));
        panel.child(title);

        LabelComponent hint = Components.label(Text.literal("随身制作"));
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
        UiWindowRuntime.openCraft(CraftContext.HANDCRAFT);
    }

    private void hydrateQuickUseFromStore() {
        hydrateQuickUseFromConfig(QuickUseSlotStore.snapshot());
    }

    private void hydrateQuickUseFromConfig(QuickSlotConfig config) {
        loadout.hydrateQuickUse(config);
    }

    private void registerAuthoritativeBarListeners(Consumer<Runnable> executor) {
        unregisterAuthoritativeBarListeners();
        quickUseStoreListener = update -> executor.accept(() -> applyQuickUseUpdate(update));
        skillBarStoreListener = ignored -> executor.accept(this::hydrateSkillBarFromStore);
        QuickUseSlotStore.Update initial = QuickUseSlotStore.subscribeAndGet(quickUseStoreListener);
        SkillBarStore.addListener(skillBarStoreListener);
        executor.accept(() -> applyQuickUseUpdate(initial));
        executor.accept(this::hydrateSkillBarFromStore);
    }

    private void unregisterAuthoritativeBarListeners() {
        if (quickUseStoreListener != null) {
            QuickUseSlotStore.removeListener(quickUseStoreListener);
            quickUseStoreListener = null;
        }
        if (skillBarStoreListener != null) {
            SkillBarStore.removeListener(skillBarStoreListener);
            skillBarStoreListener = null;
        }
        lastQuickUseUpdateSequence = -1L;
        pendingQuickUseIntent = null;
    }

    private void applyQuickUseUpdate(QuickUseSlotStore.Update update) {
        if (update == null || update.sequence() <= lastQuickUseUpdateSequence) {
            return;
        }
        lastQuickUseUpdateSequence = update.sequence();
        hydrateQuickUseFromConfig(update.config());

        PendingQuickUseIntent pending = pendingQuickUseIntent;
        if (pending == null
                || update.source() != QuickUseSlotStore.Source.SERVER
                || update.ackRequestId() == null
                || !update.ackRequestId().equals(pending.requestId())
                || update.bindAccepted() == null) {
            return;
        }

        pendingQuickUseIntent = null;
        QuickSlotEntry confirmed = update.config() == null
            ? null
            : update.config().slot(pending.slot());
        boolean expectedState = pending.expectedInstanceId() == null
            ? confirmed == null
            : confirmed != null && pending.expectedInstanceId() == confirmed.instanceId();
        if (update.bindAccepted() && expectedState) {
            pending.onAccepted().run();
        } else {
            pending.onRejected().run();
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
        loadout.hydrateSkills();
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

    private boolean requestQuickUseSlot(
        int index,
        InventoryItem item,
        Runnable onAccepted,
        Runnable onRejected
    ) {
        if (!QuickSlotConfig.isAvailable(index) || pendingQuickUseIntent != null) {
            return false;
        }
        if (item != null && !InventoryEquipRules.canPlaceIntoQuickUse(item)) return false;
        Long instanceId = item == null ? null : item.instanceId();
        String requestId = com.bong.client.network.ClientRequestSender.sendQuickSlotBindTracked(
            index, instanceId);
        if (requestId == null) {
            return false;
        }
        pendingQuickUseIntent = new PendingQuickUseIntent(
            requestId,
            index,
            instanceId,
            onAccepted,
            onRejected
        );
        return true;
    }

    /** 绑定现有库存实例的快捷使用链接。 */
    boolean commitQuickUseDrop(int index, InventoryItem item) {
        return requestQuickUseSlot(index, item, () -> {}, () -> {});
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
        if (idx == TAB_PRACTICE) {
            UiWindowRuntime.openPractice();
            return;
        }
        if (idx == activeTab || idx < 0 || idx >= TAB_NAMES.length) return;
        activeTab = idx;
        FlowLayout[] tabs = {
            equipTabContent,
            cultivationTabContent,
            null,
            craftTabContent,
        };
        for (int i = 0; i < tabs.length; i++) {
            tabLabels[i].color(Color.ofArgb(i == idx ? TAB_ACTIVE_COLOR : TAB_INACTIVE_COLOR));
            if (tabs[i] != null) {
                tabs[i].positioning(i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999));
            }
        }
        if (idx == TAB_CRAFT) {
            openCraftScreen();
        }
    }

    // plan-layered-equip-v1 P4（决议 #19）：行囊重量条 backpackWeightBreakdown 删除——负重沿用整体
    // inventory 底部既有 BottomInfoBar（读 model.currentWeight()/maxWeight()，过载变红）。

    /**
     * owner 背包件是否当前穿戴在身体槽的 worn 层；穿戴容器显示常驻入口。
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
     * 口袋入口置首，穿戴容器随后；其他套包通过容器物品右键打开。
     * 穿脱只改变入口集合，已打开的窗口在权威快照移除容器时关闭。
     */
    public static java.util.List<InventoryModel.ContainerDef> computeContainerDefs(InventoryModel m) {
        java.util.List<InventoryModel.ContainerDef> defs = new java.util.ArrayList<>();
        InventoryModel.ContainerDef bodyPocketDef = null;
        for (var def : m.containers()) {
            if (InventoryModel.BODY_POCKET_CONTAINER_ID.equals(def.id())) {
                bodyPocketDef = def;
            } else if (parseWornPackInstance(def.id()).isPresent()) {
                Long owner = def.ownerInstanceId();
                if (owner != null && isPackOwnerInWornSlot(m, owner)) {
                    defs.add(def);
                }
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
     * 比较入口定义；仅内含物变化不重建入口，名称、容量或成员变化时刷新。
     */
    static boolean containerDefsDiffer(
            java.util.List<InventoryModel.ContainerDef> a,
            java.util.List<InventoryModel.ContainerDef> b) {
        if (a.size() != b.size()) return true;
        for (int i = 0; i < a.size(); i++) {
            var x = a.get(i);
            var y = b.get(i);
            if (!x.equals(y)) {
                return true;
            }
        }
        return false;
    }

    /** 穿戴容器入口或容量变化时重建入口列表。 */
    private boolean containerStructureChanged(InventoryModel next) {
        return containerDefsDiffer(computeContainerDefs(next), filteredContainerDefs);
    }

    /** 入口只请求打开统一窗口，不持有或重新挂载容器网格。 */
    private void rebuildContainerSection() {
        if (containerSection == null) return;
        filteredContainerDefs = computeContainerDefs(model);
        var entries = containerSection.childById(FlowLayout.class, "container-entries");
        entries.clearChildren();
        var template = OwoXmlTemplateRegistry.production().require("inventory-container");
        for (var def : filteredContainerDefs) {
            var button = template.expandTemplate(ButtonComponent.class, "container-entry", java.util.Map.of());
            button.setMessage(Text.literal(MinecraftClient.getInstance().textRenderer.trimToWidth(def.name(), 94)));
            button.tooltip(Text.literal(def.name()));
            button.onPress(ignored -> UiWindowRuntime.openContainer(def.id()));
            entries.child(button);
        }
    }

    /** 记录最近操作的容器；无匹配时保持当前目标。 */
    private void switchToGridContainer(String containerId) {
        if (containerId == null) return;
        for (int i = 0; i < containerCount; i++) {
            if (containerGrids[i].containerId().equals(containerId)) {
                activeContainer = i;
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

    /** 测试通过同一 loadout 刷新路径验证实例耗尽清槽、选中态复位与空快照保护。 */
    void reconcileSkillBarForTests(InventoryModel next) {
        if (next == null) return;
        this.model = next;
        loadout.populate(next);
    }

    private void populateFromModel() {
        UiWindowRuntime.containers().refresh();
        String selected = activeGrid() == null ? null : activeGrid().containerId();
        containerGrids = UiWindowRuntime.containers().grids().toArray(BackpackGridPanel[]::new);
        containerCount = containerGrids.length;
        activeContainer = containerCount == 0 ? -1 : 0;
        switchToGridContainer(InventoryModel.BODY_POCKET_CONTAINER_ID);
        switchToGridContainer(selected);
        if (dragState.isDragging() && dragState.sourceKind() == DragState.SourceKind.GRID) {
            var grid = gridById(dragState.sourceContainerId());
            if (grid != null) {
                for (var entry : grid.toGridEntries()) {
                    if (entry.item().instanceId() == dragState.draggedItem().instanceId()) grid.remove(entry.item());
                }
            }
        }

        loadout.populate(model);
        bottomBar.updateFromModel(model);
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

    /** Headless 交互测试 seam：注入生产 quick-equip / equip-drop 共用的 grid 与装备面板。 */
    void configureEquipInteractionForTests(BackpackGridPanel grid, EquipmentPanel panel) {
        this.containerGrids = grid == null ? new BackpackGridPanel[0] : new BackpackGridPanel[] {grid};
        this.activeContainer = grid == null ? -1 : 0;
        this.equipPanel = panel;
        this.activeTab = TAB_EQUIP;
    }

    /** 调用 mouseClicked 的 Shift 分支所用的同一生产方法。 */
    void quickEquipFromGridForTests(InventoryItem item) {
        quickEquipFromGrid(item);
    }

    boolean beginGridEquipDragForTests(BackpackGridPanel grid, InventoryItem item) {
        return beginGridDrag(grid, item);
    }

    BackpackGridPanel openPackGridForTests(String containerId) {
        var existing = gridById(containerId);
        if (existing != null) return existing;
        var def = model.containers().stream().filter(value -> value.id().equals(containerId)).findFirst().orElseThrow();
        var grid = new BackpackGridPanel(containerId, def.rows(), def.cols());
        grid.populateFromModel(model);
        containerGrids = java.util.Arrays.copyOf(containerGrids, containerGrids.length + 1);
        containerGrids[containerGrids.length - 1] = grid;
        containerCount = containerGrids.length;
        return grid;
    }

    boolean beginPackGridEquipDragForTests(String containerId, InventoryItem item) {
        BackpackGridPanel grid = openPackGridForTests(containerId);
        return beginGridDrag(grid, item);
    }

    InventoryItem packGridItemForTests(String containerId, int row, int col) {
        var grid = gridById(containerId);
        return grid == null ? null : grid.itemAt(row, col);
    }

    private BackpackGridPanel gridById(String id) {
        for (var grid : containerGrids) if (grid.containerId().equals(id)) return grid;
        return null;
    }

    /** Headless 回归复用真实 QUICK_USE 链接拾起语义。 */
    boolean beginQuickUseEquipDragForTests(InventoryItem item, int index) {
        quickUseItems[index] = item;
        return beginQuickUseDrag(index);
    }

    boolean bindQuickUseForTests(InventoryItem item, int index) {
        return requestQuickUseSlot(index, item, () -> {}, () -> {});
    }

    boolean beginQuickUseDragForTests(int index) {
        return beginQuickUseDrag(index);
    }

    boolean isDraggingForTests() {
        return dragState.isDragging();
    }

    void registerAuthoritativeBarListenersForTests() {
        registerAuthoritativeBarListeners(Runnable::run);
    }

    void unregisterAuthoritativeBarListenersForTests() {
        unregisterAuthoritativeBarListeners();
    }

    void returnCurrentDragToSourceForTests() {
        returnDragToSource();
    }

    /** Headless 回归复用 attemptDrop 的装备提交/失败回源编排。 */
    boolean commitCurrentDragToEquipForTests(EquipSlotType targetSlot) {
        InventoryItem dragged = dragState.draggedItem();
        return dragged != null && commitEquipDropOrReturnToSource(
            dragged,
            snapshotSourceLocation(),
            targetSlot
        );
    }

    InventoryItem quickUseItemForTests(int index) {
        return quickUseItems[index];
    }

    private static void showActionToast(String text, int color) {
        BongToast.show(text, color, System.currentTimeMillis(), ACTION_TOAST_MS);
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
        return UiWindowRuntime.hotbarAt(sx, sy);
    }

    private int quickUseSlotAtScreen(double sx, double sy) {
        return UiWindowRuntime.quickUseAt(sx, sy);
    }

    private boolean isOverDiscard(double sx, double sy) {
        return sx >= discardStrip.x() && sx < discardStrip.x() + discardStrip.width()
            && sy >= discardStrip.y() && sy < discardStrip.y() + discardStrip.height();
    }

    // ==================== Mouse interaction ====================

    @Override
    public void tick() {
        super.tick();
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
                itemInspectClicks.cancel();
                return true;
            }
            if (button == 1) {
                pillContextMenu = null;
                pendingMeridianUse = null;
                itemInspectClicks.cancel();
                return true;
            }
            return false;
        }

        if (skillBarContextMenu != null) {
            int actionIdx = skillBarMenuActionIndexAt(mouseX, mouseY);
            if (actionIdx >= 0) {
                triggerSkillBarMenuAction(skillBarContextMenu.actions().get(actionIdx).slot());
                itemInspectClicks.cancel();
                return true;
            }
            if (button == 1) {
                skillBarContextMenu = null;
                itemInspectClicks.cancel();
                return true;
            }
            return false;
        }

        if (weaponContextMenu != null) {
            int actionIdx = weaponMenuActionIndexAt(mouseX, mouseY);
            if (actionIdx >= 0) {
                triggerWeaponMenuAction(weaponContextMenu.actions().get(actionIdx).kind());
                itemInspectClicks.cancel();
                return true;
            }
            if (button == 1) {
                weaponContextMenu = null;
                itemInspectClicks.cancel();
                return true;
            }
            return false;
        }

        return false;
    }

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (handleContextMenuClick(mouseX, mouseY, button)) return true;
        if (button == 1 && pendingMeridianUse != null) {
            pendingMeridianUse = null;
            itemInspectClicks.cancel();
            return true;
        }
        var modelTarget = UiWindowRuntime.bodyAt(mouseX, mouseY);
        if (modelTarget != null) {
            bodyInspect = modelTarget;
            if (button == 0 && pendingMeridianUse != null && modelTarget.activeLayer() == BodyInspectComponent.Layer.MERIDIAN) {
                var target = modelTarget.channelAtScreen(mouseX, mouseY);
                if (target != null) modelTarget.setSelectedChannel(target);
                if (target != null && confirmPendingMeridianUse()) {
                    itemInspectClicks.cancel();
                    return true;
                }
            }
        }
        boolean shift = hasShiftDown();
        if (modelTarget != null && button == 0 && !dragState.isDragging()) {
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
                }
            }
        }


        var containerGrid = UiWindowRuntime.containerGridAt(mouseX, mouseY);
        boolean loadoutSlot = UiWindowRuntime.loadoutSlotAt(mouseX, mouseY);
        if (containerGrid != null) {
            UiWindowRuntime.focusContainerAt(mouseX, mouseY);
            uiAdapter.rootComponent.focusHandler().focus(null, Component.FocusSource.MOUSE_CLICK);
            switchToGridContainer(containerGrid.containerId());
        } else if (loadoutSlot) {
            UiWindowRuntime.focusLoadoutAt(mouseX, mouseY);
            uiAdapter.rootComponent.focusHandler().focus(null, Component.FocusSource.MOUSE_CLICK);
        } else if (UiWindowRuntime.mouseDown(mouseX, mouseY, button)) {
            uiAdapter.rootComponent.focusHandler().focus(null, Component.FocusSource.MOUSE_CLICK);
            itemInspectClicks.cancel();
            if (dragState.isDragging()) returnDragToSource();
            return true;
        }
        if (button != 0 || hasShiftDown()) itemInspectClicks.cancel();

        if (button == 0 && !hasShiftDown() && !startingItemDrag && !dragState.isDragging()
) {
            InventoryItem item = itemAtScreen(mouseX, mouseY);
            if (item != null) {
                if (itemInspectClicks.press(item.instanceId(), mouseX, mouseY, System.currentTimeMillis())) {
                    UiWindowRuntime.openItem(item.instanceId());
                }
                return true;
            }
            itemInspectClicks.cancel();
        }

        if (containerGrid != null) {
            var pos = containerGrid.screenToGrid(mouseX, mouseY);
            var item = pos == null ? null : containerGrid.itemAt(pos.row(), pos.col());
            if (item != null && button == 1) {
                pendingMeridianUse = null;
                if (InventoryEquipRules.isContainer(item)) openContainerItem(item);
                else if (!openPillContextMenu(item, (int) mouseX, (int) mouseY)) {
                    openSkillBarContextMenu(item, (int) mouseX, (int) mouseY);
                }
            } else if (item != null && button == 0 && !dragState.isDragging()) {
                if (hasShiftDown()) quickEquipFromGrid(item);
                else beginGridDrag(containerGrid, item);
            }
            return true;
        }

        if (button == 1) {
            {
                var eq = UiWindowRuntime.equipmentAt(mouseX, mouseY);
                // 决议 #12：仅栈顶/held（representative）可操作。
                InventoryItem top = eq == null ? null : eq.representative();
                // fix/tarkov-nest-persistence §C3 — 右键背包件直接开包（容器件对 weapon/pill 菜单
                // 都返回 false，故前置拦截）。覆盖装备槽持有位。
                if (eq != null && top != null && InventoryEquipRules.isContainer(top)) {
                    openContainerItem(top);
                    itemInspectClicks.cancel();
                    return true;
                }
                if (eq != null && top != null && openWeaponContextMenu(eq.slotType(), top, (int) mouseX, (int) mouseY)) {
                    itemInspectClicks.cancel();
                    return true;
                }
            }

            // plan-exploration-probe-return-v1 P1 fix(M1-②): 先清理 pendingMeridianUse 状态，
            // 防止 Shift+右键触发保鲜探针时残留旧的「待选经脉外敷���状态导致后续误触发。
            if (pendingMeridianUse != null) {
                pendingMeridianUse = null;
                itemInspectClicks.cancel();
                return true;
            }

            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            // fix/tarkov-nest-persistence §C3 — 右键快捷栏背包件直接开包（前置拦截）。
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && InventoryEquipRules.isContainer(hotbarItems[hIdx])) {
                openContainerItem(hotbarItems[hIdx]);
                itemInspectClicks.cancel();
                return true;
            }
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && openPillContextMenu(hotbarItems[hIdx], (int) mouseX, (int) mouseY)) {
                itemInspectClicks.cancel();
                return true;
            }
            if (hIdx >= 0 && hotbarItems[hIdx] != null
                    && openSkillBarContextMenu(hotbarItems[hIdx], (int) mouseX, (int) mouseY)) {
                itemInspectClicks.cancel();
                return true;
            }
            int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
            if (qIdx >= 0 && quickUseItems[qIdx] != null) {
                com.bong.client.network.ClientRequestSender.sendUseQuickSlot(qIdx);
                return true;
            }
            // 右键【已绑定功法】的 1-9 槽 → 清空解绑（绑定功法不写 hotbarItems[]，单独走 SkillBarStore）。
            if (hIdx >= 0) {
                SkillBarEntry bound = SkillBarStore.snapshot().slot(hIdx);
                if (bound != null && bound.kind() == SkillBarEntry.Kind.SKILL
                        && clearCombatSkill(hIdx)) {
                    hydrateSkillBarFromStore();
                    itemInspectClicks.cancel();
                    return true;
                }
            }
        }

        if (button == 0) {


            // Equip
            // 决议 #12：仅栈顶/held（representative）可被拖下/卸下；下层被压住不可动。
            {
                var eq = UiWindowRuntime.equipmentAt(mouseX, mouseY);
                InventoryItem item = eq == null ? null : eq.representative();
                if (eq != null && item != null) {
                    if (shift) quickUnequipToGrid(eq.slotType(), item);
                    else {
                        popSlotTop(eq); // 乐观弹出栈顶/held（server 快照为权威）
                        dragState.pickupFromEquip(item, eq.slotType());
                    }
                    return true;
                }
            }

            // Body inspect applied items (physical or meridian layer)
            int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
            if (hIdx >= 0 && SkillBarStore.snapshot().slot(hIdx) != null
                    && SkillBarStore.snapshot().slot(hIdx).kind() == SkillBarEntry.Kind.SKILL) {
                UiWindowRuntime.openPracticeDetail("technique:" + SkillBarStore.snapshot().slot(hIdx).id());
                return true;
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
                if (shift) clearQuickUseSlot(qIdx);
                else beginQuickUseDrag(qIdx);
                return true;
            }

            // Loot grid (supply coffin)
            if (!loadoutSlot && lootPanel != null && !lootPanel.isClosed()) {
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

        }

        return loadoutSlot || super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double deltaX, double deltaY) {
        if (UiWindowRuntime.mouseDrag(mouseX, mouseY, button, deltaX, deltaY)) return true;
        if (button == 0) {
            var press = itemInspectClicks.drag(mouseX, mouseY);
            if (press != null) {
                InventoryItem item = itemAtScreen(press.x(), press.y());
                if (item != null && item.instanceId() == press.instanceId()) {
                    startingItemDrag = true;
                    try {
                        mouseClicked(press.x(), press.y(), button);
                    } finally {
                        startingItemDrag = false;
                    }
                }
            }
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
        if (button == 0 && itemInspectClicks.release(mouseX, mouseY, System.currentTimeMillis())) return true;
        if (!dragState.isDragging() && UiWindowRuntime.mouseUp(mouseX, mouseY, button)) {
            itemInspectClicks.cancel();
            return true;
        }
        if (button == 0 && dragState.isDragging()) {
            attemptDrop(mouseX, mouseY);
            return true;
        }
        return super.mouseReleased(mouseX, mouseY, button);
    }

    private boolean clearCombatSkill(int slot) {
        return new com.bong.client.combat.inspect.TechniqueClientIntentSink()
            .dispatch(new com.bong.client.combat.inspect.TechniqueIntent.Clear(slot)).kind()
            == com.bong.client.ui.intent.UiIntentResult.Kind.LOCAL_ACCEPTED;
    }

    private InventoryItem itemAtScreen(double mouseX, double mouseY) {
        BackpackGridPanel grid = UiWindowRuntime.containerGridAt(mouseX, mouseY);
        if (grid != null && grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                InventoryItem item = grid.itemAt(pos.row(), pos.col());
                if (item != null) return item;
            }
        }
        {
            var eq = UiWindowRuntime.equipmentAt(mouseX, mouseY);
            if (eq != null && eq.representative() != null) return eq.representative();
        }
        int hIdx = hotbarSlotAtScreen(mouseX, mouseY);
        if (hIdx >= 0 && hotbarItems[hIdx] != null) return hotbarItems[hIdx];
        int qIdx = quickUseSlotAtScreen(mouseX, mouseY);
        if (qIdx >= 0 && quickUseItems[qIdx] != null) return quickUseItems[qIdx];
        if (UiWindowRuntime.hit(mouseX, mouseY)) return null;
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
        if (keyCode == GLFW.GLFW_KEY_ESCAPE && UiWindowRuntime.manager().capturedKey() != null) {
            UiWindowRuntime.cancelInput();
            return true;
        }
        if (UiWindowRuntime.keyPressed(keyCode, scanCode, modifiers)) return true;
        if (keyCode != GLFW.GLFW_KEY_ESCAPE && uiAdapter != null
            && uiAdapter.rootComponent.focusHandler().focused()
                instanceof io.wispforest.owo.ui.inject.GreedyInputComponent) {
            return super.keyPressed(keyCode, scanCode, modifiers);
        }
        if (keyCode == GLFW.GLFW_KEY_ESCAPE || client.options.inventoryKey.matchesKey(keyCode, scanCode)) {
            close();
            return true;
        }
        if (UiWindowRuntime.hasKeyboardFocus()) return true;
        // plan-rotate-v1 — 拖拽中按 R 旋转拖拽物（2x1 ↔ 1x2）。旋转后 draggedItem
        // 换成宽高互换的副本，高亮 / canPlace / 拖拽 ghost 每帧重读宽高即自动生效；
        // 落位时 dispatchMoveIntent 透传 dragState.draggedRotated() 让 server 权威互换。
        if (keyCode == GLFW.GLFW_KEY_R && dragState.isDragging()) {
            if (dragState.rotateDraggedItem()) {
                updateHighlights(mouseX(), mouseY());
            }
            return true;
        }
        if (keyCode == GLFW.GLFW_KEY_Q && !dragState.isDragging()) {
            var eq = UiWindowRuntime.equipmentAt(mouseX(), mouseY());
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
                itemInspectClicks.cancel();
                return true;
            }
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    /** 当前鼠标焦点所在 backpack 网格槽位的物品；无则返回 null。 */
    private InventoryItem focusedGridItem() {
        BackpackGridPanel grid = UiWindowRuntime.containerGridAt(mouseX(), mouseY());
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

    /** 容器拾取先记录精确来源，再移除本地格子。 */
    private boolean beginGridDrag(BackpackGridPanel grid, InventoryItem item) {
        if (grid == null || item == null || dragState.phase() != DragState.Phase.IDLE) {
            return false;
        }
        var anchor = grid.anchorOf(item);
        if (anchor == null) {
            return false;
        }
        dragState.pickup(item, grid.containerId(), anchor.row(), anchor.col());
        grid.remove(item);
        return true;
    }

    /** 快捷槽拖拽只拾取使用链接，物品仍留在原容器中。 */
    private boolean beginQuickUseDrag(int index) {
        if (!QuickSlotConfig.isAvailable(index)) {
            return false;
        }
        InventoryItem item = quickUseItems[index];
        if (item == null) {
            return false;
        }
        // 快捷槽只保存使用链接，不是物品容器。拾取链接不得先解绑或从背包移除物品。
        dragState.pickupFromQuickUse(item, index);
        return true;
    }

    private void attemptDrop(double mouseX, double mouseY) {
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) { dragState.cancel(); clearAllHighlights(); return; }

        // QUICK_USE 是引用链接，不参与库存移动。拖到另一个快捷槽时复制链接；
        // 其它位置直接结束拖拽，原快捷链接和背包物品都保持不变。
        if (dragState.sourceKind() == DragState.SourceKind.QUICK_USE) {
            int target = quickUseSlotAtScreen(mouseX, mouseY);
            if (target >= 0 && InventoryEquipRules.canPlaceIntoQuickUse(dragged)) {
                requestQuickUseSlot(target, dragged, dragState::drop, () ->
                    com.bong.client.BongClient.LOGGER.warn(
                        "[bong][inspect] authoritative quick-use copy rejected slot={}", target));
            } else {
                dragState.drop();
            }
            clearAllHighlights();
            return;
        }

        // Capture source before drop() resets dragState; needed for the C2S move intent.
        com.bong.client.network.ClientRequestProtocol.InvLocation fromLoc = snapshotSourceLocation();
        // plan-rotate-v1 — 同样要在 drop() 复位前捕获旋转奇偶标志；仅网格落位出口透传，
        // 非网格目标（装备槽 / hotbar / 快捷栏 / 丢弃 / loot 外部容器）恒发 false。
        boolean dropRotated = dragState.draggedRotated();

        if (UiWindowRuntime.dropWorkstationMaterial(mouseX, mouseY, dragState.originalDraggedItem())) {
            // 拖起只修改了本地格子。先恢复投影，再由服务端快照原子地移到材料区。
            returnDragToSource();
            clearAllHighlights();
            return;
        }

        if (UiWindowRuntime.hit(mouseX, mouseY) && !UiWindowRuntime.loadoutSlotAt(mouseX, mouseY)) {
            var grid = UiWindowRuntime.containerGridAt(mouseX, mouseY);
            var pos = grid == null ? null : grid.screenToGrid(mouseX, mouseY);
            boolean fromLoot = lootPanel != null && !lootPanel.isClosed()
                && lootPanel.extContainerId().equals(dragState.sourceContainerId());
            var item = fromLoot && dropRotated ? dragState.originalDraggedItem() : dragged;
            if (pos != null && grid.canPlace(item, pos.row(), pos.col())
                && isWornPackContainerDroppable(InventoryStateStore.snapshot(), grid.containerId())) {
                boolean accepted;
                if (fromLoot) {
                    lootPanel.sendMove(item.instanceId(), dragState.sourceContainerId(),
                        dragState.sourceRow(), dragState.sourceCol(), grid.containerId(), pos.row(), pos.col());
                    accepted = true;
                } else {
                    accepted = dispatchMoveIntent(item, fromLoc,
                        new com.bong.client.network.ClientRequestProtocol.ContainerLoc(grid.containerId(), pos.row(), pos.col()),
                        dropRotated);
                }
                if (accepted) {
                    grid.place(item, pos.row(), pos.col());
                    dragState.drop();
                } else returnDragToSource();
            } else returnDragToSource();
            clearAllHighlights();
            return;
        }

        boolean workbenchTarget = !UiWindowRuntime.hit(mouseX, mouseY);

        // Discard
        if (workbenchTarget && isOverDiscard(mouseX, mouseY)) {
            if (dispatchDiscardIntent(dragged, fromLoc)) {
                dragState.drop();
            } else {
                returnDragToSource();
            }
            clearAllHighlights();
            return;
        }


        // Loot grid drop (supply coffin) — handle both directions
        if (workbenchTarget && lootPanel != null && !lootPanel.isClosed()) {
            boolean fromLoot = lootPanel.extContainerId().equals(dragState.sourceContainerId());

            // plan-rotate-v1 — loot 面板走 external_container_move 协议（无 rotated 字段），
            // 旋转态落位一律还原原朝向：与 server 权威状态一致，避免快照回来时形状跳变。
            InventoryItem lootDropItem = dropRotated && dragState.originalDraggedItem() != null
                ? dragState.originalDraggedItem()
                : dragged;

            // Drop onto loot grid (from player container)
            BackpackGridPanel lg = lootPanel.lootGrid();
            if (lg.containsPoint(mouseX, mouseY)) {
                var pos = lg.screenToGrid(mouseX, mouseY);
                if (pos != null && lg.canPlace(lootDropItem, pos.row(), pos.col())) {
                    lg.place(lootDropItem, pos.row(), pos.col());
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

        }

        // Equip (with hand restriction from physical body)
        // plan-layered-equip-v1 P4（决议 #3/#12）：删除旧 swap 分支——满/占=飘红退回不顶替；
        // worn 合法则 push 栈顶（乐观更新，server 快照为权威）。canEquip 已含「worn 满 / held 占 / 锁手」拒绝。
        {
            var eq = UiWindowRuntime.equipmentAt(mouseX, mouseY);
            if (eq != null) {
                if (!commitEquipDropOrReturnToSource(dragged, fromLoc, eq.slotType())) {
                    clearAllHighlights();
                    return;
                }
                clearAllHighlights();
                return;
            }
        }

        // Body inspect drop (physical or meridian layer) — only 1×1 items
        var dropBody = UiWindowRuntime.bodyAt(mouseX, mouseY);
        if (dropBody != null) bodyInspect = dropBody;
        if (dropBody != null
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
            if (!dispatchMoveIntent(
                dragged,
                fromLoc,
                new com.bong.client.network.ClientRequestProtocol.HotbarLoc(hIdx),
                false
            )) {
                returnDragToSource();
                clearAllHighlights();
                return;
            }
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
            // 绑定只引用现有库存；结束网格拖拽时立即把物品放回原位。
            returnDragToSource();
            if (!requestQuickUseSlot(
                    qIdx,
                    dragged,
                    () -> {
                        clearAllHighlights();
                    },
                    () -> com.bong.client.BongClient.LOGGER.warn(
                        "[bong][inspect] authoritative quick-use bind rejected slot={}", qIdx)
                )) {
                clearAllHighlights();
                return;
            }
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
        if (item.isCraftRecipeScroll()) {
            actions.add(new PillMenuAction("研读制作残卷", ActionKind.CRAFT_RECIPE_SCROLL_USE));
        }
        if (item.isTechniqueScroll() && hasTechniqueScrollMetadata(item) && !isKnownTechnique(item)) {
            actions.add(new PillMenuAction("研读功法", ActionKind.TECHNIQUE_SCROLL_USE));
        }
        // plan-scroll-reading-v1 P0 — 可阅读残卷（如《经脉浅述·残卷》）右键菜单 [阅读]。
        // 与 TECHNIQUE_SCROLL_USE 互斥：readable_scroll_spec 挂载的物品不带 scrollKind，
        // 不会命中上面的 isTechniqueScroll() 分支，故不需要额外互斥判断。
        if (com.bong.client.scroll.ScrollVanillaIconMap.isReadableScroll(item.itemId())) {
            actions.add(new PillMenuAction("阅读", ActionKind.READ_SCROLL));
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
                if (uiAdapter != null) openBodyWindow(BodyInspectComponent.Layer.MERIDIAN);
                else if (bodyInspect != null) bodyInspect.setActiveLayer(BodyInspectComponent.Layer.MERIDIAN);
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
            case CRAFT_RECIPE_SCROLL_USE -> {
                pendingMeridianUse = null;
                dispatchCraftRecipeScrollUse(item);
            }
            case TECHNIQUE_SCROLL_USE -> {
                pendingMeridianUse = null;
                dispatchTechniqueScrollUse(item);
            }
            case READ_SCROLL -> {
                pendingMeridianUse = null;
                dispatchScrollReadRequest(item);
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
        MeridianChannel target = bodyInspect.selectedChannel();
        if (!dispatchApplyPillMeridianToChannel(pendingMeridianUse.item(), target)) {
            return false;
        }
        pendingMeridianUse = null;
        return true;
    }

    boolean dispatchMoveIntent(
        InventoryItem item,
        com.bong.client.network.ClientRequestProtocol.InvLocation from,
        com.bong.client.network.ClientRequestProtocol.InvLocation to,
        boolean rotated
    ) {
        if (item == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent skipped: item is null");
            return false;
        }
        if (from == null || to == null) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent skipped: from={} to={} item={}",
                from, to, item.itemId());
            return false;
        }
        if (to instanceof com.bong.client.network.ClientRequestProtocol.HotbarLoc hotbar
                && !SkillBarConfig.isAvailable(hotbar.index())) {
            return false;
        }
        if (item.instanceId() == 0L) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent skipped: item {} has instanceId=0 "
                    + "(likely Mock data — server snapshot didn't load)",
                item.itemId());
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchMoveIntent instance={} from={} to={} rotated={} item={}",
            item.instanceId(), from, to, rotated, item.itemId());
        boolean accepted = com.bong.client.network.ClientRequestSender.sendInventoryMove(
            item.instanceId(), from, to, rotated);
        if (!accepted) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] dispatchMoveIntent rejected by local transport instance={} from={} to={}",
                item.instanceId(), from, to);
        }
        return accepted;
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
        // 链接没有被移动出库存，取消时也不能回填出第二份物品。
        if (dragState.sourceKind() == DragState.SourceKind.QUICK_USE) {
            // 取消引用拖拽只结束 UI 手势，不能把链接误当作库存移动请求。
            dragState.drop();
            return;
        }
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
                    if (!restoreItemToGrid(lg, item, r.sourceRow(), r.sourceCol()))
                        placeItemAnywhere(item);
                } else {
                    restoreItemToGrid(gridById(r.sourceContainerId()), item, r.sourceRow(), r.sourceCol());
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
                // 已在 cancel() 之前单独处理；仅为 exhaustiveness 保留。
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

    /** 优先原锚点、其次同一来源 grid 空位；返回 false 才允许跨容器兜底。 */
    private static boolean restoreItemToGrid(
        BackpackGridPanel grid,
        InventoryItem item,
        int sourceRow,
        int sourceCol
    ) {
        if (grid == null || item == null) {
            return false;
        }
        if (grid.toGridEntries().stream().anyMatch(entry -> entry.item().instanceId() == item.instanceId())) return true;
        if (grid.canPlace(item, sourceRow, sourceCol)) {
            grid.place(item, sourceRow, sourceCol);
            return true;
        }
        var freePos = grid.findFreeSpace(item);
        if (freePos == null) {
            return false;
        }
        grid.place(item, freePos.row(), freePos.col());
        return true;
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

        BackpackGridPanel grid = UiWindowRuntime.containerGridAt(mouseX, mouseY);
        if (grid != null && grid.containsPoint(mouseX, mouseY)) {
            var pos = grid.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                boolean valid = grid.canPlace(dragged, pos.row(), pos.col());
                grid.highlightArea(pos.row(), pos.col(), dragged.gridWidth(), dragged.gridHeight(),
                    valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
            }
        }
        var hoverBody = UiWindowRuntime.bodyAt(mouseX, mouseY);
        if (hoverBody != null) bodyInspect = hoverBody;
        if (UiWindowRuntime.hit(mouseX, mouseY) && !UiWindowRuntime.loadoutSlotAt(mouseX, mouseY) && hoverBody == null) return;

        {
            var eq = UiWindowRuntime.equipmentAt(mouseX, mouseY);
            if (eq != null) {
                boolean valid = isEquipSlotDropValid(dragged, eq.slotType());
                eq.setHighlightState(valid
                    ? GridSlotComponent.HighlightState.VALID
                    : GridSlotComponent.HighlightState.INVALID);
            }
        }

        // Body inspect highlight
        if (hoverBody != null) {
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
        }
        for (GridSlotComponent slot : quickUseSlots) {
            if (slot != null) slot.setHighlightState(GridSlotComponent.HighlightState.NONE);
        }
        if (bodyInspect != null) bodyInspect.clearHighlight();
        if (lootPanel != null && lootPanel.lootGrid() != null) lootPanel.lootGrid().clearHighlights();
        discardStrip.surface(Surface.flat(0xFF201010));
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

    boolean dispatchCraftRecipeScrollUse(InventoryItem item) {
        if (item == null || item.instanceId() == 0L || !item.isCraftRecipeScroll()) {
            return false;
        }
        skillScrollDropFeedback = "已送出制作残卷请求";
        com.bong.client.network.ClientRequestSender.sendLearnSkillScroll(item.instanceId());
        return true;
    }

    private boolean tryReadTechniqueScroll(InventoryItem item) {
        if (!hasTechniqueScrollMetadata(item)) {
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
        if (item == null || item.instanceId() == 0L || !item.isTechniqueScroll() || !hasTechniqueScrollMetadata(item)) {
            return false;
        }
        com.bong.client.cultivation.TechniqueScrollReadScreen.showReadRequested(
            item.displayName(),
            System.currentTimeMillis()
        );
        com.bong.client.network.ClientRequestSender.sendTechniqueScrollUse(item.instanceId());
        return true;
    }

    /**
     * plan-scroll-reading-v1 P0 — 右键可阅读残卷菜单 [阅读] 触发，向 server 发送
     * {@code ScrollReadRequest}。server 校验模板挂了 {@code readable_scroll_spec} 后 emit
     * {@code ScrollOpen}（{@link com.bong.client.network.ScrollOpenHandler} 接住并弹出
     * {@link com.bong.client.scroll.ScrollReadScreen}），本方法不直接开屏——阅读不消耗物品，
     * 也无需像 {@link #dispatchTechniqueScrollUse} 那样先本地占位（round-trip 由 server 权威驱动）。
     */
    boolean dispatchScrollReadRequest(InventoryItem item) {
        if (item == null || item.instanceId() == 0L
                || !com.bong.client.scroll.ScrollVanillaIconMap.isReadableScroll(item.itemId())) {
            return false;
        }
        com.bong.client.BongClient.LOGGER.info(
            "[bong][inspect] dispatchScrollReadRequest instance={} item={}",
            item.instanceId(), item.itemId());
        com.bong.client.network.ClientRequestSender.sendScrollReadRequest(item.instanceId());
        return true;
    }

    private boolean isKnownSkillScroll(InventoryItem item) {
        return com.bong.client.skill.SkillId.fromWire(item.scrollSkillId()) != null;
    }

    private boolean hasTechniqueScrollMetadata(InventoryItem item) {
        return item.isTechniqueScroll() && !item.scrollSkillId().isBlank();
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
        PhysicalBody pb = PhysicalBodyStore.snapshot();
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
     * 拖拽命中装备槽后的最薄编排边界：校验目标，确认权威 move intent 可发送后再乐观落位。
     * 生产 {@link #attemptDrop(double, double)} 与 headless 交互回归共用此路径，避免测试只锁
     * {@link InventoryEquipRules} 而漏掉 {@code EquipLoc} 构造或 C2S dispatch。
     */
    boolean commitEquipDrop(
        InventoryItem dragged,
        com.bong.client.network.ClientRequestProtocol.InvLocation fromLoc,
        EquipSlotType targetSlot
    ) {
        if (equipPanel == null || targetSlot == null) return false;
        var eq = equipPanel.slotFor(targetSlot);
        if (eq == null || eq.isInteractionBlocked() || !isEquipSlotDropValid(dragged, targetSlot)) {
            return false;
        }
        // plan-race-system-v1 P3c — 被拖入物品的 wearer_race 与当前形态不符 → 拒绝落位
        // （client 预览拦截）。server validate_equip_to 权威兜底：即便绕过此拦截，也会
        // 回 race_mismatch 拒绝 + toast（InventoryMoveRejectedHandler）。
        if (dragged != null && dragged.itemId() != null
            && com.bong.client.inventory.state.RaceGateEval.isItemBlocked(dragged.itemId())) {
            return false;
        }

        var toLoc = new com.bong.client.network.ClientRequestProtocol.EquipLoc(
            targetSlot.name().toLowerCase(java.util.Locale.ROOT), targetSlot.wireState());
        if (!dispatchMoveIntent(dragged, fromLoc, toLoc, false)) {
            return false;
        }

        if (targetSlot.isHand()) {
            eq.setContents(com.bong.client.inventory.model.SlotContents.ofHeld(dragged));
        } else {
            java.util.List<InventoryItem> stack =
                new java.util.ArrayList<>(eq.contents().worn());
            stack.add(dragged);
            eq.setContents(new com.bong.client.inventory.model.SlotContents(stack, eq.held()));
        }
        dragState.drop();
        return true;
    }

    /** attemptDrop 与 headless 回归共用：提交失败时把拖拽物完整恢复到原来源。 */
    boolean commitEquipDropOrReturnToSource(
        InventoryItem dragged,
        com.bong.client.network.ClientRequestProtocol.InvLocation fromLoc,
        EquipSlotType targetSlot
    ) {
        if (commitEquipDrop(dragged, fromLoc, targetSlot)) {
            return true;
        }
        returnDragToSource();
        return false;
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

        if (!dispatchMoveIntent(
            item,
            new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                grid.containerId(),
                anchor.row(),
                anchor.col()
            ),
            new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                targetSlot.name().toLowerCase(java.util.Locale.ROOT), targetSlot.wireState()
            ),
            // plan-rotate-v1 — shift 快捷穿戴不经拖拽，无旋转语义。
            false
        )) {
            return;
        }
        grid.remove(item);
        // 手槽 quick-equip → held 单件。
        equipPanel.slotFor(targetSlot).setContents(
            com.bong.client.inventory.model.SlotContents.ofHeld(item));
    }

    private void quickUnequipToGrid(EquipSlotType slotType, InventoryItem item) {
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
        var pos = grid.findFreeSpace(item);
        if (pos != null) {
            if (!dispatchMoveIntent(
                item,
                new com.bong.client.network.ClientRequestProtocol.EquipLoc(
                    slotType.name().toLowerCase(java.util.Locale.ROOT), slotType.wireState()
                ),
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    grid.containerId(),
                    pos.row(),
                    pos.col()
                ),
                false
            )) {
                return;
            }
            // 决议 #12：卸下仅弹出栈顶/held（被压住下层不动）。
            popSlotTop(equipPanel.slotFor(slotType));
            grid.place(item, pos.row(), pos.col());
        }
    }

    private void quickMoveHotbarToGrid(int index) {
        InventoryItem item = hotbarItems[index];
        if (item == null) return;
        BackpackGridPanel grid = activeGrid();
        if (grid == null) return;
        var pos = grid.findFreeSpace(item);
        if (pos != null) {
            if (!dispatchMoveIntent(
                item,
                new com.bong.client.network.ClientRequestProtocol.HotbarLoc(index),
                new com.bong.client.network.ClientRequestProtocol.ContainerLoc(
                    grid.containerId(),
                    pos.row(),
                    pos.col()
                ),
                false
            )) {
                return;
            }
            hotbarItems[index] = null;
            hotbarSlots[index].clearItem();
            grid.place(item, pos.row(), pos.col());
        }
    }

    private void clearQuickUseSlot(int index) {
        if (quickUseItems[index] == null) return;
        requestQuickUseSlot(
            index,
            null,
            () -> {},
            () -> com.bong.client.BongClient.LOGGER.warn(
                "[bong][inspect] authoritative quick-use clear rejected slot={}", index)
        );
    }

    // ==================== Render ====================

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        int windowMouseX = mouseX;
        int windowMouseY = mouseY;
        if (UiWindowRuntime.hit(mouseX, mouseY)) {
            mouseX = -1;
            mouseY = -1;
        }
        super.render(context, mouseX, mouseY, delta);

        // Buff bar tooltip — 同理逃出 owo 组件裁剪区；buff 条所有 tab 常驻，此处不按 activeTab 过滤。
        if (buffBarPanel != null) {
            var matrices = context.getMatrices();
            matrices.push();
            matrices.translate(0, 0, 400);
            buffBarPanel.drawTooltip(context, mouseX, mouseY);
            matrices.pop();
        }

        UiWindowRuntime.renderWorkspace(context, windowMouseX, windowMouseY, delta);
        mouseX = windowMouseX;
        mouseY = windowMouseY;
        context.getMatrices().push();
        try {
            context.getMatrices().translate(0, 0, UiWindowRuntime.overlayDepth());
            drawPendingMeridianPrompt(context);
            drawPillMenuOverlay(context, mouseX, mouseY);
            drawWeaponMenuOverlay(context, mouseX, mouseY);
            drawSkillBarMenuOverlay(context, mouseX, mouseY);

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


            // 左下角功法名：绑定功法图标都一样（缺专属贴图，全 fallback 同一张残卷），
            // 拖拽中 / hover 已绑槽时在左下角标出名字以便分辨。
            String cornerName = cornerTechniqueName(mouseX, mouseY);
            if (cornerName != null) {
                drawCornerTechniqueName(context, cornerName);
            }
            context.draw();
        } finally { context.getMatrices().pop(); }
    }

    @Override
    public boolean mouseScrolled(double x, double y, double amount) {
        return UiWindowRuntime.scroll(x, y, amount) || super.mouseScrolled(x, y, amount);
    }

    @Override
    public boolean charTyped(char chr, int modifiers) {
        return UiWindowRuntime.charTyped(chr, modifiers) || super.charTyped(chr, modifiers);
    }

    public boolean windowHostReadyForPreview() { return uiAdapter != null && !invalid; }
    public boolean windowHostFailedForPreview() { return invalid; }

    public GridSlotComponent itemSlotForPreview(long instanceId) {
        for (BackpackGridPanel grid : containerGrids) {
            for (int row = 0; row < grid.rows(); row++) {
                for (int col = 0; col < grid.cols(); col++) {
                    InventoryItem item = grid.itemAt(row, col);
                    if (item != null && item.instanceId() == instanceId) return grid.slotAt(row, col);
                }
            }
        }
        return null;
    }

    /** 左下角要显示的功法名：优先拖拽中的功法，其次 hover 到的已绑定 1-9 槽；都没有则 null。 */
    private String cornerTechniqueName(int mouseX, int mouseY) {
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


    private void drawPendingMeridianPrompt(DrawContext context) {
        if (pendingMeridianUse == null || bodyInspect == null) return;
        MeridianChannel focus = bodyInspect.focusedChannel();
        String text = focus == null
            ? "外敷：请先选择经脉（左键确认 / 右键取消）"
            : "外敷：点击模型中的 " + focus.displayName() + "（右键取消）";
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
        InspectScreenBootstrap.openRepairScreen(MinecraftClient.getInstance(), item);
    }

    // ==================== Loot panel mount/unmount ====================

    private void mountLootPanelIfActive() {
        LootContainerStateStore.Session session = LootContainerStateStore.current();
        if (!(session instanceof LootContainerStateStore.OpenSession open)) return;
        if (lootPanel != null && !lootPanel.isClosed()
                && lootPanel.sessionId() == open.sessionId()) return; // same session already mounted
        // 新 session 到达时先卸载旧 panel，避免 UI 继续操作已失效容器。
        unmountLootPanel();
        lootPanel = new LootContainerPanel(LootContainerSessionAdapter.open(open));
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

    /** 右键容器物品打开内含物；与服务端 container_id_for_worn_pack 使用同一身份。 */
    private void openContainerItem(InventoryItem packItem) {
        if (packItem != null) UiWindowRuntime.openContainer("pack_" + packItem.instanceId());
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
