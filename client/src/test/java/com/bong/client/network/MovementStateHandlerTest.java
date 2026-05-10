package com.bong.client.network;

import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementStateHandlerTest {
    @AfterEach
    void tearDown() {
        MovementStateStore.resetForTests();
    }

    @Test
    void acceptsMovementStatePayloadIntoStore() {
        ServerDataEnvelope envelope = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": true,
              "movement_action": "dashing",
              "zone_kind": "negative",
              "dash_cooldown_remaining_ticks": 35,
              "slide_cooldown_remaining_ticks": 0,
              "double_jump_charges_remaining": 1,
              "double_jump_charges_max": 2,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 70,
              "stamina_max": 100,
              "low_stamina": false,
              "last_action_tick": 120
            }
            """);

        ServerDataDispatch dispatch = new MovementStateHandler().handle(envelope, 2_000L);

        assertTrue(dispatch.handled());
        MovementState state = MovementStateStore.snapshot();
        assertEquals(MovementState.Action.DASHING, state.action());
        assertEquals(MovementState.ZoneKind.NEGATIVE, state.zoneKind());
        assertEquals(35L, state.dashCooldownRemainingTicks());
        assertEquals(2, state.doubleJumpChargesMax());
        assertEquals(2_000L, state.hudActivityAtMs());
    }

    @Test
    void rejectedActionRecordsFlashTime() {
        ServerDataEnvelope envelope = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "slide_cooldown_remaining_ticks": 0,
              "double_jump_charges_remaining": 0,
              "double_jump_charges_max": 1,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 4,
              "stamina_max": 100,
              "low_stamina": true,
              "rejected_action": "stamina_insufficient"
            }
            """);

        ServerDataDispatch dispatch = new MovementStateHandler().handle(envelope, 3_000L);

        assertTrue(dispatch.handled());
        assertEquals("stamina_insufficient", MovementStateStore.snapshot().rejectedAction());
        assertEquals(3_000L, MovementStateStore.snapshot().rejectedAtMs());
    }

    @Test
    void invalidPayloadIsNoOpAndLeavesStoreUntouched() {
        ServerDataEnvelope envelope = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": true,
              "movement_action": "teleporting",
              "zone_kind": "normal"
            }
            """);

        ServerDataDispatch result = new MovementStateHandler().handle(envelope, 2_000L);

        assertFalse(result.handled());
        assertTrue(MovementStateStore.snapshot().isEmpty());
    }

    private static ServerDataEnvelope parse(String json) {
        byte[] bytes = json.getBytes(StandardCharsets.UTF_8);
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, bytes.length);
        assertTrue(result.isSuccess(), result.errorMessage());
        return result.envelope();
    }
}
