package com.bong.client.npc;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@link NpcLodWorldRenderer} LOD gate 阈值 + label/color 逻辑测试
 * （plan-offscreen-war-v1 §P7 PR-11）。
 *
 * <p>只测 headless 可测的纯逻辑部分（距离阈值判断、label 映射、color 映射）；
 * 实际 WorldRenderEvents 渲染效果需人工 {@code ./gradlew runClient} 目视验收。
 */
public class NpcLodWorldRendererTest {

    // ── NEAR_CUTOFF_DISTANCE 常量锁定 ─────────────────────────────────────────

    @Test
    void nearCutoffDistanceIsEightyBlocks() {
        assertEquals(80.0, NpcLodWorldRenderer.NEAR_CUTOFF_DISTANCE, 1e-9,
            "期望 NEAR_CUTOFF_DISTANCE=80.0（对应 server NpcLodConfig.near_radius=80），实际="
                + NpcLodWorldRenderer.NEAR_CUTOFF_DISTANCE);
    }

    @Test
    void farCutoffDistanceIs256Blocks() {
        assertEquals(256.0, NpcLodWorldRenderer.FAR_CUTOFF_DISTANCE, 1e-9,
            "期望 FAR_CUTOFF_DISTANCE=256.0（对应 server NpcLodConfig.mid_radius=256），实际="
                + NpcLodWorldRenderer.FAR_CUTOFF_DISTANCE);
    }

    // ── isInLodRange ──────────────────────────────────────────────────────────

    /** 恰好在边界内（80格，闭区间下界）。 */
    @Test
    void isInLodRange_exactlyAtNearBoundary_included() {
        // NPC at (80, 64, 0), player at (0, 0, 0): xzDist=80 → distSq=6400 == nearSq=6400
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", "引气", 1.0f, 80.0, 64.0, 0.0);
        assertTrue(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 xzDist=80（恰好等于 NEAR_CUTOFF）在 LOD range 内（闭区间），实际 false");
    }

    /** 距离 79格（在 Near 档内），应被排除。 */
    @Test
    void isInLodRange_justInsideNear_excluded() {
        // xzDist≈79: 79^2=6241 < nearSq=6400
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", "引气", 1.0f, 79.0, 64.0, 0.0);
        assertFalse(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 xzDist=79（小于 NEAR_CUTOFF=80）不在 LOD range 内，实际 true");
    }

    /** 恰好在边界上（256格，闭区间上界）。 */
    @Test
    void isInLodRange_exactlyAtFarBoundary_included() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "zombie", "醒灵", 1.0f, 256.0, 64.0, 0.0);
        assertTrue(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 xzDist=256（恰好等于 FAR_CUTOFF）在 LOD range 内（闭区间），实际 false");
    }

    /** 距离 257格（超出 Mid 档），应被排除。 */
    @Test
    void isInLodRange_justOutsideFar_excluded() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "zombie", "醒灵", 1.0f, 257.0, 64.0, 0.0);
        assertFalse(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 xzDist=257（超出 FAR_CUTOFF=256）不在 LOD range 内，实际 true");
    }

    /** 中间距离（120格）应在范围内。 */
    @Test
    void isInLodRange_midRange_included() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "commoner", "凝脉", 0.5f, 120.0, 64.0, 0.0);
        assertTrue(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 xzDist=120（Mid 范围内）在 LOD range，实际 false");
    }

    /** Y 轴不参与距离判断。 */
    @Test
    void isInLodRange_yAxisIgnored() {
        // NPC 在 xz=(100,0)，y=99999（极端高空）；xzDist=100 在范围内
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "beast", "固元", 1.0f, 100.0, 99999.0, 0.0);
        assertTrue(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 Y 轴不影响 LOD 范围判断（xzDist=100 在 80–256 内），实际 false");
    }

    /** 玩家非原点的相对位置正确计算。 */
    @Test
    void isInLodRange_nonOriginPlayer() {
        // NPC at (500, 64, 500), player at (400, 64, 400): dx=100, dz=100 → xzDist=√20000≈141
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "disciple", "化虚", 1.0f, 500.0, 64.0, 500.0);
        assertTrue(NpcLodWorldRenderer.isInLodRange(snap, 400.0, 400.0),
            "期望非原点玩家的相对距离≈141格在 LOD range 内，实际 false");
    }

    /** 同一位置（距离 0）应被排除（Near 档 <80格）。 */
    @Test
    void isInLodRange_samePosition_excluded() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", "引气", 1.0f, 0.0, 64.0, 0.0);
        assertFalse(NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0),
            "期望 xzDist=0（同一位置）不在 LOD range（<80格），实际 true");
    }

    // ── lodLabelFor ───────────────────────────────────────────────────────────

    @Test
    void lodLabelForAllKnownArchetypes() {
        assertEquals("尸", NpcLodWorldRenderer.lodLabelFor("zombie"),
            "期望 zombie→尸");
        assertEquals("散", NpcLodWorldRenderer.lodLabelFor("rogue"),
            "期望 rogue→散");
        assertEquals("凡", NpcLodWorldRenderer.lodLabelFor("commoner"),
            "期望 commoner→凡");
        assertEquals("余", NpcLodWorldRenderer.lodLabelFor("disciple"),
            "期望 disciple→余");
        assertEquals("兽", NpcLodWorldRenderer.lodLabelFor("beast"),
            "期望 beast→兽");
        assertEquals("守", NpcLodWorldRenderer.lodLabelFor("guardian_relic"),
            "期望 guardian_relic→守");
        assertEquals("伥", NpcLodWorldRenderer.lodLabelFor("daoxiang"),
            "期望 daoxiang→伥");
        assertEquals("执", NpcLodWorldRenderer.lodLabelFor("zhinian"),
            "期望 zhinian→执");
        assertEquals("压", NpcLodWorldRenderer.lodLabelFor("fuya"),
            "期望 fuya→压");
    }

    @Test
    void lodLabelForUnknownArchetypeFallsBackToDefault() {
        assertEquals("人", NpcLodWorldRenderer.lodLabelFor("unknown_archetype"),
            "期望未知 archetype fallback 到 '人'");
        assertEquals("人", NpcLodWorldRenderer.lodLabelFor(null),
            "期望 null archetype fallback 到 '人'");
        assertEquals("人", NpcLodWorldRenderer.lodLabelFor(""),
            "期望空字符串 archetype fallback 到 '人'");
    }

    @Test
    void lodLabelForCaseInsensitive() {
        assertEquals("尸", NpcLodWorldRenderer.lodLabelFor("ZOMBIE"),
            "期望 ZOMBIE（大写）与 zombie 结果一致");
        assertEquals("散", NpcLodWorldRenderer.lodLabelFor("Rogue"),
            "期望 Rogue（首字母大写）与 rogue 结果一致");
    }

    // ── lodColorFor ───────────────────────────────────────────────────────────

    @Test
    void lodColorForHostileArchetypesAreReddish() {
        int zombieColor = NpcLodWorldRenderer.lodColorFor("zombie");
        int relicColor = NpcLodWorldRenderer.lodColorFor("guardian_relic");

        // 提取 R 通道 (bits 16-23)；偏红色的 R > G, R > B
        int zombieR = (zombieColor >> 16) & 0xFF;
        int zombieG = (zombieColor >> 8) & 0xFF;
        assertTrue(zombieR > zombieG,
            "期望 zombie 颜色 R(" + zombieR + ") > G(" + zombieG + ")（偏红色，敌意），实际不符");

        int relicR = (relicColor >> 16) & 0xFF;
        int relicG = (relicColor >> 8) & 0xFF;
        assertTrue(relicR > relicG,
            "期望 guardian_relic 颜色 R(" + relicR + ") > G(" + relicG + ")（偏红色），实际不符");
    }

    @Test
    void lodColorForBeastIsBrownish() {
        int beastColor = NpcLodWorldRenderer.lodColorFor("beast");
        // beast 棕色：R > G > B
        int r = (beastColor >> 16) & 0xFF;
        int g = (beastColor >> 8) & 0xFF;
        int b = beastColor & 0xFF;
        assertTrue(r >= g,
            "期望 beast 颜色 R(" + r + ") ≥ G(" + g + ")（棕色色调），实际不符");
        assertTrue(g >= b,
            "期望 beast 颜色 G(" + g + ") ≥ B(" + b + ")（棕色色调），实际不符");
    }

    @Test
    void lodColorForNullArchetypeReturnsDefault() {
        int nullColor = NpcLodWorldRenderer.lodColorFor(null);
        int defaultColor = NpcLodWorldRenderer.lodColorFor("rogue");
        assertEquals(defaultColor, nullColor,
            "期望 null archetype 返回与普通 archetype 相同的默认颜色");
    }

    @Test
    void lodColorForUnknownArchetypeReturnsDefault() {
        int unknownColor = NpcLodWorldRenderer.lodColorFor("not_an_archetype");
        int defaultColor = NpcLodWorldRenderer.lodColorFor("commoner");
        assertEquals(defaultColor, unknownColor,
            "期望未知 archetype 返回与 commoner 相同的默认颜色");
    }

    // ── LOD gate 与 NpcMetadataStore 的互动 ──────────────────────────────────

    /**
     * isInLodRange 是距离纯函数，不依赖 NpcMetadataStore——
     * 即使 store 有 metadata，gate 逻辑本身仍按距离判断。
     * （render() 方法中会有额外逻辑跳过 near，但 isInLodRange 本身不感知 store）
     */
    @Test
    void isInLodRange_isPureDistanceFunction() {
        NpcLodSnapshot snap = new NpcLodSnapshot(1, "rogue", "引气", 1.0f, 150.0, 64.0, 0.0);
        boolean result1 = NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0);
        boolean result2 = NpcLodWorldRenderer.isInLodRange(snap, 0.0, 0.0);
        assertEquals(result1, result2,
            "期望 isInLodRange 是纯函数（相同输入总返回相同结果），实际两次结果不一致");
        assertTrue(result1, "期望 xzDist=150 在 LOD range 内，实际 false");
    }
}
