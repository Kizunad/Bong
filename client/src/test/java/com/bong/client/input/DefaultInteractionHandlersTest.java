package com.bong.client.input;

import com.bong.client.inventory.DeadDropInteractIntentHandler;
import com.bong.client.inventory.StorageCrateInteractIntentHandler;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;

class DefaultInteractionHandlersTest {
    @AfterEach
    void resetGlobal() {
        InteractKeyRouter.resetGlobalForTests();
    }

    @Test
    void registerDefaultsIncludesWorldContainerHandlers() {
        InteractKeyRouter.resetGlobalForTests();

        DefaultInteractionHandlers.registerDefaults();

        assertTrue(
            InteractKeyRouter.global().hasHandlerForTests(StorageCrateInteractIntentHandler.class),
            "expected StorageCrateInteractIntentHandler because default route must expose storage crates"
        );
        assertTrue(
            InteractKeyRouter.global().hasHandlerForTests(DeadDropInteractIntentHandler.class),
            "expected DeadDropInteractIntentHandler because default route must expose dead drops"
        );
    }
}
