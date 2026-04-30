package com.bong.client.visual.realm_vision;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ClientRenderDistanceAdvisorTest {
    @Test
    void toastUnderThreshold() {
        assertTrue(ClientRenderDistanceAdvisor.shouldWarn(8));
    }

    @Test
    void noToastAboveThreshold() {
        assertFalse(ClientRenderDistanceAdvisor.shouldWarn(16));
    }
}
