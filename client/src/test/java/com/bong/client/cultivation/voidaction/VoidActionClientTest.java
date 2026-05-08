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
    void legacyParserAcceptsCommaSeparatedIds() {
        assertEquals(List.of(1001L, 1002L, 1003L), LegacyAssignPanel.parseIds("1001, 1002,,1003"));
    }

    @Test
    void storeKeepsLegacyDraft() {
        VoidActionStore.setLegacyDraft("heir", List.of(7L), " 一封死信 ");
        VoidActionStore.Snapshot snapshot = VoidActionStore.snapshot();
        assertEquals("heir", snapshot.legacyInheritorId());
        assertEquals(List.of(7L), snapshot.legacyItemInstanceIds());
        assertEquals("一封死信", snapshot.legacyMessage());
    }
}
