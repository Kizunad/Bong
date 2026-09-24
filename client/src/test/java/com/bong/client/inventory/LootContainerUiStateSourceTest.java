package com.bong.client.inventory;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;

class LootContainerUiStateSourceTest {
    @AfterEach
    void resetStores() {
        LootContainerStateStore.resetForTests();
        InventoryStateStore.resetForTests();
    }

    @Test
    void closedEventRemainsClosedAfterStoreClearsCurrentSession() {
        LootContainerStateStore.OpenSession expected = session(17L);
        LootContainerStateStore.open(expected);
        LootContainerUiStateSource source = LootContainerUiStateSource.production(expected);
        List<LootContainerScreenViewModel> updates = new ArrayList<>();

        source.subscribe(updates::add);
        LootContainerStateStore.close(expected.sessionId(), "expired");

        LootContainerScreenViewModel closed = updates.get(updates.size() - 1);
        assertInstanceOf(LootContainerSession.Closed.class, closed.session(),
            "close 事件必须穿过 source，不能因 Store current=null 回退成旧 open session");
        assertEquals("expired", ((LootContainerSession.Closed) closed.session()).reason());
        assertInstanceOf(LootContainerSession.Closed.class, source.snapshot().session(),
            "关闭后再次读取 source 仍须保持 Closed，避免 late inventory update 复活界面");
    }

    @Test
    void inventoryUpdateKeepsTheLatestClosedSessionInsteadOfReopeningIt() {
        LootContainerStateStore.OpenSession expected = session(18L);
        LootContainerStateStore.open(expected);
        LootContainerUiStateSource source = LootContainerUiStateSource.production(expected);
        List<LootContainerScreenViewModel> updates = new ArrayList<>();
        source.subscribe(updates::add);

        LootContainerStateStore.close(expected.sessionId(), "closed");
        InventoryStateStore.replace(InventoryModel.empty());

        assertInstanceOf(LootContainerSession.Closed.class, updates.get(updates.size() - 1).session(),
            "关闭后的库存推送不得把搜刮屏恢复为 expected open session");
    }

    @Test
    void firstSnapshotFailsClosedWhenAnotherSessionAlreadyReplacedTheExpectedOne() {
        LootContainerStateStore.OpenSession expected = session(19L);
        LootContainerStateStore.open(expected);
        LootContainerUiStateSource source = LootContainerUiStateSource.production(expected);

        LootContainerStateStore.open(session(20L));

        LootContainerScreenViewModel initial = source.snapshot();
        assertInstanceOf(LootContainerSession.Closed.class, initial.session(),
            "source 首次读取遇到另一 session 时必须 fail closed，不能把新 session 显示到旧 screen");
        assertEquals("session replaced", ((LootContainerSession.Closed) initial.session()).reason());
    }

    @Test
    void snapshotFailsClosedAfterDisconnectClearsTheStoreWithoutAnEvent() {
        LootContainerStateStore.OpenSession expected = session(20L);
        LootContainerStateStore.open(expected);
        LootContainerUiStateSource source = LootContainerUiStateSource.production(expected);

        LootContainerStateStore.clearOnDisconnect();

        LootContainerScreenViewModel disconnected = source.snapshot();
        assertInstanceOf(LootContainerSession.Closed.class, disconnected.session(),
            "断线清理不发事件时，source 也必须阻止旧搜刮 session 复活");
        assertEquals("session unavailable", ((LootContainerSession.Closed) disconnected.session()).reason());
    }

    private static LootContainerStateStore.OpenSession session(long id) {
        return new LootContainerStateStore.OpenSession(
            id, "dead_drop", "rare", 2, 3, 0L, List.of()
        );
    }
}
