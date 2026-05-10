package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NpcMetadataHandlerTest {
    @AfterEach
    void clearStore() {
        NpcMetadataStore.clearAll();
    }

    @Test
    void storesValidNpcMetadataPayload() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":42,"archetype":"rogue","realm":"凝脉","faction_name":"中立盟","faction_rank":"客卿","reputation_to_player":55,"display_name":"散修·凝脉","age_band":"正值壮年","greeting_text":"道友，可有灵草出让？","qi_hint":"真元流转平稳"}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(java.nio.charset.StandardCharsets.UTF_8).length));

        NpcMetadata metadata = NpcMetadataStore.get(42);
        assertEquals("rogue", metadata.archetype());
        assertEquals("散修·凝脉", metadata.displayName());
        assertEquals("道友，可有灵草出让？", metadata.greetingText());
        assertTrue(metadata.tradeCandidate());
    }

    @Test
    void rejectsOversizeNpcMetadataPayload() {
        String payload = "{\"type\":\"npc_metadata\",\"v\":1,\"entity_id\":42}";

        assertFalse(NpcMetadataHandler.handle(payload, ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1));
        assertEquals(0, NpcMetadataStore.snapshot().size());
    }

    @Test
    void rejectsNegativeEntityId() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":-1,"archetype":"rogue","realm":"凝脉","reputation_to_player":0,"display_name":"散修·凝脉"}
            """.trim();

        assertFalse(NpcMetadataHandler.handle(payload, payload.getBytes(java.nio.charset.StandardCharsets.UTF_8).length));
        assertEquals(0, NpcMetadataStore.snapshot().size());
    }
}
