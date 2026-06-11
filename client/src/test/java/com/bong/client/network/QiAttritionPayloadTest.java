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
    void parsesBoundaryValues() {
        String json = "{\"v\":1,\"item_entity_id\":0,\"amount_lost\":1.0E-12,\"world_pos\":[0,64,0]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertTrue(result.success(), result.errorMessage());
        assertEquals(0L, result.payload().itemEntityId());
        assertEquals(1.0E-12, result.payload().amountLost());
    }

    @Test
    void rejectsWrongVersion() {
        String json = "{\"v\":2,\"item_entity_id\":42,\"amount_lost\":3.25,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("Unsupported version"));
    }

    @Test
    void rejectsMissingRequiredFieldsIndividually() {
        assertParseError(
            "{\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,65,2]}",
            "version"
        );
        assertParseError("{\"v\":1,\"amount_lost\":1.0,\"world_pos\":[1,65,2]}", "item_entity_id");
        assertParseError("{\"v\":1,\"item_entity_id\":42,\"world_pos\":[1,65,2]}", "amount_lost");
        assertParseError("{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0}", "world_pos");
    }

    @Test
    void rejectsNegativeItemEntityId() {
        String json = "{\"v\":1,\"item_entity_id\":-1,\"amount_lost\":1.0,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("item_entity_id"));
    }

    @Test
    void rejectsNonPositiveAmount() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":0.0,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("amount_lost"));
    }

    @Test
    void rejectsNonFiniteAmount() {
        assertParseError(
            "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":NaN,\"world_pos\":[1,65,2]}",
            "amount_lost"
        );
        assertParseError(
            "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":Infinity,\"world_pos\":[1,65,2]}",
            "amount_lost"
        );
    }

    @Test
    void rejectsBadWorldPos() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,65]}";
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("world_pos"));
    }

    @Test
    void rejectsWorldPosNullOrNonArray() {
        assertParseError("{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":null}", "world_pos");
        assertParseError("{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":64}", "world_pos");
    }

    @Test
    void rejectsNonFiniteWorldPosElements() {
        assertParseError(
            "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,NaN,2]}",
            "world_pos"
        );
        assertParseError(
            "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,Infinity,2]}",
            "world_pos"
        );
    }

    @Test
    void rejectsTopLevelNonObject() {
        assertParseError("[1,2,3]", "top-level object");
        assertParseError("42", "top-level object");
    }

    @Test
    void rejectsOversizePayload() {
        String json = "{\"v\":1,\"item_entity_id\":42,\"amount_lost\":1.0,\"world_pos\":[1,65,2]}";
        QiAttritionPayload.ParseResult result =
            QiAttritionPayload.parse(json, QiAttritionPayload.MAX_PAYLOAD_BYTES + 1);

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains("Payload size"));
    }

    private static void assertParseError(String json, String expectedMessagePart) {
        QiAttritionPayload.ParseResult result = QiAttritionPayload.parse(json, jsonLen(json));

        assertFalse(result.success());
        assertTrue(result.errorMessage().contains(expectedMessagePart), result.errorMessage());
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }
}
