package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class QiAttritionPayloadTest {
    @Test
    void parsesPlanPacketFormat() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":3.25,\"world_pos\":[1.5,65.0,-2.25]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertTrue(result.success(), result.errorMessage());
        assertEquals(42L, result.payload().itemEntityId());
        assertEquals(3.25, result.payload().amountLost());
        assertArrayEquals(new double[] {1.5, 65.0, -2.25}, result.payload().worldPos());
    }

    @Test
    void rejectsWrongVersion() {
        String json = "{\"v\":2,\"item_entity_id\":42,\"amount_lost\":3.25,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("Unsupported version"));
    }

    @Test
    void rejectsNonPositiveAmount() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":0.0,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("amount_lost"));
    }

    @Test
    void rejectsBadWorldPos() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,65]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("world_pos"));
    }

    @Test
    void rejectsOversizePayload() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result =
            QiAttritionPayload.parse(json, QiAttritionPayload.MAX_PAYLOAD_BYTES + 1);

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("Payload size"));
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }
}
