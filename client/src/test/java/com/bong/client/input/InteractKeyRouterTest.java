package com.bong.client.input;

import net.minecraft.client.MinecraftClient;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InteractKeyRouterTest {
    @Test
    void noHandlerDoesNothing() {
        assertFalse(new InteractKeyRouter().route(null, false));
    }

    @Test
    void dispatchesOnlyWinner() {
        InteractKeyRouter router = new InteractKeyRouter();
        AtomicInteger low = new AtomicInteger();
        AtomicInteger high = new AtomicInteger();
        router.register(handler(InteractIntent.PickupDroppedItem, 70, 1.0, low, true));
        router.register(handler(InteractIntent.TradePlayer, 90, 8.0, high, true));

        assertTrue(router.route(null, false));
        assertEquals(0, low.get());
        assertEquals(1, high.get());
    }

    @Test
    void dispatchFalseDoesNotFallback() {
        InteractKeyRouter router = new InteractKeyRouter();
        AtomicInteger winner = new AtomicInteger();
        AtomicInteger fallback = new AtomicInteger();
        router.register(handler(InteractIntent.TradePlayer, 90, 1.0, winner, false));
        router.register(handler(InteractIntent.PickupDroppedItem, 70, 1.0, fallback, true));

        assertFalse(router.route(null, false));
        assertEquals(1, winner.get());
        assertEquals(0, fallback.get());
    }

    @Test
    void screenOpenSkipsRoute() {
        InteractKeyRouter router = new InteractKeyRouter();
        AtomicInteger calls = new AtomicInteger();
        router.register(handler(InteractIntent.TradePlayer, 90, 1.0, calls, true));

        assertFalse(router.route(null, true));
        assertEquals(0, calls.get());
    }

    private static IntentHandler handler(
        InteractIntent intent,
        int priority,
        double distanceSq,
        AtomicInteger dispatches,
        boolean dispatchResult
    ) {
        return new IntentHandler() {
            @Override
            public Optional<InteractCandidate> candidate(MinecraftClient client) {
                return Optional.of(InteractCandidate.of(intent, priority, distanceSq, intent.name()));
            }

            @Override
            public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
                dispatches.incrementAndGet();
                return dispatchResult;
            }
        };
    }
}
