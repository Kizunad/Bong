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
            router.route(true)
        );
    }

    @Test
    void noMovementKeyDoesNotRouteAction() {
        MovementKeyRouter router = new MovementKeyRouter();

        assertNull(router.route(false));
    }
}
