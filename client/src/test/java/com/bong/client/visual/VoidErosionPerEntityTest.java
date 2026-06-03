package com.bong.client.visual;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 CR fix — per-entity alpha 串扰修复测试。
 *
 * <p>锁住"仅匹配实体应用 alpha，其他实体不受影响"契约：
 * 多玩家场景中每个实体独立存储虚蚀状态，渲染器只读取匹配自身的 State，
 * 其他玩家的 erosion 不串扰本玩家模型透明度。
 */
class VoidErosionPerEntityTest {

    @AfterEach
    void cleanup() {
        VoidErosionVisualStore.reset();
    }

    // ── per-entity 独立存储 ──────────────────────────────────────────────────

    @Test
    void twoEntitiesStoredIndependently() {
        // 玩家 A：虚蚀阶段 4（半透明）
        VoidErosionVisualStore.replace("offline:alice", 4, 420.0, true, 0.4f, true);
        // 玩家 B：阶段 0（不透明）
        VoidErosionVisualStore.replace("offline:bob", 0, 0.0, false, 1.0f, false);

        VoidErosionVisualStore.State alice = VoidErosionVisualStore.snapshotForEntity("offline:alice");
        VoidErosionVisualStore.State bob = VoidErosionVisualStore.snapshotForEntity("offline:bob");

        assertNotNull(alice, "alice 应有虚蚀状态");
        assertNotNull(bob, "bob 应有虚蚀状态");
        assertEquals(4, alice.stage(), "alice 应为阶段 4");
        assertEquals(0, bob.stage(), "bob 应为阶段 0，不受 alice 影响");
        assertEquals(0.4f, alice.modelAlpha(), 0.001f, "alice modelAlpha 应为 0.4");
        assertEquals(1.0f, bob.modelAlpha(), 0.001f, "bob modelAlpha 应为 1.0，不受 alice 影响");
    }

    @Test
    void unknownEntityReturnsNull() {
        VoidErosionVisualStore.replace("offline:alice", 4, 420.0, true, 0.4f, true);

        // 未收到 payload 的玩家应返回 null（渲染器跳过该玩家）
        VoidErosionVisualStore.State charlie = VoidErosionVisualStore.snapshotForEntity("offline:charlie");
        assertNull(charlie, "未收到 payload 的玩家应返回 null，不套 alice 的 alpha");
    }

    @Test
    void alphaOfOneEntityDoesNotAffectOther() {
        // alice 处于阶段 4（严重虚蚀）
        VoidErosionVisualStore.replace("offline:alice", 4, 420.0, true, 0.4f, true);

        // bob 无虚蚀 payload（snapshotForEntity 返回 null → 渲染器跳过 bob）
        VoidErosionVisualStore.State bobState = VoidErosionVisualStore.snapshotForEntity("offline:bob");
        assertNull(bobState,
                "bob 无虚蚀 payload，snapshotForEntity 应返回 null，渲染器不应为 bob 应用 alice 的 alpha 0.4");
    }

    @Test
    void updateOneEntityDoesNotOverwriteOther() {
        VoidErosionVisualStore.replace("offline:alice", 1, 25.0, false, 0.85f, false);
        VoidErosionVisualStore.replace("offline:bob", 2, 85.0, false, 0.70f, false);

        // 更新 alice 到阶段 3
        VoidErosionVisualStore.replace("offline:alice", 3, 210.0, true, 0.55f, true);

        VoidErosionVisualStore.State alice = VoidErosionVisualStore.snapshotForEntity("offline:alice");
        VoidErosionVisualStore.State bob = VoidErosionVisualStore.snapshotForEntity("offline:bob");

        assertNotNull(alice);
        assertNotNull(bob);
        assertEquals(3, alice.stage(), "alice 应更新为阶段 3");
        assertEquals(2, bob.stage(), "bob 应保持阶段 2，不受 alice 更新影响");
        assertEquals(0.55f, alice.modelAlpha(), 0.001f);
        assertEquals(0.70f, bob.modelAlpha(), 0.001f);
    }

    @Test
    void removeOneEntityLeavesOtherIntact() {
        VoidErosionVisualStore.replace("offline:alice", 2, 90.0, false, 0.70f, false);
        VoidErosionVisualStore.replace("offline:bob", 3, 210.0, true, 0.55f, true);

        VoidErosionVisualStore.remove("offline:alice");

        assertNull(VoidErosionVisualStore.snapshotForEntity("offline:alice"),
                "remove 后 alice 应为 null");
        assertNotNull(VoidErosionVisualStore.snapshotForEntity("offline:bob"),
                "remove alice 不应影响 bob");
        assertEquals(3, VoidErosionVisualStore.snapshotForEntity("offline:bob").stage());
    }

    @Test
    void resetClearsAllEntities() {
        VoidErosionVisualStore.replace("offline:alice", 4, 420.0, true, 0.4f, true);
        VoidErosionVisualStore.replace("offline:bob", 2, 90.0, false, 0.70f, false);

        VoidErosionVisualStore.reset();

        assertNull(VoidErosionVisualStore.snapshotForEntity("offline:alice"),
                "reset 后 alice 应为 null");
        assertNull(VoidErosionVisualStore.snapshotForEntity("offline:bob"),
                "reset 后 bob 应为 null");
        assertTrue(VoidErosionVisualStore.allSnapshots().isEmpty(),
                "reset 后 allSnapshots 应为空");
    }

    // ── resolveWireId 逻辑 ─────────────────────────────────────────────────

    @Test
    void resolveWireIdMatchesOfflinePrefix() {
        VoidErosionVisualStore.replace("offline:kiz", 2, 90.0, false, 0.70f, false);

        // entity name = "kiz" → 应解析为 "offline:kiz"
        String wireId = VoidErosionModelAlphaRenderer.resolveWireId("kiz");
        assertEquals("offline:kiz", wireId,
                "resolveWireId('kiz') 应优先解析为 'offline:kiz'（最常见 offline mode 格式）");
    }

    @Test
    void resolveWireIdFallsBackToLowerCase() {
        // 如果存储的是纯小写名（无 offline: 前缀）
        VoidErosionVisualStore.replace("alice", 1, 25.0, false, 0.85f, false);

        String wireId = VoidErosionModelAlphaRenderer.resolveWireId("Alice");
        assertEquals("alice", wireId,
                "resolveWireId('Alice') 应 fallback 到小写 'alice'");
    }

    @Test
    void resolveWireIdDefaultOfflineWhenNoMatch() {
        // store 中无对应实体
        String wireId = VoidErosionModelAlphaRenderer.resolveWireId("charlie");
        assertEquals("offline:charlie", wireId,
                "无 payload 时 resolveWireId 应默认返回 'offline:charlie'");
    }

    @Test
    void resolveWireIdHandlesNullEntityName() {
        // null 或空 name 不应抛出异常
        assertDoesNotThrow(() -> VoidErosionModelAlphaRenderer.resolveWireId(null),
                "null entityName 不应抛 NPE");
        assertDoesNotThrow(() -> VoidErosionModelAlphaRenderer.resolveWireId(""),
                "空 entityName 不应抛出");
    }

    // ── handler → per-entity store 集成 ─────────────────────────────────────

    @Test
    void handlerUpdatesPerEntityStoreForEachEntity() {
        // 两个连续 payload，分属不同 entityId
        VoidErosionVisualHandler.handle("""
                {"entity_id":"offline:alice","stage":2,"cumulative_erosion":90.0,
                 "ambient_active":false,"model_alpha":0.70,"sound_distortion_active":false,
                 "server_tick":100}""");
        VoidErosionVisualHandler.handle("""
                {"entity_id":"offline:bob","stage":4,"cumulative_erosion":420.0,
                 "ambient_active":true,"model_alpha":0.40,"sound_distortion_active":true,
                 "server_tick":200}""");

        VoidErosionVisualStore.State alice = VoidErosionVisualStore.snapshotForEntity("offline:alice");
        VoidErosionVisualStore.State bob = VoidErosionVisualStore.snapshotForEntity("offline:bob");

        assertNotNull(alice, "alice 应有状态");
        assertNotNull(bob, "bob 应有状态");
        assertEquals(2, alice.stage(), "alice 阶段应为 2");
        assertEquals(4, bob.stage(), "bob 阶段应为 4");
        assertEquals(0.70f, alice.modelAlpha(), 0.001f,
                "alice modelAlpha 应为 0.70，不被 bob 的 0.40 覆盖");
        assertEquals(0.40f, bob.modelAlpha(), 0.001f, "bob modelAlpha 应为 0.40");
        assertFalse(alice.voidDistortionActive(),
                "alice 阶段 2 无扭曲 overlay");
        assertTrue(bob.voidDistortionActive(),
                "bob 阶段 4 有扭曲 overlay");
    }
}
