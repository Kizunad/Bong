package com.bong.client.combat.screen;

import com.bong.client.network.ClientRequestProtocol;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

final class ZhenfaLayoutScreenTest {

    @Test
    void defaultsClassicArrayToOriginTrapAndKeepsTrigger() {
        ZhenfaLayoutScreen screen = new ZhenfaLayoutScreen(null, null, 0L, null);
        JsonObject payload = payload(screen);

        assertString(payload, "type", "zhenfa_place", "the screen emits placement requests");
        assertInt(payload, "v", 1, "the client request schema version must stay stable");
        assertInt(payload, "x", 0, "null target position falls back to the default layout origin");
        assertInt(payload, "y", 64, "null target position falls back to the default layout origin");
        assertInt(payload, "z", 0, "null target position falls back to the default layout origin");
        assertString(payload, "kind", "trap", "null kind maps to the classic array trap");
        assertString(payload, "carrier", "common_stone", "the layout screen always places common-stone carriers");
        assertDouble(payload, "qi_invest_ratio", 0.1, "the default slider value should be preserved");
        assertString(payload, "trigger", "proximity", "classic arrays keep the trigger selection");
    }

    @Test
    void fixedTrapOmitsTriggerAndForwardsItemAndTargetFace() {
        ZhenfaLayoutScreen screen = new ZhenfaLayoutScreen(
            new BlockPos(11, 64, -3),
            ClientRequestProtocol.ZhenfaKind.BLAST_TRAP,
            9001L,
            ClientRequestProtocol.ZhenfaTargetFace.NORTH
        );
        JsonObject payload = payload(screen);

        assertInt(payload, "x", 11, "the clicked block x coordinate must be forwarded");
        assertInt(payload, "y", 64, "the clicked block y coordinate must be forwarded");
        assertInt(payload, "z", -3, "the clicked block z coordinate must be forwarded");
        assertString(payload, "kind", "blast_trap", "ordinary blast traps preserve their request kind");
        assertString(payload, "carrier", "common_stone", "ordinary traps use the common-stone carrier");
        assertFalse(
            payload.has("trigger"),
            "expected trigger to be omitted because ordinary traps use fixed server-side trigger semantics; actual payload=" + payload
        );
        assertLong(payload, "item_instance_id", 9001L, "positive item ids must be sent so the server consumes the held trap");
        assertString(payload, "target_face", "north", "the clicked block face must be forwarded for surface placement");
    }

    @Test
    void nonPositiveItemIdsAreOmittedForFixedTraps() {
        ZhenfaLayoutScreen zeroItem = new ZhenfaLayoutScreen(
            new BlockPos(1, 65, 2),
            ClientRequestProtocol.ZhenfaKind.SLOW_TRAP,
            0L,
            ClientRequestProtocol.ZhenfaTargetFace.TOP
        );
        ZhenfaLayoutScreen negativeItem = new ZhenfaLayoutScreen(
            new BlockPos(1, 65, 2),
            ClientRequestProtocol.ZhenfaKind.WARNING_TRAP,
            -1L,
            null
        );
        JsonObject zeroPayload = payload(zeroItem);
        JsonObject negativePayload = payload(negativeItem);

        assertString(zeroPayload, "kind", "slow_trap", "zero item id still preserves the requested trap kind");
        assertString(zeroPayload, "target_face", "top", "zero item id should not erase the clicked target face");
        assertFalse(
            zeroPayload.has("item_instance_id"),
            "expected zero item id to be omitted because only positive inventory ids are valid; actual payload=" + zeroPayload
        );
        assertFalse(
            zeroPayload.has("trigger"),
            "expected slow_trap trigger to be omitted because ordinary traps use fixed trigger semantics; actual payload=" + zeroPayload
        );
        assertString(negativePayload, "kind", "warning_trap", "negative item id still preserves the requested trap kind");
        assertFalse(
            negativePayload.has("item_instance_id"),
            "expected negative item id to be omitted because it cannot reference inventory state; actual payload=" + negativePayload
        );
        assertFalse(
            negativePayload.has("target_face"),
            "expected absent target face to remain omitted for vertical embedded traps; actual payload=" + negativePayload
        );
    }

    private static JsonObject payload(ZhenfaLayoutScreen screen) {
        return JsonParser.parseString(screen.encodePlacementRequestForTests()).getAsJsonObject();
    }

    private static void assertString(JsonObject payload, String field, String expected, String reason) {
        assertEquals(
            expected,
            payload.get(field).getAsString(),
            "expected " + field + "=" + expected + " because " + reason + "; actual payload=" + payload
        );
    }

    private static void assertInt(JsonObject payload, String field, int expected, String reason) {
        assertEquals(
            expected,
            payload.get(field).getAsInt(),
            "expected " + field + "=" + expected + " because " + reason + "; actual payload=" + payload
        );
    }

    private static void assertLong(JsonObject payload, String field, long expected, String reason) {
        assertEquals(
            expected,
            payload.get(field).getAsLong(),
            "expected " + field + "=" + expected + " because " + reason + "; actual payload=" + payload
        );
    }

    private static void assertDouble(JsonObject payload, String field, double expected, String reason) {
        assertEquals(
            expected,
            payload.get(field).getAsDouble(),
            0.0001,
            "expected " + field + "=" + expected + " because " + reason + "; actual payload=" + payload
        );
    }
}
