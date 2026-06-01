package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@link NpcLodHandler} 全路径测试（plan-offscreen-war-v1 §P7 PR-11）。
 *
 * <p>覆盖：happy path、所有拒绝路径、字段解析、守恒红线（无 qi 字段）。
 */
public class NpcLodHandlerTest {

    @AfterEach
    void clearStore() {
        NpcLodStore.clearAll();
    }

    // ── channel 常量契约 ───────────────────────────────────────────────────────

    @Test
    void channelNamespaceAndPathAreCorrect() {
        assertEquals("bong", NpcLodHandler.CHANNEL_NAMESPACE,
            "期望 CHANNEL_NAMESPACE=bong（与 server ident!(\"bong:npc_lod\") 对齐），实际=" + NpcLodHandler.CHANNEL_NAMESPACE);
        assertEquals("npc_lod", NpcLodHandler.CHANNEL_PATH,
            "期望 CHANNEL_PATH=npc_lod，实际=" + NpcLodHandler.CHANNEL_PATH);
    }

    @Test
    void versionConstantIsOne() {
        assertEquals(1, NpcLodHandler.VERSION,
            "期望 VERSION=1（与 server NpcLodS2c.v=1 对齐），实际=" + NpcLodHandler.VERSION);
    }

    // ── happy path ────────────────────────────────────────────────────────────

    @Test
    void storesValidNpcLodPayload() {
        String payload = """
            {"v":1,"type":"npc_lod","entity_id":42,"archetype":"rogue","realm":"引气",\
            "hp_ratio":0.75,"x":100.5,"y":64.0,"z":-200.0}
            """.trim();

        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望解析合法 payload 返回 true，实际 false");

        NpcLodSnapshot got = NpcLodStore.get(42);
        assertNotNull(got, "期望 handle 后 NpcLodStore.get(42) 返回非 null，但得到 null");
        assertEquals(42, got.entityId(), "期望 entityId=42，实际=" + got.entityId());
        assertEquals("rogue", got.archetype(), "期望 archetype=rogue，实际=" + got.archetype());
        assertEquals("引气", got.realm(), "期望 realm=引气，实际=" + got.realm());
        assertTrue(Math.abs(got.hpRatio() - 0.75f) < 1e-3f,
            "期望 hpRatio≈0.75，实际=" + got.hpRatio());
        assertTrue(Math.abs(got.x() - 100.5) < 1e-6,
            "期望 x≈100.5，实际=" + got.x());
        assertTrue(Math.abs(got.y() - 64.0) < 1e-6,
            "期望 y≈64.0，实际=" + got.y());
        assertTrue(Math.abs(got.z() - (-200.0)) < 1e-6,
            "期望 z≈-200.0，实际=" + got.z());
    }

    @Test
    void storesAllSixRealmVariants() {
        String[] realms = {"醒灵", "引气", "凝脉", "固元", "通灵", "化虚"};
        int entityId = 100;
        for (String realm : realms) {
            String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":" + entityId
                + ",\"archetype\":\"zombie\",\"realm\":\"" + realm
                + "\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
            assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
                "期望 realm=" + realm + " 的 payload 解析成功，实际 false");
            NpcLodSnapshot got = NpcLodStore.get(entityId);
            assertNotNull(got, "期望 get(" + entityId + ") 非 null，realm=" + realm);
            assertEquals(realm, got.realm(),
                "期望 realm=" + realm + " 正确存储，实际=" + got.realm());
            NpcLodStore.clearAll();
            entityId++;
        }
    }

    @Test
    void storesAllArchetypeVariants() {
        String[] archetypes = {"zombie", "rogue", "commoner", "disciple", "beast",
                "guardian_relic", "daoxiang", "zhinian", "fuya"};
        int entityId = 200;
        for (String arch : archetypes) {
            String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":" + entityId
                + ",\"archetype\":\"" + arch + "\",\"realm\":\"醒灵\""
                + ",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
            assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
                "期望 archetype=" + arch + " 的 payload 解析成功，实际 false");
            NpcLodSnapshot got = NpcLodStore.get(entityId);
            assertNotNull(got, "期望 get(" + entityId + ") 非 null，archetype=" + arch);
            assertEquals(arch, got.archetype(),
                "期望 archetype=" + arch + " 正确存储，实际=" + got.archetype());
            NpcLodStore.clearAll();
            entityId++;
        }
    }

    // ── 拒绝路径 ──────────────────────────────────────────────────────────────

    @Test
    void rejectsNullPayload() {
        assertFalse(NpcLodHandler.handle(null, 10),
            "期望 null payload 返回 false，因为不存在 JSON 可解析");
    }

    @Test
    void rejectsNegativePayloadSize() {
        assertFalse(NpcLodHandler.handle("{}", -1),
            "期望负 payloadSizeBytes 返回 false");
    }

    @Test
    void rejectsOversizePayload() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":1}";
        assertFalse(NpcLodHandler.handle(payload, ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1),
            "期望 payloadSizeBytes > MAX 返回 false，因为超出限制");
        assertNull(NpcLodStore.get(1), "期望 oversize 拒绝后 store 中无数据");
    }

    @Test
    void rejectsMalformedJson() {
        assertFalse(NpcLodHandler.handle("{not valid json!!!}", 20),
            "期望 malformed JSON 返回 false");
    }

    @Test
    void rejectsWrongVersion() {
        String payload = "{\"v\":99,\"type\":\"npc_lod\",\"entity_id\":1,\"archetype\":\"rogue\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertFalse(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望 v=99（非 1）返回 false");
    }

    @Test
    void rejectsMissingVersion() {
        String payload = "{\"type\":\"npc_lod\",\"entity_id\":1,\"archetype\":\"rogue\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertFalse(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望缺少 v 字段返回 false");
    }

    @Test
    void rejectsWrongType() {
        String payload = "{\"v\":1,\"type\":\"npc_metadata\",\"entity_id\":1,\"archetype\":\"rogue\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertFalse(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望 type=npc_metadata（非 npc_lod）返回 false");
    }

    @Test
    void rejectsMissingType() {
        String payload = "{\"v\":1,\"entity_id\":1,\"archetype\":\"rogue\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertFalse(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望缺少 type 字段返回 false");
    }

    @Test
    void rejectsNegativeEntityId() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":-1,\"archetype\":\"rogue\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertFalse(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望 entity_id=-1（负值）返回 false");
        assertNull(NpcLodStore.get(-1), "期望 store 中无 entity_id=-1");
    }

    @Test
    void rejectsMissingEntityId() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"archetype\":\"rogue\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertFalse(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望缺少 entity_id 字段返回 false");
    }

    // ── 字段 fallback ─────────────────────────────────────────────────────────

    @Test
    void missingArchetypeFallsBackToUnknown() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":50,\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望缺少 archetype 字段仍返回 true（有 fallback）");
        NpcLodSnapshot got = NpcLodStore.get(50);
        assertNotNull(got, "期望 store 中有 entityId=50");
        assertEquals("unknown", got.archetype(),
            "期望缺少 archetype fallback 为 'unknown'，实际=" + got.archetype());
    }

    @Test
    void missingRealmFallsBackToDefault() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":51,\"archetype\":\"zombie\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望缺少 realm 字段仍返回 true（有 fallback）");
        NpcLodSnapshot got = NpcLodStore.get(51);
        assertNotNull(got);
        assertEquals("醒灵", got.realm(),
            "期望缺少 realm fallback 为 '醒灵'，实际=" + got.realm());
    }

    @Test
    void missingHpRatioFallsBackToOne() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":52,\"archetype\":\"zombie\",\"realm\":\"醒灵\",\"x\":0,\"y\":0,\"z\":0}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcLodSnapshot got = NpcLodStore.get(52);
        assertNotNull(got);
        assertEquals(1.0f, got.hpRatio(), 1e-4f,
            "期望缺少 hp_ratio fallback 为 1.0（满血），实际=" + got.hpRatio());
    }

    @Test
    void missingCoordinatesFallBackToZero() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":53,\"archetype\":\"zombie\",\"realm\":\"醒灵\",\"hp_ratio\":0.5}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcLodSnapshot got = NpcLodStore.get(53);
        assertNotNull(got);
        assertEquals(0.0, got.x(), 1e-9, "期望缺少 x fallback 为 0.0，实际=" + got.x());
        assertEquals(0.0, got.y(), 1e-9, "期望缺少 y fallback 为 0.0，实际=" + got.y());
        assertEquals(0.0, got.z(), 1e-9, "期望缺少 z fallback 为 0.0，实际=" + got.z());
    }

    // ── 守恒红线：无 qi 字段 ──────────────────────────────────────────────────

    @Test
    void payloadContainsNoQiFields() {
        // 即使 payload 中含有 qi 字段，snapshot 中也不应有；record 无 qi 字段
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":60,\"archetype\":\"rogue\",\"realm\":\"引气\","
            + "\"hp_ratio\":0.8,\"x\":0,\"y\":0,\"z\":0,\"qi_current\":999,\"qi_ratio\":0.9}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望含额外 qi 字段的 payload 仍能成功解析（忽略额外字段）");
        // NpcLodSnapshot 无 qi 字段——只要编译通过已在类型层面保证守恒
        NpcLodSnapshot got = NpcLodStore.get(60);
        assertNotNull(got);
        // hpRatio 正常：0.8
        assertTrue(Math.abs(got.hpRatio() - 0.8f) < 1e-3f,
            "期望 hpRatio=0.8，实际=" + got.hpRatio());
    }

    // ── 边界值 ─────────────────────────────────────────────────────────────────

    @Test
    void entityIdZeroIsAccepted() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":0,\"archetype\":\"zombie\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":0,\"y\":0,\"z\":0}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望 entity_id=0（边界值，非负）被接受，实际 false");
        assertNotNull(NpcLodStore.get(0), "期望 entity_id=0 已存储");
    }

    @Test
    void hpRatioClampedOnStore() {
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":70,\"archetype\":\"zombie\",\"realm\":\"醒灵\",\"hp_ratio\":2.5,\"x\":0,\"y\":0,\"z\":0}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcLodSnapshot got = NpcLodStore.get(70);
        assertNotNull(got);
        assertTrue(got.hpRatio() <= 1.0f,
            "期望 hp_ratio=2.5 clamp 到 ≤1.0，实际=" + got.hpRatio());
    }

    @Test
    void infiniteCoordinateFallsBackToZero() {
        // JSON Infinity 不合法，parseDouble 应 fallback 到 0
        // 用字符串 "Infinity" 测试 getAsDouble 异常路径
        String payload = "{\"v\":1,\"type\":\"npc_lod\",\"entity_id\":80,\"archetype\":\"zombie\",\"realm\":\"醒灵\",\"hp_ratio\":1.0,\"x\":\"Infinity\",\"y\":0,\"z\":0}";
        assertTrue(NpcLodHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "期望 x=Infinity 字符串 fallback 到 0.0 仍返回 true");
        NpcLodSnapshot got = NpcLodStore.get(80);
        assertNotNull(got);
        assertEquals(0.0, got.x(), 1e-9, "期望 x fallback 为 0.0，实际=" + got.x());
    }
}
