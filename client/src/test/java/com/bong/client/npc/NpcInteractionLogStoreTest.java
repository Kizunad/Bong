package com.bong.client.npc;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcInteractionLogStoreTest {
    @AfterEach
    void reset() {
        NpcInteractionLogStore.resetForTests();
    }

    @Test
    void clearOnDisconnectRemovesStaleEntriesAcrossSessions() {
        NpcInteractionLogStore.record(new NpcInteractionLogEntry(1, "NPC 1", "rogue", "greeting", 100L));
        NpcInteractionLogStore.record(new NpcInteractionLogEntry(2, "NPC 2", "sage", "trade", 200L));
        NpcInteractionLogStore.toggleVisible();
        assertEquals(2, NpcInteractionLogStore.snapshot().size(),
            "先决条件：两条交互日志应已入库");
        assertTrue(NpcInteractionLogStore.visible(), "先决条件：面板应处于打开状态");

        NpcInteractionLogStore.clearOnDisconnect();

        assertTrue(NpcInteractionLogStore.snapshot().isEmpty(),
            "断线后旧会话日志条目不应残留到重连后的新会话");
        assertFalse(NpcInteractionLogStore.visible(),
            "断线后面板可见状态应重置，避免重连后自动弹出上一局残留状态");
    }

    @Test
    void clearOnDisconnectIsSafeWhenAlreadyEmpty() {
        NpcInteractionLogStore.clearOnDisconnect();

        assertTrue(NpcInteractionLogStore.snapshot().isEmpty());
        assertFalse(NpcInteractionLogStore.visible());
    }
}
