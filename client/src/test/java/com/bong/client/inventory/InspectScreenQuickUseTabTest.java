package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class InspectScreenQuickUseTabTest {
    @Test
    void inspectScreenExposesDedicatedQuickUseTab() {
        assertEquals(
            List.of("装备", "修仙", "技艺", "功法", "快捷使用"),
            InspectScreen.tabNamesForTests()
        );
    }
}
