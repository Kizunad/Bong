package com.bong.client.cultivation;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * F7：{@link BreakthroughBillboardWorldRenderer} headless 可测的纯逻辑部分
 * （label 映射 + alpha 合成）。真实 {@link net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents}
 * 渲染效果需人工 {@code ./gradlew runClient} 目视验收——风格仿
 * {@code com.bong.client.npc.NpcLodWorldRendererTest} 的分层注释。
 */
class BreakthroughBillboardWorldRendererTest {

    // ── labelFor ──────────────────────────────────────────────────────────────

    @Test
    void labelFor_preludeIsGenericTribulationGlyph() {
        assertEquals("劫", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.PRELUDE, "pending", false)));
    }

    @Test
    void labelFor_chargeIsGenericTribulationGlyph() {
        assertEquals("劫", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.CHARGE, "pending", false)));
    }

    @Test
    void labelFor_catalyzeIsGenericTribulationGlyph() {
        assertEquals("劫", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.CATALYZE, "pending", false)));
    }

    @Test
    void labelFor_apexIsGenericTribulationGlyph() {
        assertEquals("劫", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.APEX, "pending", false)));
    }

    @Test
    void labelFor_aftermathSuccess_isSuccessGlyph() {
        assertEquals("成", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.AFTERMATH, "success", false)));
    }

    @Test
    void labelFor_aftermathFailure_isFailureGlyph() {
        assertEquals("破", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.AFTERMATH, "failure", false)));
    }

    @Test
    void labelFor_aftermathInterrupted_isFailureGlyph_evenIfResultPending() {
        // interrupted 优先于 result：即使 result 还是 pending，被打断也应算失败态
        assertEquals("破", BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.AFTERMATH, "pending", true)));
    }

    @Test
    void labelFor_aftermathSuccessGlyph_differsFromFailureGlyph() {
        String success = BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.AFTERMATH, "success", false));
        String failure = BreakthroughBillboardWorldRenderer.labelFor(
            payload(BreakthroughCinematicPayload.Phase.AFTERMATH, "failure", false));
        org.junit.jupiter.api.Assertions.assertNotEquals(success, failure,
            "成功/失败 aftermath 标签必须视觉可区分");
    }

    // ── applyAlpha ────────────────────────────────────────────────────────────

    @Test
    void applyAlpha_fullOpacity_setsFFAlphaByte() {
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0x00112233, 1.0);
        assertEquals(0xFF112233, result,
            "alpha=1.0 应写入 0xFF 高位字节，RGB 保持不变");
    }

    @Test
    void applyAlpha_zeroOpacity_setsZeroAlphaByte() {
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0x00112233, 0.0);
        assertEquals(0x00112233, result,
            "alpha=0.0 应写入 0x00 高位字节");
    }

    @Test
    void applyAlpha_midOpacity_roundsToNearestByte() {
        // 0.5 * 255 = 127.5 → round → 128 = 0x80
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0x00ABCDEF, 0.5);
        assertEquals(0x80ABCDEF, result,
            "alpha=0.5 应四舍五入到 0x80（128/255），实际=0x" + Integer.toHexString(result));
    }

    @Test
    void applyAlpha_ignoresPreexistingAlphaByteInInput() {
        // 输入自带 0xCC 高位字节（如 tintFor() 的占位值），应被完全覆盖而非叠加
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0xCC445566, 1.0);
        assertEquals(0xFF445566, result,
            "输入自带的高位字节应被 applyAlpha 完全覆盖，不应保留原 0xCC");
    }

    @Test
    void applyAlpha_negativeInput_clampsToZero() {
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0x00123456, -0.5);
        assertEquals(0x00123456, result,
            "alpha 为负数应 clamp 到 0（不产生负字节）");
    }

    @Test
    void applyAlpha_aboveOneInput_clampsToFullOpacity() {
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0x00123456, 1.5);
        assertEquals(0xFF123456, result,
            "alpha 超过 1.0 应 clamp 到 0xFF（不溢出）");
    }

    @Test
    void applyAlpha_nanInput_clampsToZero() {
        int result = BreakthroughBillboardWorldRenderer.applyAlpha(0x00123456, Double.NaN);
        assertEquals(0x00123456, result,
            "alpha=NaN 应安全 fallback 到 0（不崩、不产生非法字节）");
    }

    private static BreakthroughCinematicPayload payload(
        BreakthroughCinematicPayload.Phase phase, String result, boolean interrupted
    ) {
        return new BreakthroughCinematicPayload(
            "actor",
            phase,
            0,
            80,
            "Condense",
            "Solidify",
            BreakthroughCinematicPayload.Result.fromWire(result),
            interrupted,
            10.0,
            64.0,
            10.0,
            1024.0,
            false,
            true,
            1.0,
            0.5,
            "adaptive",
            "fresh_spiral",
            100L
        );
    }
}
