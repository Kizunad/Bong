package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.Executor;
import java.util.function.Consumer;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftScreenControllerTest {
    @Test
    void openDeliversSnapshotOnceAndDoesNotDuplicateSubscription() {
        MutableSource source = new MutableSource(model(0L, CraftScreenViewModel.Change.INITIAL));
        List<CraftScreenViewModel> rendered = new ArrayList<>();
        CraftScreenController controller = new CraftScreenController(
            source,
            ignored -> UiIntentResult.accepted(),
            rendered::add
        );
        DefaultUiScreenScope scope = openScope();

        controller.onOpen(scope);
        controller.onOpen(scope);

        assertEquals(List.of(source.snapshot()), rendered,
            "controller open 必须先渲染唯一 snapshot，重复 open 不得重复首帧");
        assertEquals(1, source.registrations,
            "同一 controller 重复 open 不得重复登记 Store listener");
        assertEquals(source.snapshot(), controller.viewModel());
    }

    @Test
    void updatesStopAtScopeCloseAndControllerRejectsLateIntent() {
        MutableSource source = new MutableSource(model(0L, CraftScreenViewModel.Change.INITIAL));
        List<CraftScreenViewModel> rendered = new ArrayList<>();
        List<CraftIntent> sent = new ArrayList<>();
        CraftScreenController controller = new CraftScreenController(
            source,
            intent -> {
                sent.add(intent);
                return UiIntentResult.accepted();
            },
            rendered::add
        );
        CraftIntent.Start start = new CraftIntent.Start("rough_handle", 1);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, controller.intentSink().dispatch(start).kind(),
            "未打开的 controller 不得发送 intent");
        DefaultUiScreenScope scope = openScope();
        controller.onOpen(scope);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, controller.intentSink().dispatch(start).kind());

        CraftScreenViewModel update = model(1L, CraftScreenViewModel.Change.RECIPES);
        source.emit(update);
        assertEquals(update, controller.viewModel());
        assertEquals(2, rendered.size(), "首帧之后的 source update 必须可见");

        scope.close();
        controller.onClose();
        source.emit(model(2L, CraftScreenViewModel.Change.SESSION));

        assertEquals(2, rendered.size(), "scope 关闭后的 late update 必须被丢弃");
        assertEquals(0, source.listenerCount(), "scope close 必须真正移除 source listener");
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, controller.intentSink().dispatch(start).kind(),
            "controller close 后的 late intent 必须 fail closed");
        assertEquals(List.of(start), sent, "只有打开期间的一个 intent 可以到达 transport");
    }

    @Test
    void scopeCloseRejectsIntentBeforeHostCloseCallbackRuns() {
        MutableSource source = new MutableSource(model(0L, CraftScreenViewModel.Change.INITIAL));
        List<CraftIntent> sent = new ArrayList<>();
        CraftScreenController controller = new CraftScreenController(
            source,
            intent -> {
                sent.add(intent);
                return UiIntentResult.accepted();
            },
            ignored -> {
            }
        );
        DefaultUiScreenScope scope = openScope();
        controller.onOpen(scope);
        scope.close();

        UiIntentResult result = controller.intentSink().dispatch(new CraftIntent.Cancel());

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind(),
            "scope 已标记 closed 时，即使 host 尚未回调 onClose，也必须拒绝 late intent");
        assertTrue(sent.isEmpty(), "scope 关闭后的 intent 不得到达 transport");
    }

    @Test
    void closedControllerCannotReopen() {
        MutableSource source = new MutableSource(model(0L, CraftScreenViewModel.Change.INITIAL));
        CraftScreenController controller = new CraftScreenController(
            source,
            ignored -> UiIntentResult.accepted(),
            ignored -> {
            }
        );
        DefaultUiScreenScope first = openScope();
        controller.onOpen(first);
        first.close();
        controller.onClose();

        assertThrows(IllegalStateException.class, () -> controller.onOpen(openScope()),
            "已关闭 controller 不得跨 screen session 复用");
    }

    @Test
    void nullDependenciesFailAtConstructionBoundary() {
        MutableSource source = new MutableSource(model(0L, CraftScreenViewModel.Change.INITIAL));
        assertThrows(NullPointerException.class, () -> new CraftScreenController(
            null, ignored -> UiIntentResult.accepted(), ignored -> {
            }
        ));
        assertThrows(NullPointerException.class, () -> new CraftScreenController(
            source, null, ignored -> {
            }
        ));
        assertThrows(NullPointerException.class, () -> new CraftScreenController(
            source, ignored -> UiIntentResult.accepted(), null
        ));
        assertThrows(NullPointerException.class, () -> new CraftScreenController(
            source, ignored -> UiIntentResult.accepted(), ignored -> {
            }, null
        ));
    }

    @Test
    void stateUpdatesAreDispatchedOnlyThroughTheConfiguredUiExecutor() {
        MutableSource source = new MutableSource(model(0L, CraftScreenViewModel.Change.INITIAL));
        List<Runnable> queued = new ArrayList<>();
        List<CraftScreenViewModel> rendered = new ArrayList<>();
        Executor executor = queued::add;
        CraftScreenController controller = new CraftScreenController(
            source,
            ignored -> UiIntentResult.accepted(),
            rendered::add,
            executor
        );
        DefaultUiScreenScope scope = openScope();

        controller.onOpen(scope);
        source.emit(model(1L, CraftScreenViewModel.Change.RECIPES));

        assertTrue(rendered.isEmpty(), "Store 线程只能提交 UI task，不得直接触碰 view listener");
        assertEquals(1, queued.size(), "连续状态变化应合并到一个有序 drain task");

        queued.remove(0).run();

        assertEquals(
            List.of(
                CraftScreenViewModel.Change.INITIAL,
                CraftScreenViewModel.Change.RECIPES
            ),
            rendered.stream().map(CraftScreenViewModel::change).toList(),
            "UI executor drain 必须按状态产生顺序交付首帧和后续变化"
        );
    }

    @Test
    void subscriptionWindowUpdateIsDeliveredWithoutDuplicateCatchUpFrame() {
        CraftScreenViewModel initial = model(0L, CraftScreenViewModel.Change.INITIAL);
        CraftScreenViewModel duringSubscribe = model(1L, CraftScreenViewModel.Change.RECIPES);
        MutableSource source = new MutableSource(initial, duringSubscribe);
        List<CraftScreenViewModel> rendered = new ArrayList<>();
        CraftScreenController controller = new CraftScreenController(
            source,
            ignored -> UiIntentResult.accepted(),
            rendered::add
        );
        DefaultUiScreenScope scope = openScope();

        controller.onOpen(scope);

        assertEquals(
            List.of(CraftScreenViewModel.Change.INITIAL, CraftScreenViewModel.Change.RECIPES),
            rendered.stream().map(CraftScreenViewModel::change).toList(),
            "snapshot 与订阅登记之间的更新必须补发一次且不能重复补发"
        );
    }

    private static DefaultUiScreenScope openScope() {
        DefaultUiScreenScope scope = new DefaultUiScreenScope();
        scope.onOpen();
        return scope;
    }

    private static CraftScreenViewModel model(long revision, CraftScreenViewModel.Change change) {
        return new CraftScreenViewModel(
            revision,
            change,
            List.of(),
            InventoryModel.empty(),
            SkillSetSnapshot.empty(),
            CraftSessionStateView.IDLE,
            Optional.empty()
        );
    }

    private static final class MutableSource implements UiStateSource<CraftScreenViewModel> {
        private final List<Consumer<? super CraftScreenViewModel>> listeners = new ArrayList<>();
        private final CraftScreenViewModel emitDuringSubscribe;
        private CraftScreenViewModel current;
        private int registrations;

        private MutableSource(CraftScreenViewModel current) {
            this(current, null);
        }

        private MutableSource(
            CraftScreenViewModel current,
            CraftScreenViewModel emitDuringSubscribe
        ) {
            this.current = current;
            this.emitDuringSubscribe = emitDuringSubscribe;
        }

        @Override
        public CraftScreenViewModel snapshot() {
            return current;
        }

        @Override
        public UiSubscription subscribe(Consumer<? super CraftScreenViewModel> listener) {
            registrations++;
            listeners.add(listener);
            if (emitDuringSubscribe != null) {
                current = emitDuringSubscribe;
                listener.accept(emitDuringSubscribe);
            }
            return UiSubscriptions.once(() -> listeners.remove(listener));
        }

        private void emit(CraftScreenViewModel next) {
            current = next;
            List.copyOf(listeners).forEach(listener -> listener.accept(next));
        }

        private int listenerCount() {
            return listeners.size();
        }
    }
}
