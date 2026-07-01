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

    // ──────────────────────────────────────────────────────────────────────
    // F19 fix — BongNetworkHandler's DISCONNECT block cleared a dozen-plus
    // stores but forgot TiandaoPresenceStore, so a stale presence (watch/
    // pressure/tribulation/annihilate vignette+shake) kept rendering across a
    // disconnect/reconnect. BongNetworkHandler.register() wires a Fabric
    // ClientPlayConnectionEvents.DISCONNECT callback that requires a live
    // Minecraft client instance and cannot be invoked directly from a unit
    // test; mirrors DyingElderEncounterTest's source-scan pattern used for
    // the same class of unreachable-from-unit-test wiring.
    // ──────────────────────────────────────────────────────────────────────

    @Test
    void bongNetworkHandlerClearsTiandaoPresenceStoreOnDisconnect() throws Exception {
        java.nio.file.Path testClasses = java.nio.file.Path.of("").toAbsolutePath().normalize();
        java.nio.file.Path clientRoot;
        if (java.nio.file.Files.isDirectory(testClasses.resolve("src"))) {
            clientRoot = testClasses;
        } else if (java.nio.file.Files.isDirectory(testClasses.resolve("client").resolve("src"))) {
            clientRoot = testClasses.resolve("client");
        } else {
            clientRoot = testClasses;
        }
        java.nio.file.Path handlerSrc = clientRoot.resolve(
            "src/main/java/com/bong/client/BongNetworkHandler.java"
        );
        assertTrue(
            java.nio.file.Files.exists(handlerSrc),
            "BongNetworkHandler.java must exist at " + handlerSrc.toAbsolutePath()
                + " — cannot verify F19 fix without source file"
        );
        String src = java.nio.file.Files.readString(handlerSrc);

        int disconnectBlockStart = src.indexOf("ClientPlayConnectionEvents.DISCONNECT.register(");
        assertTrue(
            disconnectBlockStart >= 0,
            "expected a ClientPlayConnectionEvents.DISCONNECT.register(...) block in BongNetworkHandler, "
                + "actual: not found in source"
        );
        int disconnectBlockEnd = src.indexOf("ClientPlayConnectionEvents.JOIN.register(", disconnectBlockStart);
        assertTrue(
            disconnectBlockEnd > disconnectBlockStart,
            "expected a JOIN.register(...) block after DISCONNECT.register(...) to bound the disconnect block"
        );
        String disconnectBlock = src.substring(disconnectBlockStart, disconnectBlockEnd);

        assertTrue(
            disconnectBlock.contains("TiandaoPresenceStore.clear()"),
            "expected the DISCONNECT block to call TiandaoPresenceStore.clear() because without it a stale "
                + "presence vignette/shake survives across disconnect/reconnect (F19 fix), actual: call not found "
                + "inside the DISCONNECT block"
        );
    }
}
