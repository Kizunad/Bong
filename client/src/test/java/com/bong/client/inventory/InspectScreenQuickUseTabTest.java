package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class InspectScreenQuickUseTabTest {
    @Test
    void inspectScreenExposesDedicatedQuickUseTab() {
        // plan-craft-v1 §2 — 「手搓」是 plan-craft-v1 P2 新加的第 6 个 tab；
        // QuickUse 仍以独立 tab 出现，本测试同时锁定两者顺序与命名。
        assertEquals(
            List.of("装备", "修仙", "技艺", "功法", "快捷使用", "手搓"),
            InspectScreen.tabNamesForTests()
        );
    }
}
