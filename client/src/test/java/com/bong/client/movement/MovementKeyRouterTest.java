package com.bong.client.movement;

import com.bong.client.network.ClientRequestProtocol;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class MovementKeyRouterTest {
    @Test
    void doubleTapForwardRoutesDashWithinWindow() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertNull(router.route(true, true, false, false, false, false, 1_000L));
        assertEquals(
            ClientRequestProtocol.MovementAction.DASH,
            router.route(true, true, false, false, false, false, 1_180L)
        );
    }

    @Test
    void expiredForwardTapDoesNotDash() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertNull(router.route(true, true, false, false, false, false, 1_000L));
        assertNull(router.route(true, true, false, false, false, false, 1_400L));
    }

    @Test
    void shiftForwardRoutesDash() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertEquals(
            ClientRequestProtocol.MovementAction.DASH,
            router.route(true, false, true, false, false, false, 1_000L)
        );
    }

    @Test
    void sprintWhileForwardRoutesSlide() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertEquals(
            ClientRequestProtocol.MovementAction.SLIDE,
            router.route(true, false, false, true, false, false, 1_000L)
        );
    }

    @Test
    void airborneJumpRoutesDoubleJump() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertEquals(
            ClientRequestProtocol.MovementAction.DOUBLE_JUMP,
            router.route(false, false, false, false, true, true, 1_000L)
        );
    }

    @Test
    void groundedJumpDoesNotRouteDoubleJump() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertNull(router.route(false, false, false, false, true, false, 1_000L));
    }
}
