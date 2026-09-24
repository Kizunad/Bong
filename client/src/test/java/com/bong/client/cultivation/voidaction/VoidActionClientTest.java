package com.bong.client.cultivation.voidaction;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class VoidActionClientTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        VoidActionStore.resetForTests();
    }

    private void installBackend() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void handlerDispatchesSuppressTsyAndStartsCooldown() {
        installBackend();

        assertTrue(VoidActionHandler.dispatchSuppressTsy("tsy_lingxu", 10L));

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"void_action\",\"v\":1,\"request\":{\"kind\":\"suppress_tsy\",\"zone_id\":\"tsy_lingxu\"}}",
            sent.get(0).body()
        );
        assertFalse(VoidActionStore.snapshot().ready(VoidActionKind.SUPPRESS_TSY, 11L));
    }

    @Test
    void handlerSkipsWhenCooldownIsActive() {
        installBackend();
        assertTrue(VoidActionHandler.dispatchExplodeZone("spawn", 10L));
        assertFalse(VoidActionHandler.dispatchExplodeZone("spawn", 11L));
        assertEquals(1, sent.size());
    }

    @Test
    void clearOnDisconnect_replacesEmptySnapshotAndNotifiesLongLivedListeners() {
        List<VoidActionStore.Snapshot> notifications = new ArrayList<>();
        VoidActionStore.addListener(notifications::add);
        notifications.clear();

        VoidActionStore.clearOnDisconnect();

        assertEquals(VoidActionStore.Snapshot.empty(), VoidActionStore.snapshot(),
            "断线必须清掉旧会话的化虚 action 草稿与冷却状态");
        assertEquals(List.of(VoidActionStore.Snapshot.empty()), notifications,
            "断线必须经 replace(empty) 通知现有 listener，使已挂载 UI 同步收起旧会话状态");

        VoidActionStore.setTargetZone("fresh-zone");

        assertEquals(2, notifications.size(),
            "断线清理不得删除长期 listener；新会话写入仍必须通知同一 listener");
        assertEquals("fresh-zone", notifications.get(1).targetZoneId());
    }

}
