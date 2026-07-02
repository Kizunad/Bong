package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.List;
import java.util.function.Consumer;

/** plan-craft-ux-v1 — 640×340 三栏手搓屏幕。 */
public final class CraftScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("手搓台");

    private CraftRecipeListWidget recipeList;
    private CraftMaterialGrid materialGrid;
    private CraftOutputPreview outputPreview;
    private CraftActionBar actionBar;
    private LabelComponent subtitle;

    private String selectedId;
    private int flashTicks;
    private long lastTickSoundElapsed = -1;

    // 5 个 listener 按"变了什么"分组件路由刷新，而不是统一 scheduleRefresh()→refreshAll()：
    // inventory 快照 server 推得很勤，若每次都 refreshAll()（内部含 recipeList.refresh()），
    // 会把左栏配方列表也牵连进刷新节奏——配合 CraftRecipeListWidget 的 diff 式 refresh() 本身
    // 不会重建行，但收窄刷新范围仍是工程卫生：session/outcome 事件与"配方集合是否变化"无关，
    // 没理由碰 recipeList / subtitle。
    private final Consumer<List<CraftRecipe>> recipeListener = recipes -> scheduleRefresh(this::refreshAll);
    private final Consumer<CraftSessionStateView> sessionListener = state -> scheduleRefresh(this::refreshSessionOnly);
    private final Consumer<CraftStore.CraftOutcomeEvent> outcomeListener = event -> {
        if (event.kind() == CraftStore.CraftOutcomeEvent.Kind.COMPLETED) {
            flashTicks = 6;
            playCompleteSound();
        }
        scheduleRefresh(this::refreshOutcomeOnly);
    };
    private final Consumer<CraftStore.RecipeUnlockedEvent> unlockListener = event -> scheduleRefresh(this::refreshAll);
    private final Consumer<InventoryModel> inventoryListener = inventory -> scheduleRefresh(this::refreshInventoryOnly);

    public CraftScreen() {
        super(TITLE);
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

        FlowLayout panel = Containers.verticalFlow(Sizing.fixed(CraftScreenLayout.PANEL_W), Sizing.fixed(CraftScreenLayout.PANEL_H));
        panel.surface(Surface.flat(0xFF0D0D15).and(Surface.outline(0xFF4A4050)));
        panel.padding(Insets.of(6));
        panel.gap(4);
        panel.child(buildHeader());

        FlowLayout columns = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(CraftScreenLayout.BODY_H));
        columns.gap(4);
        recipeList = new CraftRecipeListWidget(id -> {
            selectedId = id;
            refreshRightPanel();
        }, CraftRecipe::isHandcraft);
        materialGrid = new CraftMaterialGrid();
        outputPreview = new CraftOutputPreview();
        columns.child(recipeList.root());
        columns.child(materialGrid.root());
        columns.child(outputPreview.root());
        panel.child(columns);

        actionBar = new CraftActionBar(() -> actionBar.setQuantityToMax(), this::startCraft, this::refreshAll);
        panel.child(actionBar.root());

        root.child(panel);
        attachListeners();
        refreshAll();
    }

    @Override
    public void removed() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null && CraftStore.sessionState().active()) {
            ClientRequestSender.sendCraftCancel();
        }
        CraftStore.removeRecipeListener(recipeListener);
        CraftStore.removeSessionListener(sessionListener);
        CraftStore.removeOutcomeListener(outcomeListener);
        CraftStore.removeUnlockListener(unlockListener);
        InventoryStateStore.removeListener(inventoryListener);
        super.removed();
    }

    @Override
    public void tick() {
        super.tick();
        CraftSessionStateView state = CraftStore.sessionState();
        if (state.active()) {
            long elapsed = state.elapsedTicks();
            if (elapsed > 0 && elapsed % 20 == 0 && elapsed != lastTickSoundElapsed) {
                lastTickSoundElapsed = elapsed;
                playTickSound();
            }
        } else {
            lastTickSoundElapsed = -1;
        }
        if (flashTicks > 0) {
            flashTicks--;
            refreshOutputOnly();
        }
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (keyCode == GLFW.GLFW_KEY_C || keyCode == GLFW.GLFW_KEY_ESCAPE) {
            close();
            return true;
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    public static boolean tabHeightMatchesAlchemy() {
        return CraftScreenLayout.matchesAlchemyTabHeight();
    }

    private FlowLayout buildHeader() {
        FlowLayout header = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(CraftScreenLayout.HEADER_H));
        header.verticalAlignment(VerticalAlignment.CENTER);
        header.gap(8);
        header.child(label("§f§l手搓台", 0xFFFFFFFF));
        subtitle = label("C 关闭 · 双击快速制作", 0xFFA8A8B8);
        header.child(subtitle);
        // fill spacer 必须排在最后：owo fill(100) 占整宽，放在中间会把副标题顶出右边界。
        header.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
        return header;
    }

    private void attachListeners() {
        CraftStore.addRecipeListener(recipeListener);
        CraftStore.addSessionListener(sessionListener);
        CraftStore.addOutcomeListener(outcomeListener);
        CraftStore.addUnlockListener(unlockListener);
        InventoryStateStore.addListener(inventoryListener);
    }

    /** 初始化 / 配方集合或解锁态变化（recipeListener、unlockListener）：结构可能变，全量刷新。 */
    private void refreshAll() {
        if (recipeList == null || materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        ensureSelection();
        CraftRecipe selected = currentRecipe();
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(inventory);
        refreshActionAndMaterial(selected, inventory);
        outputPreview.refresh(selected, flashTicks);
        updateSubtitle(selected, inventory);
    }

    /** 左栏点击选中配方（不经 listener）：右栏 + 副标题，不碰 recipeList 自身。 */
    private void refreshRightPanel() {
        if (materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        CraftRecipe selected = currentRecipe();
        refreshActionAndMaterial(selected, inventory);
        outputPreview.refresh(selected, flashTicks);
        updateSubtitle(selected, inventory);
    }

    /**
     * inventoryListener：server 快照推得很勤的高频路径。recipeList.refresh() 内部走 diff 式
     * 原地更新（id 序列不变则不 clearChildren），不会触发 owo ScrollContainer 的滚动回弹；
     * outputPreview 与配方数量无关，跳过。
     */
    private void refreshInventoryOnly() {
        if (recipeList == null || materialGrid == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        CraftRecipe selected = currentRecipe();
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(inventory);
        refreshActionAndMaterial(selected, inventory);
        updateSubtitle(selected, inventory);
    }

    /** sessionListener：制作进度相关，只有 actionBar / materialGrid 会随 session tick 变化。 */
    private void refreshSessionOnly() {
        if (materialGrid == null || actionBar == null) {
            return;
        }
        refreshActionAndMaterial(currentRecipe(), InventoryStateStore.snapshot());
    }

    /** outcomeListener：制作完成/失败后 outputPreview 需要反映最新产物；inventory 快照会另行
     * 触发 inventoryListener 推数量，这里只需 actionBar/materialGrid 跟上 session 状态复位。 */
    private void refreshOutcomeOnly() {
        if (outputPreview == null || materialGrid == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        CraftRecipe selected = currentRecipe();
        outputPreview.refresh(selected, flashTicks);
        refreshActionAndMaterial(selected, inventory);
    }

    /** 五条刷新路径共用的 actionBar+materialGrid 段：session 现取现用，quantity 依赖
     * actionBar 先 refresh 完再读，顺序不能倒。 */
    private void refreshActionAndMaterial(CraftRecipe selected, InventoryModel inventory) {
        CraftSessionStateView session = CraftStore.sessionState();
        actionBar.refresh(selected, inventory, session);
        materialGrid.refresh(selected, inventory, session, actionBar.quantity());
    }

    private void refreshOutputOnly() {
        if (outputPreview != null) {
            outputPreview.refresh(currentRecipe(), flashTicks);
        }
    }

    private void updateSubtitle(CraftRecipe selected, InventoryModel inventory) {
        if (subtitle == null) {
            return;
        }
        int known = (int) CraftStore.recipes().stream().filter(CraftRecipe::isHandcraft).count();
        int craftable = selected == null ? 0 : CraftInventoryCounter.maxCraftable(selected, inventory);
        subtitle.text(Text.literal("C 关闭 · 已知配方 " + known + " · 当前可做 x" + craftable));
    }

    private void ensureSelection() {
        // 仅在手搓配方(station=null)内选择：制作台配方归 WorkbenchScreen，不在此屏出现。
        if (selectedId != null
            && CraftStore.recipe(selectedId).filter(CraftRecipe::isHandcraft).isPresent()) {
            return;
        }
        selectedId = CraftStore.recipes().stream()
            .filter(CraftRecipe::isHandcraft)
            .filter(CraftRecipe::unlocked)
            .findFirst()
            .or(() -> CraftStore.recipes().stream().filter(CraftRecipe::isHandcraft).findFirst())
            .map(CraftRecipe::id)
            .orElse(null);
    }

    private CraftRecipe currentRecipe() {
        return selectedId == null ? null : CraftStore.recipe(selectedId).orElse(null);
    }

    private void startCraft(int quantity) {
        CraftRecipe selected = currentRecipe();
        if (selected == null) {
            return;
        }
        ClientRequestSender.sendCraftStart(selected.id(), Math.max(1, quantity));
        playTickSound();
    }

    private void scheduleRefresh(Runnable action) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null) {
            client.execute(action);
        } else {
            action.run();
        }
    }

    private static void playTickSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.BLOCK_ANVIL_USE, 0.1F, 1.5F);
        }
    }

    private static void playCompleteSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.ENTITY_PLAYER_LEVELUP, 0.2F, 1.5F);
        }
    }

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        return label;
    }
}
