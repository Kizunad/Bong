package com.bong.client.craft;

import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;

import java.util.Set;
import java.util.concurrent.Executor;
import java.util.function.Consumer;
import java.util.function.Predicate;
import java.util.function.LongSupplier;

/** 单一制作会话的窗口所有者。隐藏视图不取消业务，只有明确关闭才发取消请求。 */
public final class CraftWindows {
    private static final long REQUEST_WAIT_MS = 5_000;
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "craft", "craft-window", 300, 200, Set.of(UiWindowDefinition.Capability.WINDOW));

    private final UiWindowManager manager;
    private final UiStateSource<CraftScreenViewModel> source;
    private final UiIntentSink<CraftIntent> sink;
    private final Executor executor;
    private final Predicate<CraftContext> available;
    private final LongSupplier clock;
    private UiWindowManager.WindowState window;
    private CraftScreenController controller;
    private CraftContext context = CraftContext.HANDCRAFT;
    private Consumer<CraftScreenViewModel> listener = ignored -> {};
    private boolean pending;
    private boolean materialPending;
    private long pendingUntil;
    private CraftScreenViewModel closingSnapshot;
    private long closingGeneration;
    private long closingUntil;

    public CraftWindows(UiWindowManager manager, UiStateSource<CraftScreenViewModel> source,
                        UiIntentSink<CraftIntent> sink, Executor executor, Predicate<CraftContext> available) {
        this(manager, source, sink, executor, available, System::currentTimeMillis);
    }

    CraftWindows(UiWindowManager manager, UiStateSource<CraftScreenViewModel> source,
                 UiIntentSink<CraftIntent> sink, Executor executor, Predicate<CraftContext> available,
                 LongSupplier clock) {
        this.manager = manager;
        this.source = source;
        this.sink = sink;
        this.executor = executor;
        this.available = available;
        this.clock = clock;
    }

    public UiWindowManager.WindowState open(CraftContext requested, UiWindowManager.Rect bounds) {
        if (window != null) {
            // 请求尚未返回或制作仍在继续时，不把同一会话重新标成另一个工位。
            if (!busy() && model().inventory().craftMaterials().isEmpty() && !context.equals(requested)) {
                context = requested;
                listener.accept(model());
            }
            return manager.openOrFocus(DEFINITION, window.key(), window.bounds());
        }
        if (!awaitingClose() && !source.snapshot().session().active()
            && source.snapshot().inventory().craftMaterials().isEmpty()) context = requested;
        window = manager.openOrFocus(DEFINITION, manager.key(DEFINITION.windowType(), "player"), bounds);
        controller = new CraftScreenController(source, sink, model -> {
            if (model.change() == CraftScreenViewModel.Change.SESSION
                || model.change() == CraftScreenViewModel.Change.OUTCOME) pending = false;
            if (model.change() == CraftScreenViewModel.Change.INVENTORY) materialPending = false;
            listener.accept(model);
        }, executor);
        var ownedController = controller;
        window.scope().addCleanup(() -> {
            ownedController.onClose();
            window = null;
            controller = null;
            pending = false;
            materialPending = false;
            listener = ignored -> {};
        });
        controller.onOpen(window.scope());
        return window;
    }

    public CraftContext context() { return context; }

    public CraftScreenViewModel model() {
        return controller == null ? source.snapshot() : controller.viewModel();
    }

    public boolean busy() {
        // 通用入口拒绝只回 event_alert，不能把缺少 craft 回执变成永久禁用。
        if (pending && clock.getAsLong() >= pendingUntil) pending = false;
        if (materialPending && clock.getAsLong() >= pendingUntil) materialPending = false;
        return pending || materialPending || model().session().active() || awaitingClose();
    }

    private boolean awaitingClose() {
        if (closingSnapshot == null) return false;
        var current = source.snapshot();
        if (clock.getAsLong() >= closingUntil
            || manager.key(DEFINITION.windowType(), "player").connectionGeneration() != closingGeneration
            || !current.session().equals(closingSnapshot.session())
            || (current.inventoryRevision() > closingSnapshot.inventoryRevision()
                && !current.session().active() && current.inventory().craftMaterials().isEmpty())
            || !current.latestOutcome().equals(closingSnapshot.latestOutcome())) {
            closingSnapshot = null;
        }
        return closingSnapshot != null;
    }

    public boolean available() { return available.test(context); }

    public void listen(Consumer<CraftScreenViewModel> listener) {
        this.listener = listener;
    }

    public UiIntentResult start(String recipeId, int quantity) {
        if (window == null || window.closed() || busy()) return UiIntentResult.rejected("制作会话忙碌或已关闭");
        var recipe = model().recipe(recipeId).orElse(null);
        if (!available() || recipe == null || !context.accepts(recipe) || !recipe.unlocked()
            || !recipe.skillSatisfied(model().skills()) || quantity < 1
            || quantity > CraftInventoryCounter.maxCraftable(recipe, model().inventory())) {
            return UiIntentResult.rejected("当前工位、技艺或材料不满足制作条件");
        }
        pending = true;
        pendingUntil = clock.getAsLong() + REQUEST_WAIT_MS;
        var result = controller.intentSink().dispatch(new CraftIntent.Start(recipeId, quantity));
        if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) pending = false;
        listener.accept(model());
        return result;
    }

    public UiIntentResult material(String recipeId, long instanceId, boolean returning, long revision) {
        if (window == null || window.closed() || busy()) return UiIntentResult.rejected("制作会话忙碌");
        var recipe = model().recipe(recipeId).orElse(null);
        if (!returning && (!available() || recipe == null || !context.accepts(recipe) || !recipe.unlocked())) {
            return UiIntentResult.rejected("当前配方不可放入材料");
        }
        materialPending = true;
        pendingUntil = clock.getAsLong() + REQUEST_WAIT_MS;
        var result = controller.intentSink().dispatch(new CraftIntent.Material(recipeId, instanceId, returning, revision));
        if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) materialPending = false;
        listener.accept(model());
        return result;
    }

    public void returnAll() {
        if (window == null || window.closed() || busy() || model().inventory().craftMaterials().isEmpty()) return;
        materialPending = true;
        pendingUntil = clock.getAsLong() + REQUEST_WAIT_MS;
        var result = controller.intentSink().dispatch(new CraftIntent.Cancel());
        if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) materialPending = false;
        listener.accept(model());
    }

    /** 传入窗口实例，避免已关闭视图的迟到回调取消重新打开的会话。 */
    public void close(UiWindowManager.WindowState expected) {
        if (window == null || window != expected || window.closed()) return;
        if ((busy() || !model().inventory().craftMaterials().isEmpty()) && !awaitingClose()) {
            // 取消与开始是不同服务端事件队列，重开不能在取消结算前再提交新任务。
            closingSnapshot = source.snapshot();
            closingGeneration = window.key().connectionGeneration();
            closingUntil = clock.getAsLong() + REQUEST_WAIT_MS;
            var result = controller.intentSink().dispatch(new CraftIntent.Cancel());
            if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) closingSnapshot = null;
        }
        manager.close(window.key());
    }
}
