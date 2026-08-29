package com.bong.client.input;

import com.bong.client.tsy.ExtractStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ClientInputPolicyTest {
    @AfterEach
    void resetExtractionState() {
        ExtractStateStore.resetForTests();
    }

    @Test
    void forgeOpenIsAllowedWhenExtractionIsIdle() {
        assertTrue(
            ClientInputPolicy.shouldDispatchForgeOpen(),
            "撤离未进行时共享输入策略应允许 Forge 消费 legacy U"
        );
    }

    @Test
    void forgeOpenIsBlockedWhileExtractionIsActive() {
        ExtractStateStore.markStarted(42L, "tsy_lingxu_01", 20, 1_000L);

        assertFalse(
            ClientInputPolicy.shouldDispatchForgeOpen(),
            "撤离进行时共享输入策略必须把 legacy U 留给撤离取消"
        );
    }
}
