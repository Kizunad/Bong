package com.bong.client.forge;

import com.bong.client.tsy.ExtractStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ForgeScreenBootstrapTest {
    @AfterEach
    void resetExtractionState() {
        ExtractStateStore.resetForTests();
    }

    @Test
    void forgeDispatchIsAvailableWhenExtractionIsIdle() {
        assertTrue(
            ForgeScreenBootstrap.shouldDispatchForgeOpen(),
            "非撤离状态允许消费 Forge 按键并请求打开锻炉屏幕"
        );
    }

    @Test
    void legacyForgeUPressIsConsumedWithoutOpeningDuringExtraction() {
        ExtractStateStore.markStarted(42L, "tsy_lingxu_01", 20, 1_000L);

        assertFalse(
            ForgeScreenBootstrap.shouldDispatchForgeOpen(),
            "撤离进行时即使 Forge 暂时回滚到历史 U，也不能派发 Forge 开屏"
        );
    }
}
