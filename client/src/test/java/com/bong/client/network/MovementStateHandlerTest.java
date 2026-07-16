package com.bong.client.network;

import bong.Envelope;
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
        assertEquals(2_000L, state.hudActivityAtMs());
    }

    @Test
    void prefixedProtoEnumsReachStoreThroughProductionBridge() {
        Envelope.ServerDataEnvelope protoEnvelope = Envelope.ServerDataEnvelope.newBuilder()
            .setMovementState(Envelope.MovementStateProto.newBuilder()
                .setCurrentSpeedMultiplier(0.75F)
                .setStaminaCostActive(true)
                .setMovementAction(Envelope.MovementAction.MOVEMENT_ACTION_DASHING)
                .setZoneKind(Envelope.MovementZoneKind.MOVEMENT_ZONE_KIND_NORMAL)
                .setDashCooldownRemainingTicks(35L)
                .setHitboxHeightBlocks(1.8F)
                .setStaminaCurrent(4.0F)
                .setStaminaMax(100.0F)
                .setLowStamina(true)
                .setLastActionTick(120L)
                .setRejectedAction(Envelope.MovementActionRequestKind.MOVEMENT_ACTION_REQUEST_KIND_DASH))
            .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(protoEnvelope.toByteArray());
        assertTrue(
            bridged.isSuccess(),
            "expected prefixed movement proto enums to bridge through the production decoder, actual error: "
                + bridged.errorMessage()
        );
        ServerDataDispatch dispatch = new MovementStateHandler().handle(parse(bridged.legacyJson()), 3_000L);

        assertTrue(
            dispatch.handled(),
            "expected bridged movement_state to reach MovementStateHandler, actual dispatch: " + dispatch
        );
        MovementState state = MovementStateStore.snapshot();
        assertEquals(
            MovementState.Action.DASHING,
            state.action(),
            "MOVEMENT_ACTION_DASHING must survive protobuf → bridge → handler → store"
        );
        assertEquals(
            MovementState.ZoneKind.NORMAL,
            state.zoneKind(),
            "MOVEMENT_ZONE_KIND_NORMAL must survive protobuf → bridge → handler → store"
        );
        assertEquals(
            "dash",
            state.rejectedAction(),
            "MOVEMENT_ACTION_REQUEST_KIND_DASH must normalize to the client wire value"
        );
        assertEquals(3_000L, state.rejectedAtMs(), "the bridged dash reject must start its flash at receipt time");
        assertEquals(3_000L, state.hudActivityAtMs(), "the bridged dash reject must activate the movement HUD");
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
              "hitbox_height_blocks": 1.8,
              "stamina_current": 4,
              "stamina_max": 100,
              "low_stamina": true,
              "rejected_action": "dash"
            }
            """);

        ServerDataDispatch dispatch = new MovementStateHandler().handle(envelope, 3_000L);

        assertTrue(dispatch.handled());
        assertEquals("dash", MovementStateStore.snapshot().rejectedAction());
        assertEquals(3_000L, MovementStateStore.snapshot().rejectedAtMs());
    }

    @Test
    void sameDashRejectRefreshesTimingAfterClearFollowup() {
        ServerDataEnvelope rejected = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 4,
              "stamina_max": 100,
              "low_stamina": true,
              "rejected_action": "dash"
            }
            """);
        ServerDataEnvelope cleared = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 5,
              "stamina_max": 100,
              "low_stamina": true
            }
            """);
        MovementStateHandler handler = new MovementStateHandler();

        assertTrue(handler.handle(rejected, 3_000L).handled(), "first dash reject should be handled");
        assertTrue(handler.handle(cleared, 3_200L).handled(), "clear followup should be handled");
        assertEquals("", MovementStateStore.snapshot().rejectedAction());
        assertEquals(
            3_000L,
            MovementStateStore.snapshot().rejectedAtMs(),
            "clear followup should preserve the historical reject timestamp without refreshing it"
        );

        assertTrue(handler.handle(rejected, 3_700L).handled(), "later same dash reject should be handled");
        MovementState refreshed = MovementStateStore.snapshot();
        assertEquals("dash", refreshed.rejectedAction());
        assertEquals(
            3_700L,
            refreshed.rejectedAtMs(),
            "a later identical dash reject is a new event and must refresh rejectedAtMs"
        );
        assertEquals(
            3_700L,
            refreshed.hudActivityAtMs(),
            "a later identical dash reject must also reactivate the movement HUD"
        );
    }

    @Test
    void staminaOnlyFollowupDoesNotRenewPriorRejectionTiming() {
        ServerDataEnvelope rejected = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 4,
              "stamina_max": 100,
              "low_stamina": true,
              "rejected_action": "dash"
            }
            """);
        ServerDataEnvelope staminaOnlyFollowup = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 5,
              "stamina_max": 100,
              "low_stamina": true
            }
            """);

        assertTrue(
            new MovementStateHandler().handle(rejected, 3_000L).handled(),
            "expected complete rejected payload to be handled, actual: false"
        );
        assertTrue(
            new MovementStateHandler().handle(staminaOnlyFollowup, 3_200L).handled(),
            "expected complete stamina-only followup to be handled, actual: false"
        );

        MovementState state = MovementStateStore.snapshot();
        assertEquals("", state.rejectedAction());
        assertEquals(
            3_000L,
            state.rejectedAtMs(),
            "stamina-only followup must not make the prior dash rejection look new"
        );
        assertEquals(
            3_000L,
            state.hudActivityAtMs(),
            "stamina recovery packets must not pin the movement HUD"
        );
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

    @Test
    void rejectsInvalidRejectedAction() {
        ServerDataEnvelope envelope = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 4,
              "stamina_max": 100,
              "low_stamina": true,
              "rejected_action": "stamina_insufficient"
            }
            """);

        ServerDataDispatch result = new MovementStateHandler().handle(envelope, 2_000L);

        assertFalse(result.handled());
        assertTrue(MovementStateStore.snapshot().isEmpty());
    }

    @Test
    void rejectsNonStringRejectedAction() {
        ServerDataEnvelope envelope = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": false,
              "movement_action": "none",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 0,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 4,
              "stamina_max": 100,
              "low_stamina": true,
              "rejected_action": 1
            }
            """);

        ServerDataDispatch result = new MovementStateHandler().handle(envelope, 2_000L);

        assertFalse(result.handled());
        assertTrue(MovementStateStore.snapshot().isEmpty());
    }

    @Test
    void outOfRangeIntegerIsNoOp() {
        ServerDataEnvelope envelope = parse("""
            {
              "v": 1,
              "type": "movement_state",
              "current_speed_multiplier": 0.75,
              "stamina_cost_active": true,
              "movement_action": "dashing",
              "zone_kind": "normal",
              "dash_cooldown_remaining_ticks": 9223372036854775808,
              "hitbox_height_blocks": 1.8,
              "stamina_current": 70,
              "stamina_max": 100,
              "low_stamina": false
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
