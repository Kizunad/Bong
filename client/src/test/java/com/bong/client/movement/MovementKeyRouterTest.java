package com.bong.client.movement;

import com.bong.client.network.ClientRequestProtocol;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class MovementKeyRouterTest {
    @Test
    void dashKeyRoutesDash() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertEquals(
            ClientRequestProtocol.MovementAction.DASH,
            router.route(true, false, false, false)
        );
    }

    @Test
    void noMovementKeyDoesNotRouteAction() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertNull(router.route(false, false, false, false));
    }

    @Test
    void slideKeyRoutesSlide() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertEquals(
            ClientRequestProtocol.MovementAction.SLIDE,
            router.route(false, true, false, false)
        );
    }

    @Test
    void airborneJumpRoutesDoubleJump() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertEquals(
            ClientRequestProtocol.MovementAction.DOUBLE_JUMP,
            router.route(false, false, true, true)
        );
    }

    @Test
    void groundedJumpDoesNotRouteDoubleJump() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertNull(router.route(false, false, true, false));
    }
}
