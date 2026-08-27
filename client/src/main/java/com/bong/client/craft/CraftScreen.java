package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.contract.UiScreenScope;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;
import org.lwjgl.glfw.GLFW;

import java.util.List;
import java.util.function.Consumer;

/** plan-craft-ux-v1 — 640×340 三栏手搓屏幕。 */
public final class CraftScreen extends OwoXmlScreenHost<FlowLayout> {
    private static final Text TITLE = Text.literal("手搓台");
    private static final int WIDE_MIN_WIDTH = 660;
    private static final int WIDE_MIN_HEIGHT = 360;

    private CraftRecipeListWidget recipeList;
    private CraftMaterialGrid materialGrid;
    private CraftOutputPreview outputPreview;
    private CraftActionBar actionBar;
    private LabelComponent subtitle;

    private String selectedId;
    private int flashTicks;
    private boolean listenersAttached;
    private final CraftOutcomeFeedback.CompleteSoundPlayer completeSound;
    private final Runnable outcomeRefresh;

    /** 测试观察点：当前完成闪光剩余 tick。 */
    public int flashTicksForTests() {
        return flashTicks;
    }
    private long lastTickSoundElapsed = -1;

    // 5 个 listener 按"变了什么"分组件路由刷新，而不是统一 scheduleRefresh()→refreshAll()：
    // inventory 快照 server 推得很勤，若每次都 refreshAll()（内部含 recipeList.refresh()），
    // 会把左栏配方列表也牵连进刷新节奏——配合 CraftRecipeListWidget 的 diff 式 refresh() 本身
    // 不会重建行，但收窄刷新范围仍是工程卫生：session/outcome 事件与"配方集合是否变化"无关，
    // 没理由碰 recipeList / subtitle。
    private final Consumer<List<CraftRecipe>> recipeListener = recipes -> scheduleRefresh(this::refreshAll);
    private final Consumer<CraftSessionStateView> sessionListener = state -> scheduleRefresh(this::refreshSessionOnly);
    private final Consumer<CraftStore.CraftOutcomeEvent> outcomeListener;
    private final Consumer<CraftStore.RecipeUnlockedEvent> unlockListener = event -> scheduleRefresh(this::refreshAll);
    private final Consumer<InventoryModel> inventoryListener = inventory -> scheduleRefresh(this::refreshInventoryOnly);
    private final Consumer<SkillSetSnapshot> skillListener = skills -> scheduleRefresh(this::refreshSkillOnly);

    public CraftScreen() {
        this(CraftOutcomeFeedback::playDefaultCompleteSound, null);
    }

    /** 测试注入点：观察 screen 自身的完成音与 outcome refresh，不替换 Store/feedback 链。 */
    CraftScreen(CraftOutcomeFeedback.CompleteSoundPlayer completeSound, Runnable outcomeRefresh) {
        super(TITLE, FlowLayout.class, "craft");
        this.completeSound = completeSound;
        this.outcomeRefresh = outcomeRefresh != null
            ? outcomeRefresh
            : () -> scheduleRefresh(this::refreshOutcomeOnly);
        this.outcomeListener = event -> CraftOutcomeFeedback.apply(
            event,
            ticks -> flashTicks = ticks,
            this.completeSound,
            this.outcomeRefresh
        );
    }

    /**
     * XML 已声明外壳、标题、三栏和底部 bridge 插槽；这里仅把已有动态
     * 列表/材料/产物组件挂入插槽，并把状态监听接到它们。
     */
    @Override
    protected void bindTemplate(FlowLayout root) {
        FlowLayout recipeHost = component(FlowLayout.class, "recipe-host");
        FlowLayout materialHost = component(FlowLayout.class, "material-host");
        FlowLayout outputHost = component(FlowLayout.class, "output-host");
        FlowLayout actionHost = component(FlowLayout.class, "action-host");
        label("craft-title").text(Text.literal("手搓台").formatted(Formatting.BOLD));
        subtitle = label("craft-subtitle");

        recipeList = new CraftRecipeListWidget(id -> {
            selectedId = id;
            refreshRightPanel();
        }, CraftRecipe::isHandcraft);
        materialGrid = new CraftMaterialGrid();
        outputPreview = new CraftOutputPreview();
        recipeHost.child(recipeList.root());
        materialHost.child(materialGrid.root());
        outputHost.child(outputPreview.root());

        actionBar = new CraftActionBar(() -> actionBar.setQuantityToMax(), this::startCraft, this::refreshAll);
        actionHost.child(actionBar.root());
        refreshAll();
    }

    @Override
    protected String selectTemplateId(int logicalWidth, int logicalHeight) {
        return templateIdForViewport(logicalWidth, logicalHeight);
    }

    static String templateIdForViewport(int logicalWidth, int logicalHeight) {
        return logicalWidth >= WIDE_MIN_WIDTH && logicalHeight >= WIDE_MIN_HEIGHT
            ? "craft"
            : "craft-compact";
    }

    @Override
    protected void onHostOpened(UiScreenScope scope) {
        attachListeners();
        scope.addCleanup(this::detachListeners);
    }

    @Override
    protected void onHostClosed() {
        // 测试缝可能在 host open 前手动 attach；这里幂等兜底，生产清理由 scope 先执行。
        detachListeners();
    }

    @Override
    public void removed() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null && CraftStore.sessionState().active()) {
            ClientRequestSender.sendCraftCancel();
        }
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


    /** 测试缝：复用生产 attach，重复调用模拟 build/resize。 */
    public void attachOutcomeListenerForTests() {
        attachListeners();
    }

    /** 测试缝：复用生产 detach。 */
    public void detachOutcomeListenerForTests() {
        detachListeners();
    }

    public static boolean tabHeightMatchesAlchemy() {
        return CraftScreenLayout.matchesAlchemyTabHeight();
    }

    private void attachListeners() {
        if (listenersAttached) {
            return;
        }
        listenersAttached = true;
        CraftStore.addRecipeListener(recipeListener);
        CraftStore.addSessionListener(sessionListener);
        CraftStore.addOutcomeListener(outcomeListener);
        CraftStore.addUnlockListener(unlockListener);
        InventoryStateStore.addListener(inventoryListener);
        SkillSetStore.addListener(skillListener);
    }

    private void detachListeners() {
        if (!listenersAttached) {
            return;
        }
        listenersAttached = false;
        CraftStore.removeRecipeListener(recipeListener);
        CraftStore.removeSessionListener(sessionListener);
        CraftStore.removeOutcomeListener(outcomeListener);
        CraftStore.removeUnlockListener(unlockListener);
        InventoryStateStore.removeListener(inventoryListener);
        SkillSetStore.removeListener(skillListener);
    }

    /** 初始化 / 配方集合或解锁态变化（recipeListener、unlockListener）：结构可能变，全量刷新。 */
    private void refreshAll() {
        if (recipeList == null || materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        SkillSetSnapshot skills = SkillSetStore.snapshot();
        ensureSelection(skills);
        CraftRecipe selected = currentRecipe();
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(inventory, skills);
        refreshActionAndMaterial(selected, inventory, skills);
        outputPreview.refresh(selected, flashTicks);
        updateSubtitle(selected, inventory);
    }

    /** 左栏点击选中配方（不经 listener）：右栏 + 副标题，不碰 recipeList 自身。 */
    private void refreshRightPanel() {
        if (materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        SkillSetSnapshot skills = SkillSetStore.snapshot();
        CraftRecipe selected = currentRecipe();
        refreshActionAndMaterial(selected, inventory, skills);
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
        SkillSetSnapshot skills = SkillSetStore.snapshot();
        CraftRecipe selected = currentRecipe();
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(inventory, skills);
        refreshActionAndMaterial(selected, inventory, skills);
        updateSubtitle(selected, inventory);
    }

    private void refreshSkillOnly() {
        if (recipeList == null || materialGrid == null || actionBar == null) {
            return;
        }
        SkillSetSnapshot skills = SkillSetStore.snapshot();
        InventoryModel inventory = InventoryStateStore.snapshot();
        ensureSelection(skills);
        CraftRecipe selected = currentRecipe();
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(inventory, skills);
        refreshActionAndMaterial(selected, inventory, skills);
        updateSubtitle(selected, inventory);
    }

    /** sessionListener：制作进度相关，只有 actionBar / materialGrid 会随 session tick 变化。 */
    private void refreshSessionOnly() {
        if (materialGrid == null || actionBar == null) {
            return;
        }
        refreshActionAndMaterial(
            currentRecipe(),
            InventoryStateStore.snapshot(),
            SkillSetStore.snapshot()
        );
    }

    /** outcomeListener：制作完成/失败后 outputPreview 需要反映最新产物；inventory 快照会另行
     * 触发 inventoryListener 推数量，这里只需 actionBar/materialGrid 跟上 session 状态复位。 */
    private void refreshOutcomeOnly() {
        if (outputPreview == null || materialGrid == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        SkillSetSnapshot skills = SkillSetStore.snapshot();
        CraftRecipe selected = currentRecipe();
        outputPreview.refresh(selected, flashTicks);
        refreshActionAndMaterial(selected, inventory, skills);
    }

    /** 五条刷新路径共用的 actionBar+materialGrid 段：session 现取现用，quantity 依赖
     * actionBar 先 refresh 完再读，顺序不能倒。 */
    private void refreshActionAndMaterial(
        CraftRecipe selected,
        InventoryModel inventory,
        SkillSetSnapshot skills
    ) {
        CraftSessionStateView session = CraftStore.sessionState();
        actionBar.refresh(selected, inventory, session, skills);
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
        String skillHint = selected != null && !CraftActionBar.skillSatisfied(selected, SkillSetStore.snapshot())
            ? " · 技艺不足"
            : "";
        subtitle.text(Text.literal("C 关闭 · 已知配方 " + known + " · 当前可做 x" + craftable + skillHint));
    }

    private void ensureSelection(SkillSetSnapshot skills) {
        // 仅在手搓配方(station=null)内选择：制作台配方归 WorkbenchScreen，不在此屏出现。
        if (selectedId != null
            && CraftStore.recipe(selectedId)
                .filter(CraftRecipe::isHandcraft)
                .filter(recipe -> recipe.unlocked() && CraftActionBar.skillSatisfied(recipe, skills))
                .isPresent()) {
            return;
        }
        selectedId = CraftStore.recipes().stream()
            .filter(CraftRecipe::isHandcraft)
            .filter(CraftRecipe::unlocked)
            .filter(recipe -> CraftActionBar.skillSatisfied(recipe, skills))
            .findFirst()
            .or(() -> CraftStore.recipes().stream()
                .filter(CraftRecipe::isHandcraft)
                .filter(CraftRecipe::unlocked)
                .findFirst())
            .or(() -> CraftStore.recipes().stream().filter(CraftRecipe::isHandcraft).findFirst())
            .map(CraftRecipe::id)
            .orElse(null);
    }

    private CraftRecipe currentRecipe() {
        return selectedId == null ? null : CraftStore.recipe(selectedId).orElse(null);
    }

    private void startCraft(int quantity) {
        CraftRecipe selected = currentRecipe();
        if (selected == null
            || !selected.unlocked()
            || !CraftActionBar.skillSatisfied(selected, SkillSetStore.snapshot())) {
            return;
        }
        ClientRequestSender.sendCraftStart(selected.id(), Math.max(1, quantity));
        playTickSound();
    }

    private void scheduleRefresh(Runnable action) {
        Runnable guarded = () -> screenScope().runIfOpen(action);
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null) {
            client.execute(guarded);
        } else {
            guarded.run();
        }
    }

    private static void playTickSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.BLOCK_ANVIL_USE, 0.1F, 1.5F);
        }
    }

}
