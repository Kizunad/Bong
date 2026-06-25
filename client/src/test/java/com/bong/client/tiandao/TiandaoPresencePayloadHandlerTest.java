package com.bong.client.tiandao;

import com.bong.client.network.ServerDataEnvelope;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TiandaoPresencePayloadHandlerTest {

    @AfterEach
    void tearDown() {
        TiandaoPresenceStore.clear();
    }

    @Test
    void validPayloadReplacesStoreState() {
        TiandaoPresencePayloadHandler.Result result = TiandaoPresencePayloadHandler.handle(
            """
            {"v":1,"type":"tiandao_presence","response":"pressure","level":48.5,"zone":"spawn","zone_spirit_qi":0.6,"vignette_rgb":4196352,"vignette_alpha":0.08,"shake_intensity":0.35,"saturation":0.95,"tick":200}
            """,
            160
        );

        assertTrue(result.handled(), result.logMessage());
        TiandaoPresenceState state = TiandaoPresenceStore.snapshot();
        assertTrue(state.active());
        assertEquals("pressure", state.response());
        assertEquals("spawn", state.zone());
        assertEquals(48.5, state.level(), 1e-6);
        assertEquals(0.95, state.saturation(), 1e-6);
    }

    @Test
    void validPayloadWithinServerDataEnvelopeBudgetReplacesStoreState() {
        String zone = "z".repeat(9000);
        String payload = """
            {"v":1,"type":"tiandao_presence","response":"pressure","level":12.5,"zone":"%s","zone_spirit_qi":0.3,"vignette_rgb":4196352,"vignette_alpha":0.08,"shake_intensity":0.1,"saturation":0.95,"tick":42}
            """.formatted(zone);
        int payloadSizeBytes = payload.getBytes(StandardCharsets.UTF_8).length;
        assertTrue(payloadSizeBytes > 8192, "fixture must exceed the old 8192 byte client cap");
        assertTrue(
            payloadSizeBytes < ServerDataEnvelope.MAX_PAYLOAD_BYTES,
            "fixture must remain within the server_data payload budget"
        );

        TiandaoPresencePayloadHandler.Result result =
            TiandaoPresencePayloadHandler.handle(payload, payloadSizeBytes);

        assertTrue(result.handled(), result.logMessage());
        TiandaoPresenceState state = TiandaoPresenceStore.snapshot();
        assertTrue(state.active());
        assertEquals("pressure", state.response());
        assertEquals(zone, state.zone());
    }

    @Test
    void invalidVersionDoesNotMutateStore() {
        TiandaoPresenceStore.replace(TiandaoPresenceState.empty());
        TiandaoPresencePayloadHandler.Result result = TiandaoPresencePayloadHandler.handle(
            """
            {"v":2,"type":"tiandao_presence","response":"watch"}
            """,
            44
        );

        assertFalse(result.handled());
        assertFalse(TiandaoPresenceStore.snapshot().active());
    }

    @Test
    void invalidSizeRejectedBeforeParsing() {
        TiandaoPresencePayloadHandler.Result result =
            TiandaoPresencePayloadHandler.handle("{}", ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1);
        assertFalse(result.handled());
        assertFalse(TiandaoPresenceStore.snapshot().active());
    }

    @Test
    void malformedJsonRejected() {
        TiandaoPresencePayloadHandler.Result result = TiandaoPresencePayloadHandler.handle("{", 1);
        assertFalse(result.handled());
        assertFalse(TiandaoPresenceStore.snapshot().active());
    }
}
