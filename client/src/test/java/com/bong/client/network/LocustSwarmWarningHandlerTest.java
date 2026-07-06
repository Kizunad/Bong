package com.bong.client.network;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LocustSwarmWarningHandlerTest {
    @Test
    void locustSwarmWarningPayloadRoutesIntoHudWarningToast() {
        ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 42L).handle("""
            {"v":1,"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近 · 朝灵泉泽推进","duration_ticks":24000}
            """);

        assertTrue(dispatch.handled());
        ServerDataDispatch.ToastSpec toast = dispatch.alertToast().orElseThrow();
        assertEquals("灵蝗潮逼近：灵蝗潮逼近 · 朝灵泉泽推进", toast.text());
        assertEquals(LocustSwarmWarningHandler.WARNING_COLOR, toast.color());
        assertEquals(1_200_000L, toast.durationMillis());

        VisualEffectState effect = dispatch.visualEffectState().orElseThrow();
        assertEquals(VisualEffectState.EffectType.PRESSURE_JITTER, effect.effectType());
        assertEquals(0.65, effect.intensity(), 0.0001);
        assertEquals(1_200_000L, effect.durationMillis());
        assertEquals(42L, effect.startedAtMillis());
    }

    @Test
    void missingDurationTicksFallsBackToDefaultWarningDuration() {
        ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 7L).handle("""
            {"v":1,"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近"}
            """);

        assertTrue(dispatch.handled());
        assertEquals(6_500L, dispatch.alertToast().orElseThrow().durationMillis());
        assertEquals(6_500L, dispatch.visualEffectState().orElseThrow().durationMillis());
    }

    @Test
    void invalidDurationTicksFallsBackToDefaultWarningDuration() {
        String[] invalidDurationTicks = {"-1", "12.5", "\"24000\"", "true"};

        for (String durationTicks : invalidDurationTicks) {
            ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 7L).handle("""
                {"v":1,"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近","duration_ticks":%s}
                """.formatted(durationTicks));

            assertTrue(dispatch.handled(), "invalid duration_ticks should not reject the whole warning: " + durationTicks);
            assertEquals(6_500L, dispatch.alertToast().orElseThrow().durationMillis(), "toast fallback duration for " + durationTicks);
            assertEquals(6_500L, dispatch.visualEffectState().orElseThrow().durationMillis(), "effect fallback duration for " + durationTicks);
        }
    }

    @Test
    void numericIntegerRepresentationsUseProtocolDuration() {
        String[] validDurationTicks = {"1.0", "1e3", "1.5e1"};
        long[] expectedMillis = {50L, 50_000L, 750L};

        for (int i = 0; i < validDurationTicks.length; i += 1) {
            String durationTicks = validDurationTicks[i];
            ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 7L).handle("""
                {"v":1,"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近","duration_ticks":%s}
                """.formatted(durationTicks));

            assertTrue(dispatch.handled(), "schema-valid integer number should be handled: " + durationTicks);
            assertEquals(expectedMillis[i], dispatch.alertToast().orElseThrow().durationMillis(), "toast duration for " + durationTicks);
            assertEquals(expectedMillis[i], dispatch.visualEffectState().orElseThrow().durationMillis(), "effect duration for " + durationTicks);
        }
    }

    @Test
    void oneTickDurationUsesProtocolDurationInsteadOfDefaultWindow() {
        ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 7L).handle("""
            {"v":1,"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近","duration_ticks":1}
            """);

        assertTrue(dispatch.handled());
        assertEquals(50L, dispatch.alertToast().orElseThrow().durationMillis());
        assertEquals(50L, dispatch.visualEffectState().orElseThrow().durationMillis());
    }

    @Test
    void oversizedDurationTicksSaturatesInsteadOfWrappingMillis() {
        ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 7L).handle("""
            {"v":1,"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近","duration_ticks":18446744073709551615}
            """);

        assertTrue(dispatch.handled());
        assertEquals(Long.MAX_VALUE, dispatch.alertToast().orElseThrow().durationMillis());
        assertEquals(Long.MAX_VALUE, dispatch.visualEffectState().orElseThrow().durationMillis());
    }

    @Test
    void malformedLocustSwarmWarningPayloadIsNoOp() {
        ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 0L).handle("""
            {"v":1,"type":"locust_swarm_warning","message":"缺少区域"}
            """);

        assertFalse(dispatch.handled());
        assertTrue(dispatch.alertToast().isEmpty());
        assertTrue(dispatch.visualEffectState().isEmpty());
    }

    @Test
    void missingVersionFieldIsNoOp() {
        ServerDataDispatch dispatch = new LocustSwarmWarningHandler(() -> 0L).handle("""
            {"type":"locust_swarm_warning","zone":"spirit_marsh","message":"灵蝗潮逼近"}
            """);

        assertFalse(dispatch.handled());
        assertTrue(dispatch.alertToast().isEmpty());
        assertTrue(dispatch.visualEffectState().isEmpty());
    }
}
