package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

class InspectScreenQuickUseTabTest {
    @Test
    void inspectScreenRemovesQuickUseTabAndKeepsCraftEntry() {
        List<String> tabs = InspectScreen.tabNamesForTests();

        assertEquals(List.of("装备", "修仙", "技艺", "功法", "手搓"), tabs);
        assertFalse(tabs.contains("快捷使用"));
    }
}
