package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;

import java.util.Optional;
import java.util.OptionalInt;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BaomaiV3VfxPlayerTest {
    @Test
    void hudSideEffectsOnlyAcceptLocalPlayerOrigin() {
        assertTrue(BaomaiV3VfxPlayer.isLocalPlayerOrigin(
            new double[] { 10.0, 64.0, 10.0 },
            new double[] { 10.5, 64.0, 10.5 }
        ));

        assertFalse(BaomaiV3VfxPlayer.isLocalPlayerOrigin(
            new double[] { 10.0, 64.0, 10.0 },
            new double[] { 16.0, 64.0, 10.0 }
        ));
    }

    @Test
    void bodyTranscendenceFlowMultiplierUsesPayloadStrength() {
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            BaomaiV3VfxPlayer.BODY_TRANSCENDENCE_PILLAR,
            new double[] { 0.0, 0.0, 0.0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.of(0.6),
            OptionalInt.empty(),
            OptionalInt.empty()
        );

        assertEquals(6.0, BaomaiV3VfxPlayer.bodyTranscendenceFlowMultiplier(payload));
    }

    @Test
    void bodyTranscendenceFlowMultiplierDefaultsToVoidProfile() {
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            BaomaiV3VfxPlayer.BODY_TRANSCENDENCE_PILLAR,
            new double[] { 0.0, 0.0, 0.0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        );

        assertEquals(10.0, BaomaiV3VfxPlayer.bodyTranscendenceFlowMultiplier(payload));
    }
}
