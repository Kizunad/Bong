package com.bong.client.network;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class EventAlertHandlerTest {
    @Test
    void parsesInfoAlertWithCustomDurationAndNoEffect() {
        ServerDataDispatch dispatch = new EventAlertHandler(() -> 77L).handle(parseEnvelope(
            "{\"v\":1,\"type\":\"event_alert\",\"title\":\"灵潮回涌\",\"message\":\"谷中灵气逐渐平稳\",\"severity\":\"info\",\"duration_ms\":2500}"
        ));

        assertTrue(dispatch.handled());
        ServerDataDispatch.ToastSpec toastSpec = dispatch.alertToast().orElseThrow();
        assertEquals("灵潮回涌：谷中灵气逐渐平稳", toastSpec.text());
        assertEquals(EventAlertHandler.INFO_COLOR, toastSpec.color());
        assertEquals(2_500L, toastSpec.durationMillis());
        assertTrue(dispatch.visualEffectState().isEmpty());
    }

    @Test
    void parsesUppercaseSeverityWithLocaleInvariantNormalization() {
        Locale previousLocale = Locale.getDefault();
        Locale.setDefault(Locale.forLanguageTag("tr"));
        try {
            ServerDataDispatch dispatch = new EventAlertHandler(() -> 77L).handle(parseEnvelope(
                "{\"v\":1,\"type\":\"event_alert\",\"title\":\"天道示警\",\"message\":\"试炼将启\",\"severity\":\"INFO\"}"
            ));

            assertTrue(dispatch.handled());
            ServerDataDispatch.ToastSpec toastSpec = dispatch.alertToast().orElseThrow();
            assertEquals(EventAlertHandler.INFO_COLOR, toastSpec.color());
            assertEquals(3_500L, toastSpec.durationMillis());
        } finally {
            Locale.setDefault(previousLocale);
        }
    }

    @Test
    void derivesTitleFromEventWhenServerPayloadOmitsTitle() {
        ServerDataDispatch dispatch = new EventAlertHandler(() -> 77L).handle(parseEnvelope(
            "{\"v\":1,\"type\":\"event_alert\",\"event\":\"thunder_tribulation\",\"message\":\"天劫将至，请于三十息内离开血谷中央。\",\"duration_ticks\":600}"
        ));

        assertTrue(dispatch.handled());
        ServerDataDispatch.ToastSpec toastSpec = dispatch.alertToast().orElseThrow();
        assertEquals("Thunder Tribulation：天劫将至，请于三十息内离开血谷中央。", toastSpec.text());
        assertEquals(EventAlertHandler.WARNING_COLOR, toastSpec.color());
        assertEquals(5_000L, toastSpec.durationMillis());
        assertTrue(dispatch.visualEffectState().isEmpty());
    }

    @Test
    void realmCollapseAlertStartsPersistentHudCountdown() {
        ServerDataDispatch dispatch = new EventAlertHandler(() -> 12_000L).handle(parseEnvelope(
            "{\"v\":1,\"type\":\"event_alert\",\"event\":\"realm_collapse\",\"message\":\"域崩撤离窗口已开启\",\"zone\":\"blood_valley\",\"duration_ticks\":12000}"
        ));

        assertTrue(dispatch.handled());
        var collapse = dispatch.realmCollapseHudState().orElseThrow();
        assertEquals("blood_valley", collapse.zone());
        assertEquals("域崩撤离窗口已开启", collapse.message());
        assertEquals(12_000L, collapse.startedAtMillis());
        assertEquals(12_000, collapse.durationTicks());
        assertEquals(11_980, collapse.remainingTicks(13_000L));
    }

    @Test
    void parsesCriticalAlertAndMapsEffectHint() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-event-alert-critical.json");
        ServerDataDispatch dispatch = new EventAlertHandler(() -> 9_999L).handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        ServerDataDispatch.ToastSpec toastSpec = dispatch.alertToast().orElseThrow();
        assertEquals("天劫将至：血谷上空雷云翻涌", toastSpec.text());
        assertEquals(EventAlertHandler.CRITICAL_COLOR, toastSpec.color());
        assertEquals(6_500L, toastSpec.durationMillis());

        VisualEffectState visualEffectState = dispatch.visualEffectState().orElseThrow();
        assertEquals(VisualEffectState.EffectType.SCREEN_SHAKE, visualEffectState.effectType());
        assertEquals(0.9, visualEffectState.intensity(), 0.0001);
        assertEquals(6_500L, visualEffectState.durationMillis());
        assertEquals(9_999L, visualEffectState.startedAtMillis());
    }

    @Test
    void unknownSeverityFallsBackToWarningAndIgnoresBadEffectFields() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-event-alert-unknown-severity.json");
        ServerDataDispatch dispatch = new EventAlertHandler(() -> 321L).handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        ServerDataDispatch.ToastSpec toastSpec = dispatch.alertToast().orElseThrow();
        assertEquals(EventAlertHandler.WARNING_COLOR, toastSpec.color());
        assertEquals(5_000L, toastSpec.durationMillis());
        assertTrue(dispatch.visualEffectState().isEmpty());
    }

    @Test
    void missingRequiredFieldsBecomeSafeNoOp() {
        ServerDataDispatch dispatch = new EventAlertHandler(() -> 0L).handle(parseEnvelope(
            "{\"v\":1,\"type\":\"event_alert\",\"title\":\"天道警示\"}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.alertToast().isEmpty());
        assertTrue(dispatch.visualEffectState().isEmpty());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), () -> "Expected payload to parse successfully but got: " + parseResult.errorMessage());
        return parseResult.envelope();
    }
}
