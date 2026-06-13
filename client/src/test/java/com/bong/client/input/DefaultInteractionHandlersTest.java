package com.bong.client.input;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class DefaultInteractionHandlersTest {
    @AfterEach
    void resetGlobal() {
        InteractKeyRouter.resetGlobalForTests();
    }

    @Test
    void registerDefaultsIncludesWorldContainerHandlers() {
        InteractKeyRouter.resetGlobalForTests();

        DefaultInteractionHandlers.registerDefaults();

        assertEquals(
            9,
            InteractKeyRouter.global().handlerCountForTests(),
            "default interact handlers must include storage-crate and dead-drop open handlers"
        );
    }
}
