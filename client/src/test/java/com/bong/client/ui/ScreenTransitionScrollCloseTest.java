package com.bong.client.ui;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.scroll.ScrollOpenViewModel;
import com.bong.client.scroll.ScrollReadScreen;
import com.bong.client.scroll.ScrollReadStore;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
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
        ScreenTransition.TransitionHandle handle = ScreenTransition.play(
            null,
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
            "转场取消必须恰好发送一条 scroll_read_closed 终态，不能静默丢失或重复发送"
        );
    }
}
