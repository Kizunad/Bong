package com.bong.client.ui;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.scroll.ScrollOpenViewModel;
import com.bong.client.scroll.ScrollReadScreen;
import com.bong.client.scroll.ScrollReadStore;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-bughunt-scroll-read-transition-esc-close-loss-v1 P0：锁定开屏转场期间 Esc 取消仍须
 * 结算残卷阅读协议终态。
 */
class ScreenTransitionScrollCloseTest {
    private record Sent(Identifier channel, String body) {
    }

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void cleanup() {
        ScreenTransitionController.resetForTests();
        ScrollReadStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    @Test
    void cancelPendingScrollOpenClosesStoreAndSendsExactlyOneTerminalRequest() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8))));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_red", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen pendingScreen = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle handle = activatePending(pendingScreen);

        ScreenTransitionController.cancelAndClose(null);
        ScreenTransitionController.cancelAndClose(null);

        assertTrue(handle.cancelled(), "Esc 取消应终止尚未完成的开屏转场");
        assertNull(ScreenTransitionController.activeTransition(), "取消后不应残留 active transition");
        assertNull(ScrollReadStore.snapshot(),
            "视觉上取消残卷开屏时也必须清空阅读 store，不能留下 client 半开会话");
        assertEquals(
            List.of(new Sent(
                new Identifier("bong", "client_request"),
                "{\"type\":\"scroll_read_closed\",\"v\":1}"
            )),
            sent,
            "重复 Esc 也必须恰好发送一条 scroll_read_closed，不能静默丢失或重复发送"
        );
    }

    @Test
    void cancelUnrelatedPendingScreenDoesNotSettleScrollSession() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8))));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_keep", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScreenTransition.TransitionHandle handle = activatePending(new DummyScreen());

        ScreenTransitionController.cancelAndClose(null);

        assertTrue(handle.cancelled(), "不携带终态的普通 screen 仍应正常取消转场");
        assertSame(offer, ScrollReadStore.snapshot(),
            "取消无关 screen 不得误结算仍在等待的残卷会话");
        assertTrue(sent.isEmpty(), "取消无关 screen 不得误发 scroll_read_closed，实际=" + sent);
    }

    @Test
    void cancelFromCurrentScrollToUnrelatedPendingScreenSettlesCurrentSession() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8))));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_current", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen currentScreen = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle handle = activateTransition(currentScreen, new DummyScreen());

        ScreenTransitionController.cancelAndClose(null);
        ScreenTransitionController.cancelAndClose(null);

        assertTrue(handle.cancelled(), "Esc 取消应终止从当前残卷离开的转场");
        assertNull(ScrollReadStore.snapshot(),
            "当前残卷被转场直接移除时也必须结算阅读终态，不能只检查无关 pending screen");
        assertEquals(
            List.of(new Sent(
                new Identifier("bong", "client_request"),
                "{\"type\":\"scroll_read_closed\",\"v\":1}"
            )),
            sent,
            "当前残卷关闭与重复 Esc 必须合计恰好发送一条 scroll_read_closed"
        );
    }

    @Test
    void rejectedTerminalTransportStillClearsPendingScrollSession() {
        ClientRequestSender.setAttemptBackendForTests((channel, payload) -> false);
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_rejected", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen pendingScreen = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle handle = activatePending(pendingScreen);

        ScreenTransitionController.cancelAndClose(null);
        ScreenTransitionController.cancelAndClose(null);

        assertTrue(handle.cancelled(), "transport 拒绝也必须完成视觉转场取消");
        assertNull(ScreenTransitionController.activeTransition(), "transport 拒绝后不得残留 active transition");
        assertNull(ScrollReadStore.snapshot(),
            "视觉已关闭时，本地阅读 store 必须完成终态，不能因 transport 拒绝永久悬挂");
    }

    @Test
    void terminalTransportExceptionStillClearsPendingScrollSessionIdempotently() {
        ClientRequestSender.setAttemptBackendForTests((channel, payload) -> {
            throw new IllegalStateException("simulated disconnect");
        });
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_exception", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen pendingScreen = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle handle = activatePending(pendingScreen);

        ScreenTransitionController.cancelAndClose(null);
        ScreenTransitionController.cancelAndClose(null);

        assertTrue(handle.cancelled(), "transport 异常也必须完成视觉转场取消");
        assertNull(ScreenTransitionController.activeTransition(), "transport 异常后不得残留 active transition");
        assertNull(ScrollReadStore.snapshot(),
            "transport 异常已被 tryDispatch 收敛时，本地阅读 store 仍必须幂等完成终态");
    }

    @Test
    void pendingProtocolSettlementRunsAfterCurrentScreenDirectClose() {
        boolean[] currentScreenClosed = {false};
        OrderingAwareScreen pendingScreen = new OrderingAwareScreen(currentScreenClosed);

        ScreenTransitionController.closeCurrentThenSettlePending(
            null,
            pendingScreen,
            () -> currentScreenClosed[0] = true
        );

        assertTrue(pendingScreen.settled(), "pending screen 必须收到取消结算回调");
        assertTrue(pendingScreen.sawCurrentScreenClosed(),
            "必须先直接关闭当前 screen，再结算 pending 协议；否则 store listener 可重入创建残留转场");
    }

    @Test
    void supersededPendingScrollCanBeCancelledWithoutClosingCurrentScreen() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8))));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_superseded", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen pendingScreen = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle handle = activatePending(pendingScreen);

        boolean cancelled = ScreenTransitionController.cancelPendingOpen(pendingScreen);
        boolean repeated = ScreenTransitionController.cancelPendingOpen(pendingScreen);

        assertTrue(cancelled, "被后续 screen 替换的 pending 残卷必须能精确取消");
        assertFalse(repeated, "同一个 pending transition 重复取消必须 no-op");
        assertTrue(handle.cancelled(), "pending transition handle 必须进入取消态");
        assertNull(ScreenTransitionController.activeTransition(), "精确取消后不得残留 active transition");
        assertNull(ScrollReadStore.snapshot(), "被替换的 pending 残卷必须完成协议终态");
        assertEquals(List.of(new Sent(
            new Identifier("bong", "client_request"),
            "{\"type\":\"scroll_read_closed\",\"v\":1}"
        )), sent,
            "superseded pending 残卷必须且只能发送一条终态");
    }

    @Test
    void rapidReplacementSettlesPendingScrollUnlessSessionContinues() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8))));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_rapid", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen firstPending = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle firstHandle = activatePending(firstPending);
        ScrollReadScreen sameSessionReplacement = new ScrollReadScreen(offer);

        ScreenTransitionController.cancelActiveTransitionForReplacement(sameSessionReplacement);

        assertTrue(firstHandle.cancelled(), "rapid replacement 必须取消旧 handle");
        assertSame(offer, ScrollReadStore.snapshot(), "同 token replacement 必须延续当前阅读会话");
        assertTrue(sent.isEmpty(), "同 token replacement 不得误发关闭终态");

        ScreenTransition.TransitionHandle continuedHandle = activatePending(sameSessionReplacement);
        ScreenTransitionController.cancelActiveTransitionForReplacement(new DummyScreen());

        assertTrue(continuedHandle.cancelled(), "无关 screen 覆盖必须取消 pending handle");
        assertNull(ScrollReadStore.snapshot(), "无关 screen 覆盖必须结算 pending 阅读会话");
        assertEquals(1, sent.size(), "同会话延续后再被无关 screen 覆盖，只能发送一次终态");
    }

    @Test
    void disconnectClearCancelsPendingVisualStateWithoutSendingTerminalRequest() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8))));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_disconnect", "《残卷》", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen pendingScreen = new ScrollReadScreen(offer);
        ScreenTransition.TransitionHandle handle = activatePending(pendingScreen);
        ScrollReadStore.clearOnDisconnect();

        boolean cancelled = ScreenTransitionController.cancelPendingOpen(pendingScreen);

        assertTrue(cancelled, "断线清空后必须取消尚未完成的视觉转场");
        assertTrue(handle.cancelled(), "断线清理必须取消 pending handle");
        assertNull(ScreenTransitionController.activeTransition(), "断线清理后不得残留 active transition");
        assertNull(ScrollReadStore.snapshot(), "断线清理后 store 必须保持空态");
        assertTrue(sent.isEmpty(), "连接已断时不得补发 scroll_read_closed");
    }

    private static ScreenTransition.TransitionHandle activatePending(Screen pendingScreen) {
        return activateTransition(null, pendingScreen);
    }

    private static ScreenTransition.TransitionHandle activateTransition(Screen currentScreen, Screen pendingScreen) {
        ScreenTransition.TransitionHandle handle = ScreenTransition.play(
            currentScreen,
            pendingScreen,
            ScreenTransition.Type.FADE,
            200,
            ScreenTransition.Easing.LINEAR,
            () -> {
            }
        );
        ScreenTransitionController.setActiveTransitionForTests(new ScreenTransitionController.ActiveTransition(
            handle,
            new TransitionConfig.TransitionSpec(
                ScreenTransition.Type.FADE,
                200,
                ScreenTransition.Easing.LINEAR,
                TransitionConfig.OverlayStyle.NONE,
                false
            ),
            ScreenTransition.nowMillis()
        ));
        return handle;
    }

    private static final class DummyScreen extends Screen {
        private DummyScreen() {
            super(Text.literal("dummy"));
        }
    }

    private static final class OrderingAwareScreen extends Screen
        implements ScreenTransitionController.PendingOpenCancellationHandler {
        private final boolean[] currentScreenClosed;
        private boolean settled;
        private boolean sawCurrentScreenClosed;

        private OrderingAwareScreen(boolean[] currentScreenClosed) {
            super(Text.literal("ordering-aware"));
            this.currentScreenClosed = currentScreenClosed;
        }

        @Override
        public void onPendingOpenCancelled() {
            settled = true;
            sawCurrentScreenClosed = currentScreenClosed[0];
        }

        private boolean settled() {
            return settled;
        }

        private boolean sawCurrentScreenClosed() {
            return sawCurrentScreenClosed;
        }
    }
}
