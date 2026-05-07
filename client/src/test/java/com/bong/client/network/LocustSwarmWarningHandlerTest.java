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
        assertEquals(6_500L, toast.durationMillis());

        VisualEffectState effect = dispatch.visualEffectState().orElseThrow();
        assertEquals(VisualEffectState.EffectType.PRESSURE_JITTER, effect.effectType());
        assertEquals(0.65, effect.intensity(), 0.0001);
        assertEquals(42L, effect.startedAtMillis());
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
}
