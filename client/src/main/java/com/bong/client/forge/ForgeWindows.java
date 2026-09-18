package com.bong.client.forge;

import com.bong.client.forge.screen.ConsecrationPanelComponent;
import com.bong.client.forge.screen.InscriptionPanelComponent;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;
import net.minecraft.util.math.BlockPos;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.function.LongSupplier;
import java.util.function.Predicate;

/** 工位窗口所有者。开炉前关闭返还材料，开炉后由再次交互工位恢复炉次。 */
public final class ForgeWindows {
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "forge", "forge-window", 300, 190, Set.of(UiWindowDefinition.Capability.STATION));
    private final UiWindowManager manager;
    private final UiStateSource<ForgeViewModel> source;
    private final UiIntentSink<ForgeIntent> sink;
    private final Predicate<BlockPos> reachable;
    private final LongSupplier clock;
    private UiWindowManager.WindowState window;
    private BlockPos station;
    private ForgeViewModel model;
    private ForgeViewModel pendingSnapshot;
    private long pendingUntil;
    private String feedback = "";
    private boolean injecting;
    private ForgeOutcomeStore.Snapshot outcome = ForgeOutcomeStore.Snapshot.empty();

    public ForgeWindows(UiWindowManager manager, UiStateSource<ForgeViewModel> source,
                        UiIntentSink<ForgeIntent> sink, Predicate<BlockPos> reachable, LongSupplier clock) {
        this.manager = manager;
        this.source = source;
        this.sink = sink;
        this.reachable = reachable;
        this.clock = clock;
        model = source.snapshot();
    }

    public UiWindowManager.WindowState open(BlockPos position, UiWindowManager.Rect bounds) {
        if (window != null && !position.equals(station)) close(window);
        station = position.toImmutable();
        var key = manager.key(DEFINITION.windowType(), position.toShortString());
        if (window != null && !window.closed()) {
            manager.openOrFocus(DEFINITION, key, bounds);
            refresh();
            return window;
        }
        window = manager.openOrFocus(DEFINITION, key, bounds);
        if (window.scope().isClosed()) throw new IllegalStateException("锻造窗口已关闭");
        var owned = window;
        if (!owned.key().equals(key)) throw new IllegalStateException("锻造窗口身份不符");
        owned.scope().addCleanup(() -> {
            if (window == owned) {
                window = null;
                feedback = "";
                pendingSnapshot = null;
                injecting = false;
                outcome = ForgeOutcomeStore.Snapshot.empty();
            }
        });
        refresh();
        return window;
    }

    public void refresh() {
        var next = source.snapshot();
        // 制坯失败可以直接回 outcome 而不创建 session；只归属本窗口发出的起炉请求。
        boolean outcomeChanged = !next.outcome().equals(model.outcome());
        if (outcomeChanged && pendingSnapshot != null
            && !pendingSnapshot.session().active() && pendingSnapshot.blueprint() != null
            && pendingSnapshot.blueprint().id().equals(next.outcome().blueprintId())) outcome = next.outcome();
        if (outcomeChanged && next.session().sessionId() > 0
            && next.outcome().sessionId() == next.session().sessionId()) {
            outcome = next.outcome();
        }
        model = next;
        if (pendingSnapshot != null && (!model.session().equals(pendingSnapshot.session())
            || !model.blueprints().equals(pendingSnapshot.blueprints()) || model.page() != pendingSnapshot.page()
            || !model.outcome().equals(pendingSnapshot.outcome()) || model.inventoryRevision() != pendingSnapshot.inventoryRevision())) {
            pendingSnapshot = null;
            feedback = "";
        }
        if (pendingSnapshot != null && clock.getAsLong() >= pendingUntil) {
            pendingSnapshot = null;
            feedback = "尚未收到确认，请检查材料与工位后重试。";
            injecting = false;
        }
    }

    public ForgeViewModel model() { return model; }
    public boolean pending() { return pendingSnapshot != null; }
    public boolean available() {
        return window != null && !window.closed() && station.equals(model.station().pos())
            && reachable.test(station) && model.station().integrity() > 0;
    }
    public String feedback() { return feedback; }
    public ForgeOutcomeStore.Snapshot outcome() { return outcome; }

    public void endInjection() { injecting = false; }

    /** 只有玩家关闭窗口才返还准备材料；断线清理不向下一条连接发送请求。 */
    public void close(UiWindowManager.WindowState expected) {
        if (window != expected || window == null || window.closed()) return;
        refresh();
        if (!model.session().active() && model.blueprint() != null
            && (!model.preparedMaterials().isEmpty() || pendingSnapshot != null)) {
            sink.dispatch(new ForgeIntent.Material(station, model.blueprint().id(), null, true, model.inventoryRevision()));
        }
        manager.close(window.key());
    }

    public void beginInjection() {
        if (available() && !pending() && "consecration".equals(model.session().currentStep())) injecting = true;
    }

    /** 由窗口输入所有者逐 tick 授权；失焦、最小化或松开后不再发包。 */
    public void tickInjection(boolean focused) {
        if (!focused || !available() || window.minimized()
            || !model.session().active() || !"consecration".equals(model.session().currentStep())) injecting = false;
        if (injecting && !pending() && inject().kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) injecting = false;
    }

    public UiIntentResult material(Long id, boolean returning) {
        refresh();
        if (!available() || pending() || model.session().active() || model.blueprint() == null) {
            return UiIntentResult.rejected("当前不能移入或取回材料");
        }
        return send(new ForgeIntent.Material(station, model.blueprint().id(), id, returning, model.inventoryRevision()));
    }

    public UiIntentResult start() {
        refresh();
        if (!available() || pending() || model.session().active() || model.blueprint() == null || model.preparedMaterials().isEmpty()) {
            return UiIntentResult.rejected("请先选择图谱和投料");
        }
        Map<String, Integer> materials = new LinkedHashMap<>();
        for (var item : model.preparedMaterials()) {
            materials.merge(item.forgeMaterialKey(), item.stackCount(), Integer::sum);
        }
        if (materials.isEmpty()) return UiIntentResult.rejected("所选材料已不可用");
        return send(new ForgeIntent.Start(station, model.blueprint().id(), materials.entrySet().stream()
            .map(entry -> new ClientRequestProtocol.ForgeMaterial(entry.getKey(), entry.getValue())).toList()));
    }

    public UiIntentResult turnPage(int delta) {
        if (!available() || pending() || model.session().active() || !model.preparedMaterials().isEmpty()) return UiIntentResult.rejected("请先取回炉中的材料再切换图谱");
        if (model.page() + delta < 0 || model.page() + delta >= model.blueprints().size()) return UiIntentResult.rejected("没有更多图谱");
        return send(new ForgeIntent.TurnPage(delta));
    }

    public UiIntentResult advance() { return act("", new ForgeIntent.Advance(model.session().sessionId())); }
    public UiIntentResult hit(ClientRequestProtocol.TemperBeat beat) {
        return act("tempering", new ForgeIntent.Hit(model.session().sessionId(), beat));
    }
    public UiIntentResult inscribe(long itemId) {
        var item = model.materials().stream().filter(value -> value.instanceId() == itemId).findFirst().orElse(null);
        var slots = InscriptionPanelComponent.renderStateFrom(model.session());
        if (item == null || item.inscriptionId().isBlank() || slots.failed() || slots.filledCount() >= slots.maxSlots()) {
            return UiIntentResult.rejected("残卷或铭文槽不可用");
        }
        return act("inscription", new ForgeIntent.Inscribe(model.session().sessionId(), item.inscriptionId()));
    }
    public UiIntentResult inject() {
        if (!ConsecrationPanelComponent.renderStateFrom(model.session(), model.inventory().realm()).canInject()) {
            return UiIntentResult.rejected("当前不能注入真元");
        }
        return act("consecration", new ForgeIntent.Inject(model.session().sessionId(), ConsecrationPanelComponent.QI_PER_TICK));
    }

    private UiIntentResult act(String step, ForgeIntent intent) {
        if (!available() || pending() || !model.session().active() || model.session().sessionId() <= 0
            || (!step.isEmpty() && !step.equals(model.session().currentStep()))) return UiIntentResult.rejected("当前工序不可操作");
        return send(intent);
    }

    private UiIntentResult send(ForgeIntent intent) {
        var result = sink.dispatch(intent);
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            if (intent instanceof ForgeIntent.Start || intent instanceof ForgeIntent.TurnPage
                || intent instanceof ForgeIntent.Material move && !move.returning()) {
                outcome = ForgeOutcomeStore.Snapshot.empty();
            }
            pendingSnapshot = model;
            pendingUntil = clock.getAsLong() + 5_000;
            feedback = "等待工位确认…";
        }
        return result;
    }
}
