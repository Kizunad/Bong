package com.bong.client.network;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class KnockbackSyncHandlerTest {
    @Test
    void knockbackSyncProducesHitPushbackVisualEffect() {
        String json = """
            {"v":1,"type":"knockback_sync","distance_blocks":6.0,"velocity_blocks_per_tick":0.75,
             "duration_ticks":6,"kinetic_energy":42.0,"collision_damage":3.5,
             "chain_depth":2,"block_broken":true}""";
        ServerDataDispatch dispatch = new KnockbackSyncHandler(() -> 1_000L).handle(parse(json));

        assertTrue(dispatch.handled());
        VisualEffectState state = dispatch.visualEffectState().orElseThrow();
        assertEquals(VisualEffectState.EffectType.HIT_PUSHBACK, state.effectType());
        assertTrue(state.intensity() > 0.0);
        assertEquals(1_000L, state.startedAtMillis());
    }

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult result = ServerDataEnvelope.parse(
            json,
            json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length
        );
        if (!result.isSuccess()) {
            throw new AssertionError(result.errorMessage());
        }
        return result.envelope();
    }
}
