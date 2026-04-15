package com.bong.client.network;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class VfxEventEnvelopeTest {
    private static final UUID FIXTURE_UUID = UUID.fromString("550e8400-e29b-41d4-a716-446655440000");

    @Test
    void parsesPlayAnimFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), "play_anim payload should parse: " + result.errorMessage());
        assertNotNull(result.payload());
        assertTrue(result.payload() instanceof VfxEventPayload.PlayAnim, "expected PlayAnim variant");
        VfxEventPayload.PlayAnim play = (VfxEventPayload.PlayAnim) result.payload();
        assertEquals(FIXTURE_UUID, play.targetPlayer());
        assertEquals(new Identifier("bong", "sword_swing_horiz"), play.animId());
        assertEquals(1000, play.priority());
        assertTrue(play.fadeInTicks().isPresent());
        assertEquals(3, play.fadeInTicks().getAsInt());
    }

    @Test
    void parsesPlayAnimMinimalFixtureOmittingFadeIn() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim-minimal.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), result.errorMessage());
        VfxEventPayload.PlayAnim play = (VfxEventPayload.PlayAnim) result.payload();
        assertEquals(200, play.priority());
        assertTrue(play.fadeInTicks().isEmpty(), "fade_in_ticks omitted should map to empty");
    }

    @Test
    void parsesStopAnimFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-stop-anim.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), result.errorMessage());
        assertTrue(result.payload() instanceof VfxEventPayload.StopAnim, "expected StopAnim variant");
        VfxEventPayload.StopAnim stop = (VfxEventPayload.StopAnim) result.payload();
        assertEquals(FIXTURE_UUID, stop.targetPlayer());
        assertEquals(new Identifier("bong", "meditate_sit"), stop.animId());
        assertTrue(stop.fadeOutTicks().isPresent());
        assertEquals(5, stop.fadeOutTicks().getAsInt());
    }

    @Test
    void rejectsUnknownType() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-unknown-type.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Unknown vfx_event type"));
    }

    @Test
    void rejectsBadUuid() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-bad-uuid.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("target_player"));
    }

    @Test
    void rejectsBadAnimId() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-bad-anim-id.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("anim_id"));
    }

    @Test
    void rejectsPriorityOutOfRange() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-priority-out-of-range.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("priority"));
    }

    @Test
    void rejectsFadeTicksOutOfRange() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-fade-out-of-range.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("fade_in_ticks"));
    }

    @Test
    void rejectsWrongVersion() {
        String json = "{\"v\":2,\"type\":\"play_anim\",\"target_player\":\""
            + FIXTURE_UUID + "\",\"anim_id\":\"bong:foo\",\"priority\":1000}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Unsupported version"));
    }

    @Test
    void rejectsOversizePayload() {
        String json = "{\"v\":1,\"type\":\"play_anim\"}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, VfxEventEnvelope.MAX_PAYLOAD_BYTES + 1);

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("exceeds max size"));
    }

    @Test
    void rejectsMissingVersion() {
        String json = "{\"type\":\"play_anim\",\"target_player\":\""
            + FIXTURE_UUID + "\",\"anim_id\":\"bong:foo\",\"priority\":1000}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Missing version"));
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }
}
