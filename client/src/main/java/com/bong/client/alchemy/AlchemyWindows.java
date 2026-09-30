package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;
import net.minecraft.util.math.BlockPos;

import java.util.Set;
import java.util.concurrent.Executor;
import java.util.function.Predicate;
import java.util.function.LongSupplier;
import java.util.stream.Stream;

/** 一个客户端只持有一个炉次投影；窗口隐藏不结束炼丹，显式收丹才结算。 */
public final class AlchemyWindows {
    private static final long RESPONSE_TIMEOUT_MS = 5_000;
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "alchemy", "alchemy-window", 100, 100, Set.of(UiWindowDefinition.Capability.STATION));
    private final UiWindowManager manager;
    private final UiStateSource<AlchemyScreenViewModel> source;
    private final UiIntentSink<AlchemyIntent> sink;
    private final Executor executor;
    private final Predicate<BlockPos> reachable;
    private final LongSupplier clock;
    private UiWindowManager.WindowState window;
    private AlchemyScreenController controller;
    private BlockPos position;
    private String feedback = "";
    private final java.util.Map<String, String> materialNames = new java.util.HashMap<>();
    private AlchemyScreenViewModel openBaseline;
    private AlchemyScreenViewModel pendingBaseline;
    private AlchemyIntent pendingIntent;
    private InventoryItem pendingMaterial;
    private long pendingUntil;
    private long openUntil;
    private long effectRevision;
    private ConfirmedEffect confirmedEffect;
    private AlchemyScreenViewModel settlementBaseline;
    private long settlementUntil;
    private ConfirmedResult confirmedResult;
    /** 只在相关权威数据到达后发布，重绘和重复快照不会再次产生表现事件。 */
    public record ConfirmedEffect(long revision, String material, boolean qi, InventoryItem item, int count) {}
    public record ConfirmedResult(
        long revision,
        AlchemyResultEffect effect,
        InventoryItem item,
        String displayName,
        String bucket
    ) {}

    public AlchemyWindows(UiWindowManager manager, UiStateSource<AlchemyScreenViewModel> source,
                          UiIntentSink<AlchemyIntent> sink, Executor executor, Predicate<BlockPos> reachable) {
        this(manager, source, sink, executor, reachable, System::currentTimeMillis);
    }

    public AlchemyWindows(UiWindowManager manager, UiStateSource<AlchemyScreenViewModel> source,
                          UiIntentSink<AlchemyIntent> sink, Executor executor, Predicate<BlockPos> reachable,
                          LongSupplier clock) {
        this.manager = manager;
        this.source = source;
        this.sink = sink;
        this.executor = executor;
        this.reachable = reachable;
        this.clock = clock;
    }

    public UiWindowManager.WindowState open(BlockPos target, UiWindowManager.Rect bounds) {
        if (window != null && !target.equals(position)) {
            manager.close(window.key());
            // 炉位切换使旧 session 不再属于当前窗口；等待新炉快照时保持空闲，
            // 不能让旧炉的进度被暂时展示或用于发送操作。
            AlchemySessionStore.replace(AlchemySessionStore.Snapshot.empty());
        }
        var key = manager.key(DEFINITION.windowType(), target.toShortString());
        if (window != null && !window.closed()) {
            manager.openOrFocus(DEFINITION, key, bounds);
            refresh();
            return window;
        }
        position = target.toImmutable();
        window = manager.openOrFocus(DEFINITION, key, bounds);
        controller = new AlchemyScreenController(source, sink, ignored -> {}, executor);
        var owned = window;
        var ownedController = controller;
        owned.scope().addCleanup(() -> {
            Throwable primary = null;
            try {
                ownedController.onClose();
            } catch (Throwable failure) {
                primary = failure;
            }
            try {
                if (window == owned) {
                    window = null;
                    controller = null;
                    position = null;
                    feedback = "";
                    openBaseline = null;
                    pendingBaseline = null;
                    pendingIntent = null;
                    pendingMaterial = null;
                    confirmedEffect = null;
                    settlementBaseline = null;
                    confirmedResult = null;
                }
            } catch (Throwable cleanupFailure) {
                if (primary == null) primary = cleanupFailure;
                else if (primary != cleanupFailure) primary.addSuppressed(cleanupFailure);
            }
            if (primary != null) {
                AlchemyWindows.<RuntimeException>throwUnchecked(primary);
            }
        });
        controller.onOpen(owned.scope());
        return window;
    }

    /** 重新交互须收到新的炉信息及 session；不清除 Store 中用于再次交互的炉坐标。 */
    public void awaitOpenResponse() {
        settlementBaseline = null;
        confirmedResult = null;
        openBaseline = model();
        openUntil = clock.getAsLong() + RESPONSE_TIMEOUT_MS;
        pendingBaseline = null;
        pendingIntent = null;
        feedback = "";
    }

    public void refresh() {
        // Inspect 可持续打开；工位绑定必须逐 tick 校验，不能等玩家再次点击才发现离炉。
        if (window != null && (position == null || !reachable.test(position))) {
            manager.close(window.key());
            return;
        }
        if (controller != null) controller.refreshFromSource();
        ownedItems(model().inventory()).forEach(item -> materialNames.put(item.itemId(), item.displayName()));
        confirmSettlement();
        if (openBaseline != null) {
            if (model().furnace() != openBaseline.furnace() && model().session() != openBaseline.session()) {
                openBaseline = null;
                feedback = "";
            } else if (clock.getAsLong() >= openUntil) {
                if (feedback.isBlank()) feedback = "无法连接这座丹炉，请靠近后重试。";
            }
        }
        if (pendingIntent != null) {
            if (responseArrived()) {
                if (pendingIntent instanceof AlchemyIntent.FeedSlot feed) {
                    confirmedEffect = new ConfirmedEffect(++effectRevision, feed.material(), false, pendingMaterial, feed.count());
                } else if (pendingIntent instanceof AlchemyIntent.PlaceIncense) {
                    confirmedEffect = new ConfirmedEffect(++effectRevision, "incense", false, null, 1);
                } else if (pendingIntent instanceof AlchemyIntent.InjectQi
                    && model().session().qiInjected() > pendingBaseline.session().qiInjected()) {
                    confirmedEffect = new ConfirmedEffect(++effectRevision, "", true, null, 0);
                }
                pendingIntent = null;
                pendingBaseline = null;
                pendingMaterial = null;
            } else if (clock.getAsLong() >= pendingUntil) {
                pendingIntent = null;
                pendingBaseline = null;
                pendingMaterial = null;
                if (feedback.isBlank()) feedback = "这次操作未能完成，请重试。";
            }
        }
    }

    private boolean responseArrived() {
        var before = pendingBaseline;
        var after = model();
        if (pendingIntent instanceof AlchemyIntent.TurnPage || pendingIntent instanceof AlchemyIntent.LearnRecipe) {
            return after.recipes() != before.recipes();
        }
        if (pendingIntent instanceof AlchemyIntent.Ignite) return after.session().isActive();
        if (pendingIntent instanceof AlchemyIntent.TakeBack) {
            return before.furnace().hasSession() && !after.furnace().hasSession();
        }
        if (pendingIntent instanceof AlchemyIntent.FeedSlot feed) {
            // 阶段也可能因为超时变为 missed，不能把它作为投料成功的证据。
            if (feed.slot() >= after.session().stages().size()) return false;
            var ingredients = after.session().stages().get(feed.slot()).ingredients();
            var oldIngredients = before.session().stages().get(feed.slot()).ingredients();
            int oldCount = oldIngredients.stream().filter(item -> item.material().equals(feed.material()))
                .mapToInt(com.bong.client.alchemy.state.AlchemySessionStore.IngredientHint::inserted).sum();
            return ingredients.stream().anyMatch(item -> item.material().equals(feed.material())
                && item.inserted() >= oldCount + feed.count())
                && materialCount(after.inventory(), feed.material()) <= materialCount(before.inventory(), feed.material()) - feed.count();
        }
        if (pendingIntent instanceof AlchemyIntent.PlaceIncense place) {
            var beforeItem = findOwnedItem(before.inventory(), place.itemInstanceId());
            var afterItem = findOwnedItem(after.inventory(), place.itemInstanceId());
            return beforeItem != null
                && (afterItem == null || afterItem.stackCount() < beforeItem.stackCount())
                && after.session().incense().state() == AlchemyIncenseTimer.State.BURNING
                && after.session().incense().kind().equals(beforeItem.itemId())
                && !after.session().incense().equals(before.session().incense());
        }
        if (pendingIntent instanceof AlchemyIntent.InjectQi) {
            return after.session().qiInjected() != before.session().qiInjected();
        }
        return after.session().tempCurrent() != before.session().tempCurrent();
    }

    public boolean pending() { return pendingIntent != null; }
    public ConfirmedEffect confirmedEffect() { return confirmedEffect; }
    public ConfirmedResult confirmedResult() { return confirmedResult; }

    private void confirmSettlement() {
        if (settlementBaseline == null) return;
        if (clock.getAsLong() >= settlementUntil) {
            settlementBaseline = null;
            return;
        }
        var after = model();
        var history = after.history();
        var before = settlementBaseline.history();
        var result = history.isEmpty() ? null : history.get(history.size() - 1);
        var previous = before.isEmpty() ? null : before.get(before.size() - 1);
        // 空炉、结束进度与结算事件可以分包到达，必须齐备且对应本次主动收取的丹方。
        if (position.equals(after.furnace().pos()) && !after.furnace().hasSession()
            && after.session() != settlementBaseline.session() && !after.session().isActive()
            && after.session().recipeId().equals(settlementBaseline.session().recipeId())
            && result != null && result != previous
            && result.recipeId().equals(settlementBaseline.session().recipeId())) {
            var session = after.session();
            var effect = AlchemyResultEffect.fromOutcome(result.bucket(), session.elapsedTicks() < session.targetTicks());
            if (effect != null) {
                InventoryItem resultItem = findResultItem(
                    settlementBaseline.inventory(), after.inventory(), result, effect);
                // 结算、库存和历史可能分包到达；没有本炉新增的目标实例时继续等，
                // 避免拿背包里上一炉同名物品冒充本次产物。
                if (resultItem == null && hasExpectedResult(after.inventory(), result, effect)) {
                    return;
                }
                String displayName = resultItem == null
                    ? resultDisplayName(result, effect)
                    : resultItem.displayName();
                confirmedResult = new ConfirmedResult(
                    ++effectRevision, effect, resultItem, displayName, result.bucket());
                feedback = settlementFeedback(result, session);
            }
            settlementBaseline = null;
        }
    }

    private static InventoryItem findResultItem(
        InventoryModel before,
        InventoryModel inventory,
        com.bong.client.alchemy.state.AlchemyAttemptHistoryStore.Entry result,
        AlchemyResultEffect effect
    ) {
        String itemId = resultItemId(result, effect);
        final String expected = itemId;
        var previous = new java.util.HashMap<Long, Integer>();
        ownedItems(before).filter(item -> expected.equals(item.itemId()))
            .forEach(item -> previous.put(item.instanceId(), item.stackCount()));
        return ownedItems(inventory)
            .filter(item -> expected.equals(item.itemId()))
            .filter(item -> !previous.containsKey(item.instanceId())
                || item.stackCount() > previous.get(item.instanceId()))
            .findFirst().orElse(null);
    }

    private static boolean hasExpectedResult(
        InventoryModel inventory,
        com.bong.client.alchemy.state.AlchemyAttemptHistoryStore.Entry result,
        AlchemyResultEffect effect
    ) {
        String expected = resultItemId(result, effect);
        return ownedItems(inventory).anyMatch(item -> expected.equals(item.itemId()));
    }

    private static String resultItemId(
        com.bong.client.alchemy.state.AlchemyAttemptHistoryStore.Entry result,
        AlchemyResultEffect effect
    ) {
        if (effect == AlchemyResultEffect.EARLY_TAKE) return "alchemy_residue_processing_dregs";
        if (result.flawedPath()) return "alchemy_residue_flawed_pill";
        if (result.pill() != null && !result.pill().isBlank()
            && effect != AlchemyResultEffect.WASTE && effect != AlchemyResultEffect.EXPLODE) {
            return result.pill();
        }
        return "alchemy_residue_failed_pill";
    }

    private static String resultDisplayName(
        com.bong.client.alchemy.state.AlchemyAttemptHistoryStore.Entry result,
        AlchemyResultEffect effect
    ) {
        if (result.pill() != null && !result.pill().isBlank()
            && effect != AlchemyResultEffect.WASTE && effect != AlchemyResultEffect.EXPLODE) {
            return result.pill();
        }
        return switch (effect) {
            case EXPLODE -> "炸炉残渣";
            case EARLY_TAKE -> "余热药渣";
            case FLAWED -> "残缺丹渣";
            case WASTE -> "炼丹炉渣";
            default -> "炼制残渣";
        };
    }

    private static String settlementFeedback(
        com.bong.client.alchemy.state.AlchemyAttemptHistoryStore.Entry result,
        com.bong.client.alchemy.state.AlchemySessionStore.Snapshot session
    ) {
        if (result.bucket().equals("waste") && session.qiInjected() + 1e-6 < session.qiTarget()) {
            return String.format("炉渣：真元不足（%.1f / %.1f），补足真元后才会开始稳定炼制。",
                session.qiInjected(), session.qiTarget());
        }
        if (result.bucket().equals("explode")) return "炸炉：火候超过安全范围，丹炉反噬。";
        if (result.bucket().equals("waste")) return "炉渣：火候、时长或投料阶段偏差过大。";
        if (result.bucket().equals("flawed")) return "残缺成品：偏差超过完美区间。";
        if (result.bucket().equals("good")) return "成品已成：火候有小幅偏差。";
        return "成品已成：火候与时长保持稳定。";
    }

    /** 连接或世界切换后清除上一炉的香效表现，避免跨会话复用确认事件。 */
    public void resetIncense() {
        effectRevision = 0;
        confirmedEffect = null;
        settlementBaseline = null;
        confirmedResult = null;
    }

    public AlchemyIncenseTimer.Snapshot incenseView() {
        return model().session().incense();
    }

    public UiIntentResult placeIncense(InventoryItem item) {
        refresh();
        if (!active()) return reject("请先起炉再投香");
        var owned = findOwnedItem(model().inventory(), item == null ? 0L : item.instanceId());
        if (owned == null || !owned.itemId().startsWith("incense_")) return reject("请从背包拖入香料");
        return send(new AlchemyIntent.PlaceIncense(position, owned.instanceId()));
    }

    public void requestIncenseHint() {
        feedback = incenseView().state() == AlchemyIncenseTimer.State.EMPTY
            ? "请从背包拖入香料到香座"
            : "香座上已有一支香。";
    }

    /** 提前收取只做确认提示；真正的结果仍由服务端结算快照决定。 */
    public void requestEarlySettleWarning() {
        feedback = "炉火未停：收取可能烫伤并损失材料，松开后再次按 R 确认";
    }
    public boolean awaitingOpen() { return openBaseline != null; }
    public boolean ready() { return available() && !window.minimized() && !pending(); }
    public AlchemyScreenViewModel model() { return controller == null ? source.snapshot() : controller.viewModel(); }
    public String feedback() { return feedback; }

    public String materialName(String id) {
        return materialNames.getOrDefault(id, "药材");
    }

    public boolean acceptMessage(String message) {
        if (window == null || window.closed() || window.minimized()) return false;
        feedback = message;
        pendingIntent = null;
        pendingBaseline = null;
        pendingMaterial = null;
        return true;
    }
    public boolean available() {
        return openBaseline == null && window != null && !window.closed() && position.equals(model().furnace().pos())
            && reachable.test(position) && model().furnace().integrity() > 0;
    }
    public boolean active() { return available() && model().session().isActive(); }

    public AlchemySessionPresentationPlanner.Presentation presentation() {
        return AlchemySessionPresentationPlanner.describe(model().furnace(), model().session());
    }

    public UiIntentResult ignite() {
        refresh();
        var recipe = model().recipes().current();
        if (recipe == null || model().furnace().hasSession() || model().session().isActive()) return reject("请先选择丹方，或收取当前炉次");
        return send(new AlchemyIntent.Ignite(position, recipe.id()));
    }

    public UiIntentResult turnPage(int delta) {
        refresh();
        if (model().session().isActive() || model().furnace().hasSession()) return reject("本炉炼制期间不能更换丹方");
        return send(new AlchemyIntent.TurnPage(delta));
    }

    public UiIntentResult inject() {
        refresh();
        if (!active()) return reject("请先起炉");
        return send(new AlchemyIntent.InjectQi(position, 1));
    }

    public UiIntentResult temperature(double delta) {
        refresh();
        if (!active()) return reject("请先起炉");
        double current = model().session().tempCurrent();
        double target = Math.max(0, Math.min(1, current + delta));
        if (target == current) return reject(delta < 0 ? "炉火已经最低" : "炉火已经最高");
        return send(new AlchemyIntent.AdjustTemp(position, target));
    }

    public UiIntentResult settle() {
        refresh();
        if (!available() || !model().furnace().hasSession()) return reject("当前没有可收取的炉次");
        return send(new AlchemyIntent.TakeBack(position, 0));
    }

    /** 投料请求不移动本地库存；材料消耗与阶段完成仅由服务端快照确认。 */
    public UiIntentResult feed(InventoryItem dragged) {
        refresh();
        if (!active()) return reject("请先起炉");
        var item = dragged == null ? null : findOwnedItem(model().inventory(), dragged.instanceId());
        if (item == null) return reject("这件药材已不在背包中");
        var stages = model().session().stages();
        for (int stage = 0; stage < Math.min(4, stages.size()); stage++) {
            if (!canFeedStage(stage)) continue;
            for (var ingredient : stages.get(stage).ingredients()) {
                if (!ingredient.material().equals(item.itemId()) || ingredient.remaining() == 0) continue;
                // 分堆拖拽只改变客户端投料投影；服务端仍按 instanceId 校验实际持有量。
                // 快照可能在拖拽期间刷新，先按权威当前堆叠再夹紧请求，避免过量投料。
                int requested = Math.min(Math.max(1, dragged.stackCount()), item.stackCount());
                int count = Math.min(requested, ingredient.remaining());
                var result = send(new AlchemyIntent.FeedSlot(position, stage, item.itemId(), count));
                if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) pendingMaterial = item;
                return result;
            }
        }
        return reject("此时无需再投入这味药材");
    }

    public UiIntentResult learn(InventoryItem dragged) {
        refresh();
        if (model().session().isActive() || model().furnace().hasSession()) return reject("请在本炉结束后研读丹方");
        var item = dragged == null ? null : findOwnedItem(model().inventory(), dragged.instanceId());
        if (item == null || !item.itemId().startsWith("recipe_scroll_")) return reject("请从背包拖入丹方残卷");
        String recipeId = item.itemId().substring("recipe_scroll_".length());
        if (model().recipes().learned().stream().anyMatch(recipe -> recipe.id().equals(recipeId))) {
            return reject("已经学过这张丹方");
        }
        return send(new AlchemyIntent.LearnRecipe(recipeId));
    }

    private static Stream<InventoryItem> ownedItems(InventoryModel inventory) {
        var items = Stream.<InventoryItem>builder();
        inventory.gridItems().forEach(entry -> items.add(entry.item()));
        inventory.hotbar().forEach(items::add);
        inventory.equippedSlots().values().forEach(slot -> {
            slot.worn().forEach(items::add);
            items.add(slot.held());
        });
        return items.build().filter(item -> item != null && !item.isEmpty());
    }

    private static long materialCount(InventoryModel inventory, String material) {
        return ownedItems(inventory).filter(item -> item.itemId().equals(material))
            .mapToLong(InventoryItem::stackCount).sum();
    }

    private static InventoryItem findOwnedItem(InventoryModel inventory, long instanceId) {
        return ownedItems(inventory).filter(item -> item.instanceId() == instanceId).findFirst().orElse(null);
    }

    private UiIntentResult send(AlchemyIntent intent) {
        if (!available() || window.minimized()) return reject("暂时无法操作这座丹炉，请靠近后重试。");
        if (pending()) return UiIntentResult.rejected("操作尚未完成");
        var baseline = model();
        var result = controller.intentSink().dispatch(intent);
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            if (intent instanceof AlchemyIntent.TakeBack) {
                settlementBaseline = baseline;
                settlementUntil = clock.getAsLong() + RESPONSE_TIMEOUT_MS;
            }
            pendingBaseline = baseline;
            pendingIntent = intent;
            pendingUntil = clock.getAsLong() + RESPONSE_TIMEOUT_MS;
            feedback = "";
        } else {
            feedback = result.reason();
        }
        return result;
    }

    public boolean canFeedStage(int stage) {
        var session = model().session();
        if (!active() || stage < 0 || stage >= Math.min(4, session.stages().size())) return false;
        var hint = session.stages().get(stage);
        return !hint.missed() && session.elapsedTicks() >= hint.atTick()
            && session.elapsedTicks() <= (long) hint.atTick() + hint.window();
    }

    private UiIntentResult reject(String message) {
        feedback = message;
        return UiIntentResult.rejected(message);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
