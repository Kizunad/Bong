package com.bong.client;

import com.bong.client.network.handlers.EventAlertHandler;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

public class EventAlertHandlerTest {
    private final EventAlertHandler handler = new EventAlertHandler();

    @AfterEach
    void tearDown() {
        EventAlertState.clear();
    }

    @Test
    void validEventAlertPayloadShowsStructuredAlert() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"event_alert\"," +
            "\"event\":\"thunder_tribulation\"," +
            "\"message\":\"天劫已在区域 spawn 触发\"" +
            "}";

        handler.handle(null, "event_alert", json);

        EventAlertState.ActiveAlert alert = EventAlertState.peek(System.currentTimeMillis());
        assertNotNull(alert);
        assertEquals("天劫已在区域 spawn 触发", alert.message());
    }

    @Test
    void malformedEventAlertPayloadIsIgnoredSafely() {
        String validJson = "{" +
            "\"v\":1," +
            "\"type\":\"event_alert\"," +
            "\"event\":\"beast_tide\"," +
            "\"message\":\"兽潮来袭\"" +
            "}";
        handler.handle(null, "event_alert", validJson);

        String malformedJson = "{" +
            "\"v\":1," +
            "\"type\":\"event_alert\"," +
            "\"event\":\"realm_collapse\"," +
            "\"message\":\"非法事件\"" +
            "}";
        handler.handle(null, "event_alert", malformedJson);

        EventAlertState.ActiveAlert alert = EventAlertState.peek(System.currentTimeMillis());
        assertNotNull(alert);
        assertEquals("兽潮来袭", alert.message());
    }

    @Test
    void unsupportedGameplayHintDoesNotHijackMajorEventAlertChannel() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"event_alert\"," +
            "\"event\":\"karma_backlash\"," +
            "\"message\":\"这类 gameplay 提示应继续走 narration 而非 major alert\"" +
            "}";

        handler.handle(null, "event_alert", json);

        assertNull(
            EventAlertState.peek(System.currentTimeMillis()),
            "non-major gameplay hints should not populate the existing event alert HUD state"
        );
    }
}
