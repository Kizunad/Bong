package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.intent.UiIntentResult;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;
import org.lwjgl.glfw.GLFW;

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
    private final CraftOutcomeFeedback.CompleteSoundPlayer completeSound;
    private final Runnable outcomeRefresh;
    private final CraftScreenController controller;
    private UiSubscription outcomeTestSubscription;

    /** 测试观察点：当前完成闪光剩余 tick。 */
    public int flashTicksForTests() {
        return flashTicks;
    }
    private long lastTickSoundElapsed = -1;

    public CraftScreen() {
        this(CraftOutcomeFeedback::playDefaultCompleteSound, null);
    }

    /** 测试注入点：观察 screen 自身的完成音与 outcome refresh，不替换 Store/feedback 链。 */
    CraftScreen(CraftOutcomeFeedback.CompleteSoundPlayer completeSound, Runnable outcomeRefresh) {
        super(TITLE, FlowLayout.class, "craft");
        this.completeSound = completeSound;
        this.outcomeRefresh = outcomeRefresh != null
            ? outcomeRefresh
            : this::refreshOutcomeOnly;
        this.controller = CraftScreenController.production(this::applyViewModel, CraftScreen::executeOnClientThread);
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

        actionBar = new CraftActionBar(
            () -> actionBar.setQuantityToMax(),
            this::startCraft,
            () -> refreshAll(controller.viewModel())
        );
        actionHost.child(actionBar.root());
        refreshAll(controller.viewModel());
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
        controller.onOpen(scope);
    }

    @Override
    protected void onHostClosed() {
        detachOutcomeListenerForTests();
        controller.onClose();
    }

    @Override
    public void removed() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null && controller.viewModel().session().active()) {
            controller.intentSink().dispatch(new CraftIntent.Cancel());
        }
        super.removed();
    }

    @Override
    public void tick() {
        super.tick();
        CraftSessionStateView state = controller.viewModel().session();
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


    /** 测试缝：经生产 state source 观察 outcome，重复调用仍只登记一次。 */
    public void attachOutcomeListenerForTests() {
        if (outcomeTestSubscription != null && !outcomeTestSubscription.isClosed()) {
            return;
        }
        outcomeTestSubscription = CraftUiStateSource.production().subscribe(update -> {
            if (update.change() == CraftScreenViewModel.Change.OUTCOME) {
                executeOnClientThread(() -> applyOutcome(update));
            }
        });
    }

    /** 测试缝：复用生产 subscription 的 exactly-once 关闭语义。 */
    public void detachOutcomeListenerForTests() {
        if (outcomeTestSubscription == null) {
            return;
        }
        outcomeTestSubscription.close();
        outcomeTestSubscription = null;
    }

    public static boolean tabHeightMatchesAlchemy() {
        return CraftScreenLayout.matchesAlchemyTabHeight();
    }

    /** 初始化或配方集合变化：结构可能变化，执行全量刷新。 */
    private void refreshAll(CraftScreenViewModel model) {
        if (recipeList == null || materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = model.inventory();
        SkillSetSnapshot skills = model.skills();
        ensureSelection(model);
        CraftRecipe selected = currentRecipe(model);
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(model.recipes(), inventory, skills);
        refreshActionAndMaterial(selected, model);
        outputPreview.refresh(selected, flashTicks);
        updateSubtitle(selected, model);
    }

    /** 左栏点击选中配方（不经 listener）：右栏 + 副标题，不碰 recipeList 自身。 */
    private void refreshRightPanel() {
        if (materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        CraftScreenViewModel model = controller.viewModel();
        CraftRecipe selected = currentRecipe(model);
        refreshActionAndMaterial(selected, model);
        outputPreview.refresh(selected, flashTicks);
        updateSubtitle(selected, model);
    }

    /**
     * INVENTORY 变化：server 快照推得很勤的高频路径。recipeList.refresh() 内部走 diff 式
     * 原地更新（id 序列不变则不 clearChildren），不会触发 owo ScrollContainer 的滚动回弹；
     * outputPreview 与配方数量无关，跳过。
     */
    private void refreshInventoryOnly(CraftScreenViewModel model) {
        if (recipeList == null || materialGrid == null || actionBar == null) {
            return;
        }
        CraftRecipe selected = currentRecipe(model);
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(model.recipes(), model.inventory(), model.skills());
        refreshActionAndMaterial(selected, model);
        updateSubtitle(selected, model);
    }

    private void refreshSkillOnly(CraftScreenViewModel model) {
        if (recipeList == null || materialGrid == null || actionBar == null) {
            return;
        }
        ensureSelection(model);
        CraftRecipe selected = currentRecipe(model);
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(model.recipes(), model.inventory(), model.skills());
        refreshActionAndMaterial(selected, model);
        updateSubtitle(selected, model);
    }

    /** SESSION 变化：只有 actionBar / materialGrid 会随制作进度变化。 */
    private void refreshSessionOnly(CraftScreenViewModel model) {
        if (materialGrid == null || actionBar == null) {
            return;
        }
        refreshActionAndMaterial(currentRecipe(model), model);
    }

    /** OUTCOME 变化：制作结果刷新产物；后续 INVENTORY 变化会独立推送材料数量。 */
    private void refreshOutcomeOnly() {
        if (outputPreview == null || materialGrid == null || actionBar == null) {
            return;
        }
        CraftScreenViewModel model = controller.viewModel();
        CraftRecipe selected = currentRecipe(model);
        outputPreview.refresh(selected, flashTicks);
        refreshActionAndMaterial(selected, model);
    }

    /** 五条刷新路径共用的 actionBar+materialGrid 段：session 现取现用，quantity 依赖
     * actionBar 先 refresh 完再读，顺序不能倒。 */
    private void refreshActionAndMaterial(
        CraftRecipe selected,
        CraftScreenViewModel model
    ) {
        actionBar.refresh(selected, model.inventory(), model.session(), model.skills());
        materialGrid.refresh(selected, model.inventory(), model.session(), actionBar.quantity());
    }

    private void refreshOutputOnly() {
        if (outputPreview != null) {
            outputPreview.refresh(currentRecipe(controller.viewModel()), flashTicks);
        }
    }

    private void updateSubtitle(CraftRecipe selected, CraftScreenViewModel model) {
        if (subtitle == null) {
            return;
        }
        int known = (int) model.recipes().stream().filter(CraftRecipe::isHandcraft).count();
        int craftable = selected == null ? 0 : CraftInventoryCounter.maxCraftable(selected, model.inventory());
        String skillHint = selected != null && !CraftActionBar.skillSatisfied(selected, model.skills())
            ? " · 技艺不足"
            : "";
        subtitle.text(Text.literal("C 关闭 · 已知配方 " + known + " · 当前可做 x" + craftable + skillHint));
    }

    private void ensureSelection(CraftScreenViewModel model) {
        // 仅在手搓配方(station=null)内选择：制作台配方归 WorkbenchScreen，不在此屏出现。
        if (selectedId != null
            && model.recipe(selectedId)
                .filter(CraftRecipe::isHandcraft)
                .filter(recipe -> recipe.unlocked() && CraftActionBar.skillSatisfied(recipe, model.skills()))
                .isPresent()) {
            return;
        }
        selectedId = model.recipes().stream()
            .filter(CraftRecipe::isHandcraft)
            .filter(CraftRecipe::unlocked)
            .filter(recipe -> CraftActionBar.skillSatisfied(recipe, model.skills()))
            .findFirst()
            .or(() -> model.recipes().stream()
                .filter(CraftRecipe::isHandcraft)
                .filter(CraftRecipe::unlocked)
                .findFirst())
            .or(() -> model.recipes().stream().filter(CraftRecipe::isHandcraft).findFirst())
            .map(CraftRecipe::id)
            .orElse(null);
    }

    private CraftRecipe currentRecipe(CraftScreenViewModel model) {
        return selectedId == null ? null : model.recipe(selectedId).orElse(null);
    }

    private void startCraft(int quantity) {
        CraftScreenViewModel model = controller.viewModel();
        CraftRecipe selected = currentRecipe(model);
        if (selected == null
            || !selected.unlocked()
            || !CraftActionBar.skillSatisfied(selected, model.skills())) {
            return;
        }
        UiIntentResult result = controller.intentSink().dispatch(
            new CraftIntent.Start(selected.id(), Math.max(1, quantity))
        );
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            playTickSound();
        }
    }

    private void applyViewModel(CraftScreenViewModel model) {
        switch (model.change()) {
            case INITIAL, RECIPES -> refreshAll(model);
            case SESSION -> refreshSessionOnly(model);
            case OUTCOME -> applyOutcome(model);
            case INVENTORY -> refreshInventoryOnly(model);
            case SKILLS -> refreshSkillOnly(model);
        }
    }

    private void applyOutcome(CraftScreenViewModel model) {
        CraftOutcomeFeedback.apply(
            model,
            ticks -> flashTicks = ticks,
            completeSound,
            outcomeRefresh
        );
    }

    private static void playTickSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.BLOCK_ANVIL_USE, 0.1F, 1.5F);
        }
    }

    /** 所有状态源更新统一回到 Minecraft 主线程，避免网络线程触碰 owo 组件。 */
    private static void executeOnClientThread(Runnable task) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            task.run();
            return;
        }
        client.execute(task);
    }

}
