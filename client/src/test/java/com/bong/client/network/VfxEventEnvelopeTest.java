package com.bong.client.network;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.OptionalInt;
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
    void parsesPlayAnimInlineFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim-inline.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), "play_anim_inline payload should parse: " + result.errorMessage());
        assertTrue(result.payload() instanceof VfxEventPayload.PlayAnimInline, "expected PlayAnimInline variant");
        VfxEventPayload.PlayAnimInline inline = (VfxEventPayload.PlayAnimInline) result.payload();
        assertEquals(FIXTURE_UUID, inline.targetPlayer());
        assertEquals(new Identifier("bong", "inline_test_pose"), inline.animId());
        assertTrue(inline.animJson().contains("inline_test_pose"));
        assertEquals(3000, inline.priority());
        assertEquals(OptionalInt.of(3), inline.fadeInTicks());
    }

    @Test
    void rejectsPlayAnimInlineEmptyJson() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-inline-empty-json.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("anim_json"));
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
    void acceptsPayloadWithinServerDataEnvelopeBudget() {
        String json = "{\"v\":1,\"type\":\"spawn_particle\",\"event_id\":\"bong:"
            + "a".repeat(9_000)
            + "\",\"origin\":[0.0,64.0,0.0]}";
        int payloadSizeBytes = jsonLen(json);
        assertTrue(payloadSizeBytes > 8192, "fixture must exceed the old 8192 byte client cap");
        assertTrue(
            payloadSizeBytes < ServerDataEnvelope.MAX_PAYLOAD_BYTES,
            "fixture must stay within the Rust/server_data payload budget"
        );

        VfxEventParseResult result = VfxEventEnvelope.parse(json, payloadSizeBytes);

        assertTrue(result.isSuccess(), result.errorMessage());
    }

    @Test
    void parsesSpawnParticleFixtureFull() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-spawn-particle.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), "spawn_particle should parse: " + result.errorMessage());
        assertTrue(result.payload() instanceof VfxEventPayload.SpawnParticle);
        VfxEventPayload.SpawnParticle particle = (VfxEventPayload.SpawnParticle) result.payload();
        assertEquals(new Identifier("bong", "sword_qi_slash"), particle.eventId());
        assertEquals(3, particle.origin().length);
        assertEquals(128.5, particle.origin()[0]);
        assertEquals(64.0, particle.origin()[1]);
        assertEquals(-32.25, particle.origin()[2]);
        assertTrue(particle.direction().isPresent());
        assertEquals(0.7071, particle.direction().get()[0]);
        assertTrue(particle.colorRgb().isPresent());
        assertEquals(0x88CCFF, particle.colorRgb().getAsInt());
        assertTrue(particle.strength().isPresent());
        assertEquals(0.8, particle.strength().get());
        assertEquals(OptionalInt.of(1), particle.count());
        assertEquals(OptionalInt.of(20), particle.durationTicks());
    }

    @Test
    void parsesSpawnParticleMinimalOmittingOptionals() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-spawn-particle-minimal.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), result.errorMessage());
        VfxEventPayload.SpawnParticle particle = (VfxEventPayload.SpawnParticle) result.payload();
        assertEquals(new Identifier("bong", "lingqi_ripple"), particle.eventId());
        assertTrue(particle.direction().isEmpty());
        assertTrue(particle.colorRgb().isEmpty());
        assertTrue(particle.strength().isEmpty());
        assertTrue(particle.count().isEmpty());
        assertTrue(particle.durationTicks().isEmpty());
    }

    @Test
    void rejectsSpawnParticleBadColor() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-particle-bad-color.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("color"));
    }

    @Test
    void rejectsSpawnParticleOriginWrongLength() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-particle-origin-bad-length.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("origin"));
    }

    @Test
    void rejectsSpawnParticleStrengthOutOfRange() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-particle-strength-out-of-range.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("strength"));
    }

    @Test
    void rejectsMissingVersion() {
        String json = "{\"type\":\"play_anim\",\"target_player\":\""
            + FIXTURE_UUID + "\",\"anim_id\":\"bong:foo\",\"priority\":1000}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Missing version"));
    }

    // ── play_entity_anim（黑武士 boss 招式动画）─────────────────────────────────

    @Test
    void parsesPlayEntityAnimFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-vfx-play-entity-anim.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));

        assertTrue(result.isSuccess(), "play_entity_anim payload should parse: " + result.errorMessage());
        assertNotNull(result.payload());
        assertTrue(result.payload() instanceof VfxEventPayload.PlayEntityAnim, "expected PlayEntityAnim variant");
        VfxEventPayload.PlayEntityAnim anim = (VfxEventPayload.PlayEntityAnim) result.payload();
        assertEquals(42, anim.entityId());
        assertEquals("animation.bong.heiwushi.dark_barrage", anim.anim());
        assertEquals(15, anim.durationTicks());
        assertEquals("play_entity_anim", anim.type());
    }

    @Test
    void rejectsPlayEntityAnimEntityIdBelowOne() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-vfx-entity-anim-bad-id.json");
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("entity_id"), result.errorMessage());
    }

    @Test
    void rejectsPlayEntityAnimEmptyAnim() {
        String json = "{\"v\":1,\"type\":\"play_entity_anim\",\"entity_id\":3,\"anim\":\"\",\"duration_ticks\":15}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("anim"), result.errorMessage());
    }

    @Test
    void rejectsPlayEntityAnimDurationOutOfRange() {
        int overMax = VfxEventEnvelope.VFX_ENTITY_ANIM_DURATION_TICKS_MAX + 1;
        String json = "{\"v\":1,\"type\":\"play_entity_anim\",\"entity_id\":3,\"anim\":\"a\",\"duration_ticks\":"
            + overMax + "}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("duration_ticks"), result.errorMessage());

        String zero = "{\"v\":1,\"type\":\"play_entity_anim\",\"entity_id\":3,\"anim\":\"a\",\"duration_ticks\":0}";
        VfxEventParseResult zeroResult = VfxEventEnvelope.parse(zero, jsonLen(zero));
        assertFalse(zeroResult.isSuccess());
        assertTrue(zeroResult.errorMessage().contains("duration_ticks"), zeroResult.errorMessage());
    }

    @Test
    void rejectsPlayEntityAnimMissingEntityId() {
        // 缺 required entity_id → 解析失败，错误消息钉住字段名（与 Rust/TypeBox required 一致）。
        String json = "{\"v\":1,\"type\":\"play_entity_anim\",\"anim\":\"a\",\"duration_ticks\":15}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess(), "缺 entity_id 应被拒");
        assertTrue(result.errorMessage().contains("entity_id"), result.errorMessage());
    }

    @Test
    void rejectsPlayEntityAnimMissingDurationTicks() {
        // 缺 required duration_ticks → 解析失败，错误消息钉住字段名。
        String json = "{\"v\":1,\"type\":\"play_entity_anim\",\"entity_id\":3,\"anim\":\"a\"}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess(), "缺 duration_ticks 应被拒");
        assertTrue(result.errorMessage().contains("duration_ticks"), result.errorMessage());
    }

    @Test
    void rejectsPlayEntityAnimAnimNameTooLong() {
        // 边界（off-by-one 上界外）：anim 名 129 char（超 128 上限）→ 解析失败。
        // 三端逐字对齐 128（server vfx_event.rs / schema vfx-event.ts）。
        String tooLong = "a".repeat(VfxEventEnvelope.VFX_ENTITY_ANIM_NAME_MAX_CHARS + 1);
        String json = "{\"v\":1,\"type\":\"play_entity_anim\",\"entity_id\":3,\"anim\":\""
            + tooLong + "\",\"duration_ticks\":15}";
        VfxEventParseResult result = VfxEventEnvelope.parse(json, jsonLen(json));
        assertFalse(result.isSuccess(), "anim 名超 128 上限应被拒");
        assertTrue(result.errorMessage().contains("exceeds max length"), result.errorMessage());
        assertTrue(result.errorMessage().contains("anim"), result.errorMessage());
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }
}
