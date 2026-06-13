package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;

class DeadDropBreakPlayerTest {

    @BeforeEach
    @AfterEach
    void clearGasBursts() {
        DeadDropBreakPlayer.clearGasBurstsForTests();
    }

    @Test
    void playWithUninitializedClientReturnsBeforeQueueingGas() {
        new DeadDropBreakPlayer().play(null, payload());

        assertEquals(0, DeadDropBreakPlayer.pendingGasBurstsForTests(),
            "headless/null-world play must return before adding a gas burst");
    }

    @Test
    void gasBurstQueueIsBoundedAndDropsOldestBursts() {
        int max = DeadDropBreakPlayer.maxPendingGasBurstsForTests();

        for (int i = 0; i < max + 5; i++) {
            DeadDropBreakPlayer.enqueueGasBurstForTests(i, 64.0, i);
        }

        assertEquals(max, DeadDropBreakPlayer.pendingGasBurstsForTests(),
            "event storms must not grow the pending gas-burst queue without bound");
    }

    @Test
    void gasBurstsExpireAfterConfiguredTicks() {
        DeadDropBreakPlayer.enqueueGasBurstForTests(1.0, 64.0, 1.0);

        for (int i = 1; i < DeadDropBreakPlayer.gasTicksForTests(); i++) {
            DeadDropBreakPlayer.advanceGasBurstsForTests();
            assertEquals(1, DeadDropBreakPlayer.pendingGasBurstsForTests(),
                "gas burst should remain pending until its final configured tick");
        }

        DeadDropBreakPlayer.advanceGasBurstsForTests();
        assertEquals(0, DeadDropBreakPlayer.pendingGasBurstsForTests(),
            "gas burst should be removed once its configured lifetime is exhausted");
    }

    private static VfxEventPayload.SpawnParticle payload() {
        return new VfxEventPayload.SpawnParticle(
            DeadDropBreakPlayer.EVENT_ID,
            new double[] { 1.5, 64.0, 1.5 },
            Optional.empty(),
            OptionalInt.of(0x3AA0C0),
            Optional.of(0.85),
            OptionalInt.of(12),
            OptionalInt.of(20)
        );
    }
}
