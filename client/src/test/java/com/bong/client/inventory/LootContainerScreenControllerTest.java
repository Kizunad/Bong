package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import java.util.Objects;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class LootContainerScreenControllerTest {
    @Test
    void intentIsRejectedBeforeOpenAfterCloseAndForAnotherSession() {
        LootContainerSession.Open open = session(21L);
        AtomicReference<LootContainerScreenViewModel> state = new AtomicReference<>(
            new LootContainerScreenViewModel(0L, open, InventoryModel.empty())
        );
        AtomicReference<LootContainerIntent> sent = new AtomicReference<>();
        LootContainerScreenController controller = new LootContainerScreenController(
            new SingleValueSource(state),
            intent -> {
                sent.set(intent);
                return UiIntentResult.accepted();
            },
            ignored -> { },
            Runnable::run
        );
        LootContainerIntent move = new LootContainerIntent.Move(
            21L, 42L, "main", 0, 0, "ext_21", 0, 0
        );

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, controller.intentSink().dispatch(move).kind(),
            "未打开的搜刮 controller 不得发送 intent");
        DefaultUiScreenScope scope = new DefaultUiScreenScope();
        scope.onOpen();
        controller.onOpen(scope);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, controller.intentSink().dispatch(move).kind());
        assertEquals(move, sent.get());

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED,
            controller.intentSink().dispatch(new LootContainerIntent.Move(
                22L, 42L, "main", 0, 0, "ext_22", 0, 0
            )).kind(), "不同 session 的移动必须 fail closed");
        scope.close();
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, controller.intentSink().dispatch(move).kind(),
            "scope 关闭后不得再到达 transport");
        controller.onClose();
        assertThrows(IllegalStateException.class, () -> controller.onOpen(openScope()),
            "关闭后的 controller 不得复用到另一份搜刮会话");
    }

    private static DefaultUiScreenScope openScope() {
        DefaultUiScreenScope scope = new DefaultUiScreenScope();
        scope.onOpen();
        return scope;
    }

    private static LootContainerSession.Open session(long id) {
        return new LootContainerSession.Open(id, "dead_drop", "rare", 2, 3, 0L, java.util.List.of());
    }

    private static final class SingleValueSource implements UiStateSource<LootContainerScreenViewModel> {
        private final AtomicReference<LootContainerScreenViewModel> state;

        private SingleValueSource(AtomicReference<LootContainerScreenViewModel> state) {
            this.state = Objects.requireNonNull(state);
        }

        @Override public LootContainerScreenViewModel snapshot() { return state.get(); }

        @Override
        public UiSubscription subscribe(java.util.function.Consumer<? super LootContainerScreenViewModel> listener) {
            Objects.requireNonNull(listener);
            return UiSubscriptions.closed();
        }
    }
}
