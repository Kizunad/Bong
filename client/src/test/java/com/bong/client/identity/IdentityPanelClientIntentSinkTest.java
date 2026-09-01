package com.bong.client.identity;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

final class IdentityPanelClientIntentSinkTest {
    @Test
    void typedActionsKeepTheIdentityCommandShapes() {
        List<String> commands = new ArrayList<>();
        IdentityPanelClientIntentSink sink = new IdentityPanelClientIntentSink(commands::add);

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED,
            sink.dispatch(new IdentityPanelIntent.NewIdentity("夜行 人")).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED,
            sink.dispatch(new IdentityPanelIntent.RenameIdentity("白面")).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED,
            sink.dispatch(new IdentityPanelIntent.SwitchIdentity(-4)).kind());

        assertEquals(List.of("identity new 夜行 人", "identity rename 白面", "identity switch 0"), commands,
            "typed intent 必须保持既有 /identity 命令语义且不带前导斜杠");
    }

    @Test
    void nullIntentIsRejectedWithoutTouchingTransport() {
        int[] calls = {0};
        IdentityPanelClientIntentSink sink = new IdentityPanelClientIntentSink(command -> calls[0]++);

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 identity intent 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        IdentityPanelClientIntentSink sink = new IdentityPanelClientIntentSink(command -> {
            throw new IllegalStateException("not connected");
        });

        UiIntentResult result = sink.dispatch(new IdentityPanelIntent.SwitchIdentity(3));

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常必须保留可修复原因");
    }
}
