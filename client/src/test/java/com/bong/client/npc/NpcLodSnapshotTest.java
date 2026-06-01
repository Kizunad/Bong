package com.bong.client.npc;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@link NpcLodSnapshot} 字段约束 + 距离计算测试（plan-offscreen-war-v1 §P7 PR-11）。
 */
public class NpcLodSnapshotTest {

    // ── happy path ────────────────────────────────────────────────────────────

    @Test
    void constructsWithAllFields() {
        NpcLodSnapshot snap = new NpcLodSnapshot(42, "rogue", "引气", 0.75f, 100.0, 64.0, -200.0);

        assertEquals(42, snap.entityId(),
            "期望 entityId=42，实际=" + snap.entityId());
        assertEquals("rogue", snap.archetype(),
            "期望 archetype=rogue，实际=" + snap.archetype());
        assertEquals("引气", snap.realm(),
            "期望 realm=引气，实际=" + snap.realm());
        assertTrue(Math.abs(snap.hpRatio() - 0.75f) < 1e-4f,
            "期望 hpRatio≈0.75，实际=" + snap.hpRatio());
        assertTrue(Math.abs(snap.x() - 100.0) < 1e-9,
            "期望 x=100.0，实际=" + snap.x());
        assertTrue(Math.abs(snap.y() - 64.0) < 1e-9,
            "期望 y=64.0，实际=" + snap.y());
        assertTrue(Math.abs(snap.z() - (-200.0)) < 1e-9,
            "期望 z=-200.0，实际=" + snap.z());
    }

    // ── hpRatio clamp ─────────────────────────────────────────────────────────

    @Test
    void hpRatioClampsAboveOne() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "zombie", "醒灵", 1.5f, 0, 0, 0);

        assertTrue(snap.hpRatio() <= 1.0f,
            "期望 hpRatio clamp 到 ≤1.0，因为输入 1.5 超出范围，实际=" + snap.hpRatio());
        assertEquals(1.0f, snap.hpRatio(), 1e-4f,
            "期望 hpRatio=1.0（clamp上限），实际=" + snap.hpRatio());
    }

    @Test
    void hpRatioClampsZero() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "zombie", "醒灵", -0.5f, 0, 0, 0);

        assertTrue(snap.hpRatio() >= 0.0f,
            "期望 hpRatio clamp 到 ≥0.0，因为输入 -0.5 低于范围，实际=" + snap.hpRatio());
        assertEquals(0.0f, snap.hpRatio(), 1e-4f,
            "期望 hpRatio=0.0（clamp下限），实际=" + snap.hpRatio());
    }

    @Test
    void hpRatioBoundary_zeroAndOne() {
        NpcLodSnapshot zero = new NpcLodSnapshot(1, "zombie", "醒灵", 0.0f, 0, 0, 0);
        NpcLodSnapshot one = new NpcLodSnapshot(2, "zombie", "醒灵", 1.0f, 0, 0, 0);

        assertEquals(0.0f, zero.hpRatio(), 1e-4f,
            "期望 hpRatio=0.0（边界值），实际=" + zero.hpRatio());
        assertEquals(1.0f, one.hpRatio(), 1e-4f,
            "期望 hpRatio=1.0（边界值），实际=" + one.hpRatio());
    }

    // ── archetype/realm fallback ──────────────────────────────────────────────

    @Test
    void nullArchetypeFallsBackToUnknown() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, null, "醒灵", 1.0f, 0, 0, 0);

        assertEquals("unknown", snap.archetype(),
            "期望 null archetype fallback 到 'unknown'，实际=" + snap.archetype());
    }

    @Test
    void blankArchetypeFallsBackToUnknown() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "   ", "醒灵", 1.0f, 0, 0, 0);

        assertEquals("unknown", snap.archetype(),
            "期望空白 archetype fallback 到 'unknown'，实际=" + snap.archetype());
    }

    @Test
    void nullRealmFallsBackToDefault() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", null, 1.0f, 0, 0, 0);

        assertEquals("醒灵", snap.realm(),
            "期望 null realm fallback 到 '醒灵'，实际=" + snap.realm());
    }

    @Test
    void blankRealmFallsBackToDefault() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", "  ", 1.0f, 0, 0, 0);

        assertEquals("醒灵", snap.realm(),
            "期望空白 realm fallback 到 '醒灵'，实际=" + snap.realm());
    }

    @Test
    void archetypeAndRealmTrimmed() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, " beast ", " 凝脉 ", 1.0f, 0, 0, 0);

        assertEquals("beast", snap.archetype(),
            "期望 archetype 前后空白被 trim，实际=" + snap.archetype());
        assertEquals("凝脉", snap.realm(),
            "期望 realm 前后空白被 trim，实际=" + snap.realm());
    }

    // ── distanceSquaredXzTo ───────────────────────────────────────────────────

    @Test
    void distanceSquaredXzOnSamePoint() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "zombie", "醒灵", 1.0f, 100.0, 64.0, 200.0);
        double distSq = snap.distanceSquaredXzTo(100.0, 200.0);

        assertEquals(0.0, distSq, 1e-9,
            "期望 distSq=0 当 NPC 与玩家在同一 XZ 位置，实际=" + distSq);
    }

    @Test
    void distanceSquaredXzIgnoresY() {
        // NPC y=1000, player y=0 — XZ 距离应为 100*100+0 = 10000
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "zombie", "醒灵", 1.0f, 100.0, 1000.0, 0.0);
        double distSq = snap.distanceSquaredXzTo(0.0, 0.0);

        assertEquals(100.0 * 100.0, distSq, 1e-9,
            "期望 distSq=10000（XZ 距离 100，Y 不参与计算），实际=" + distSq);
    }

    @Test
    void distanceSquaredXz_knownValue() {
        // NPC at (300, 64, 400), player at (0, 64, 0)
        // dx=300, dz=400 → distSq = 90000 + 160000 = 250000
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", "引气", 0.5f, 300.0, 64.0, 400.0);
        double distSq = snap.distanceSquaredXzTo(0.0, 0.0);

        assertEquals(250000.0, distSq, 1e-6,
            "期望 distSq=250000（dx=300, dz=400），实际=" + distSq);
    }

    @Test
    void distanceSquaredXzNegativeCoordinates() {
        // NPC at (-100, 64, -50), player at (50, 0, 50)
        // dx = -100 - 50 = -150, dz = -50 - 50 = -100
        // distSq = 22500 + 10000 = 32500
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "beast", "凝脉", 1.0f, -100.0, 64.0, -50.0);
        double distSq = snap.distanceSquaredXzTo(50.0, 50.0);

        assertEquals(32500.0, distSq, 1e-6,
            "期望 distSq=32500（负坐标，dx=-150, dz=-100），实际=" + distSq);
    }
}
