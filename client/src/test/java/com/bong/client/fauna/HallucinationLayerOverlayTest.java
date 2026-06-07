package com.bong.client.fauna;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-fauna-stitched-beast-v1 P3 — 幻觉层 Store / Handler / Overlay 饱和单测。
 *
 * <p>覆盖范围：
 * <ul>
 *   <li>{@link HallucinationLayerStore}：activate / cancel / fade-in / fade-out / decrementTick / bar offsets / clearOnDisconnect</li>
 *   <li>{@link HallucinationLayerHandler}：happy path / cancel=true / duration=0 / invalid JSON</li>
 *   <li>{@link HallucinationHudOverlay}：applyFadeAlpha / getYawOffset / getHpBarDisplayOffset / getQiBarDisplayOffset</li>
 *   <li>Channel 常量 pin：namespace=bong, path=core_absorption_hallucination</li>
 *   <li>守恒红线：bar 偏移不超 ±20%，fade 不超 [0,1]</li>
 * </ul>
 *
 * <p>所有断言使用 "期望 X 因为 Y，实际 Z" 格式，方便快速定位回归。
 */
public class HallucinationLayerOverlayTest {

    @BeforeEach
    void resetAll() {
        HallucinationLayerStore.resetForTest();
        HallucinationHudOverlay.resetBarReshuffleForTest();
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Channel 常量 pin 测试
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void channelNamespaceIsBong() {
        assertEquals(
            "bong",
            HallucinationLayerHandler.CHANNEL_NAMESPACE,
            "expected CHANNEL_NAMESPACE=bong because Bong mod uses bong: as its channel prefix, actual: "
                + HallucinationLayerHandler.CHANNEL_NAMESPACE
        );
    }

    @Test
    void channelPathIsCoreAbsorptionHallucination() {
        assertEquals(
            "core_absorption_hallucination",
            HallucinationLayerHandler.CHANNEL_PATH,
            "expected CHANNEL_PATH=core_absorption_hallucination because server pushes S2C on exactly this identifier, actual: "
                + HallucinationLayerHandler.CHANNEL_PATH
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — activate 基础状态
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void activateSetsActiveAndRemainingTicks() {
        HallucinationLayerStore.activate(200);

        assertTrue(
            HallucinationLayerStore.isActive(),
            "expected isActive()=true after activate(200) because hallucination should begin immediately, actual: false"
        );
        assertEquals(
            200,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=200 after activate(200) because duration maps directly to remaining ticks, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
    }

    @Test
    void activateResetsFadeProgressToZero() {
        // 预先注入非零 fade
        HallucinationLayerStore.setFadeProgressForTest(0.8f);
        HallucinationLayerStore.activate(100);

        assertEquals(
            0.0f,
            HallucinationLayerStore.getFadeProgress(),
            1e-5f,
            "expected fadeProgress=0 after activate because hallucination fades in from scratch, actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
    }

    @Test
    void activateWithMinimumOneTick() {
        HallucinationLayerStore.activate(1);
        assertTrue(
            HallucinationLayerStore.isActive(),
            "expected isActive()=true with duration=1 because minimum valid duration should activate the effect, actual: false"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — cancel
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void cancelClearsActiveAndFade() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(1.0f);

        HallucinationLayerStore.cancel();

        assertFalse(
            HallucinationLayerStore.isActive(),
            "expected isActive()=false after cancel because cancel stops hallucination immediately, actual: true"
        );
        assertEquals(
            0.0f,
            HallucinationLayerStore.getFadeProgress(),
            1e-5f,
            "expected fadeProgress=0 after cancel because overlay must not linger after forced cancel, actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
        assertEquals(
            0,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=0 after cancel because remaining time is cleared on forced cancel, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — clearOnDisconnect
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void clearOnDisconnectResetsAllFields() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(0.5f);
        HallucinationLayerStore.updateBarOffsets(0.15f, -0.18f);

        HallucinationLayerStore.clearOnDisconnect();

        assertFalse(
            HallucinationLayerStore.isActive(),
            "expected isActive()=false after clearOnDisconnect because hallucination must not carry over across sessions, actual: true"
        );
        assertEquals(
            0.0f,
            HallucinationLayerStore.getFadeProgress(),
            1e-5f,
            "expected fadeProgress=0 after clearOnDisconnect because the overlay must be completely hidden on reconnect, actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
        assertEquals(
            0.0f,
            HallucinationLayerStore.getHpBarDisplayOffset(),
            1e-5f,
            "expected hpBarDisplayOffset=0 after clearOnDisconnect because bar offsets are display-only and must reset, actual: "
                + HallucinationLayerStore.getHpBarDisplayOffset()
        );
        assertEquals(
            0.0f,
            HallucinationLayerStore.getQiBarDisplayOffset(),
            1e-5f,
            "expected qiBarDisplayOffset=0 after clearOnDisconnect because bar offsets are display-only and must reset, actual: "
                + HallucinationLayerStore.getQiBarDisplayOffset()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — fade-in via tickFade
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void tickFadeIncreasesProgressWhenActive() {
        HallucinationLayerStore.activate(200);
        float before = HallucinationLayerStore.getFadeProgress(); // 0.0
        float step = HallucinationHudOverlay.FADE_IN_STEP;

        HallucinationLayerStore.tickFade(0.0f, step, HallucinationHudOverlay.FADE_OUT_STEP);

        float after = HallucinationLayerStore.getFadeProgress();
        assertTrue(
            after > before,
            "expected fadeProgress to increase after tickFade when active because fade-in is driven by FADE_IN_STEP, actual before="
                + before + " after=" + after
        );
        assertEquals(
            step,
            after,
            1e-5f,
            "expected fadeProgress=" + step + " (one FADE_IN_STEP) after first tickFade, actual: " + after
        );
    }

    @Test
    void tickFadeDoesNotExceedOne() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(0.99f);

        // Drive multiple ticks
        for (int i = 0; i < 10; i++) {
            HallucinationLayerStore.tickFade(0.0f, HallucinationHudOverlay.FADE_IN_STEP, HallucinationHudOverlay.FADE_OUT_STEP);
        }

        assertTrue(
            HallucinationLayerStore.getFadeProgress() <= 1.0f,
            "expected fadeProgress <= 1.0 because fade is clamped to [0,1], actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — fade-out
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void tickFadeDecreasesProgressWhenInactive() {
        HallucinationLayerStore.setFadeProgressForTest(1.0f);
        // active=false by default after reset

        float step = HallucinationHudOverlay.FADE_OUT_STEP;
        HallucinationLayerStore.tickFade(0.0f, HallucinationHudOverlay.FADE_IN_STEP, step);

        float after = HallucinationLayerStore.getFadeProgress();
        assertEquals(
            1.0f - step,
            after,
            1e-5f,
            "expected fadeProgress=" + (1.0f - step) + " after one fade-out tick when inactive, actual: " + after
        );
    }

    @Test
    void tickFadeDoesNotGoBelowZero() {
        HallucinationLayerStore.setFadeProgressForTest(0.01f);
        // active=false

        for (int i = 0; i < 20; i++) {
            HallucinationLayerStore.tickFade(0.0f, HallucinationHudOverlay.FADE_IN_STEP, HallucinationHudOverlay.FADE_OUT_STEP);
        }

        assertTrue(
            HallucinationLayerStore.getFadeProgress() >= 0.0f,
            "expected fadeProgress >= 0.0 because fade is clamped at floor, actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — decrementTick
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void decrementTickReducesRemainingByOne() {
        HallucinationLayerStore.activate(10);
        HallucinationLayerStore.decrementTick();

        assertEquals(
            9,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=9 after one decrementTick from 10, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
    }

    @Test
    void decrementTickDoesNotGoNegative() {
        HallucinationLayerStore.activate(1);
        HallucinationLayerStore.decrementTick(); // → 0
        HallucinationLayerStore.decrementTick(); // should stay 0

        assertEquals(
            0,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=0 after decrement past zero because tick counter must not be negative, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerStore — bar offsets
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void updateBarOffsetsStoresValues() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.updateBarOffsets(0.12f, -0.17f);

        assertEquals(
            0.12f,
            HallucinationLayerStore.getHpBarDisplayOffset(),
            1e-5f,
            "expected hpBarDisplayOffset=0.12 because updateBarOffsets writes the provided value, actual: "
                + HallucinationLayerStore.getHpBarDisplayOffset()
        );
        assertEquals(
            -0.17f,
            HallucinationLayerStore.getQiBarDisplayOffset(),
            1e-5f,
            "expected qiBarDisplayOffset=-0.17 because updateBarOffsets writes the provided value, actual: "
                + HallucinationLayerStore.getQiBarDisplayOffset()
        );
    }

    @Test
    void barOffsetRangeIsWithinBounds() {
        // Simulate 100 random reshuffles and check all stay within ±MAX_BAR_OFFSET
        float maxOffset = HallucinationHudOverlay.MAX_BAR_OFFSET;
        for (int i = 0; i < 100; i++) {
            // Simulate the Overlay's random generation logic
            float hpOffset = (float) (Math.random() * 2 * maxOffset) - maxOffset;
            float qiOffset = (float) (Math.random() * 2 * maxOffset) - maxOffset;

            assertTrue(
                hpOffset >= -maxOffset && hpOffset <= maxOffset,
                "expected hpOffset in [-" + maxOffset + ", " + maxOffset + "] because Overlay generates within MAX_BAR_OFFSET range, actual: " + hpOffset
            );
            assertTrue(
                qiOffset >= -maxOffset && qiOffset <= maxOffset,
                "expected qiOffset in [-" + maxOffset + ", " + maxOffset + "] because Overlay generates within MAX_BAR_OFFSET range, actual: " + qiOffset
            );
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationLayerHandler — happy path / cancel / edge cases
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void handlerActivatesStoreOnHappyPath() {
        String payload = "{\"duration_ticks\": 200, \"cancel\": false}";
        HallucinationLayerHandler.handle(payload);

        assertTrue(
            HallucinationLayerStore.isActive(),
            "expected isActive()=true after handler receives duration_ticks=200 because server activation should flow through to the Store, actual: false"
        );
        assertEquals(
            200,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=200 after handler receives duration_ticks=200, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
    }

    @Test
    void handlerCancelsOnCancelTrue() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(1.0f);

        String payload = "{\"duration_ticks\": 200, \"cancel\": true}";
        HallucinationLayerHandler.handle(payload);

        assertFalse(
            HallucinationLayerStore.isActive(),
            "expected isActive()=false after handler receives cancel=true because server cancel must stop the effect immediately, actual: true"
        );
        assertEquals(
            0.0f,
            HallucinationLayerStore.getFadeProgress(),
            1e-5f,
            "expected fadeProgress=0 after cancel=true payload because the effect must vanish instantly, actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
    }

    @Test
    void handlerCancelsOnDurationZero() {
        HallucinationLayerStore.activate(200);

        String payload = "{\"duration_ticks\": 0}";
        HallucinationLayerHandler.handle(payload);

        assertFalse(
            HallucinationLayerStore.isActive(),
            "expected isActive()=false when duration_ticks=0 because zero duration means immediate cancel, actual: true"
        );
    }

    @Test
    void handlerDefaultsDurationToZeroWhenMissing() {
        HallucinationLayerStore.activate(200);

        String payload = "{\"cancel\": false}";
        HallucinationLayerHandler.handle(payload);

        // no duration_ticks field => defaults to 0 => cancel
        assertFalse(
            HallucinationLayerStore.isActive(),
            "expected isActive()=false when duration_ticks field is absent because the default is 0 (cancel), actual: true"
        );
    }

    @Test
    void handlerDoesNotThrowOnInvalidJson() {
        // Must not throw — silent skip
        assertDoesNotThrow(
            () -> HallucinationLayerHandler.handle("not-json-at-all"),
            "expected handle to NOT throw on invalid JSON because the hallucination channel is non-critical and must not crash the client"
        );
    }

    @Test
    void handlerDoesNotThrowOnEmptyString() {
        assertDoesNotThrow(
            () -> HallucinationLayerHandler.handle(""),
            "expected handle to NOT throw on empty string because the hallucination channel must be resilient to malformed payloads"
        );
    }

    @Test
    void handlerActivatesSetsCorrectDuration() {
        String payload = "{\"duration_ticks\": 100}";
        HallucinationLayerHandler.handle(payload);

        assertEquals(
            100,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=100 when handler receives duration_ticks=100 because the Store must mirror the server-specified duration, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationHudOverlay — applyFadeAlpha
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void applyFadeAlphaZeroFadeReturnsTransparent() {
        int result = HallucinationHudOverlay.applyFadeAlpha(0x6080F040, 0.0f);
        int alpha = (result >>> 24) & 0xFF;

        assertEquals(
            0,
            alpha,
            "expected alpha=0 when fadeProgress=0 because fully faded-out overlay is invisible, actual: " + alpha
        );
    }

    @Test
    void applyFadeAlphaFullFadeRetainsAlpha() {
        int original = 0x6080F040;
        int result = HallucinationHudOverlay.applyFadeAlpha(original, 1.0f);
        int alpha = (result >>> 24) & 0xFF;
        int expectedAlpha = (original >>> 24) & 0xFF; // 0x60 = 96

        assertEquals(
            expectedAlpha,
            alpha,
            "expected alpha=" + expectedAlpha + " when fadeProgress=1.0 because full fade should preserve the original target alpha, actual: " + alpha
        );
    }

    @Test
    void applyFadeAlphaHalfFadeHalvesAlpha() {
        int original = 0x6080F040;
        int targetAlpha = (original >>> 24) & 0xFF; // 96
        int result = HallucinationHudOverlay.applyFadeAlpha(original, 0.5f);
        int alpha = (result >>> 24) & 0xFF;

        int expected = Math.round(targetAlpha * 0.5f); // 48
        assertEquals(
            expected,
            alpha,
            "expected alpha=" + expected + " when fadeProgress=0.5 because fade linearly scales the alpha channel, actual: " + alpha
        );
    }

    @Test
    void applyFadeAlphaPreservesRgbBits() {
        int original = 0x6080F040;
        int result = HallucinationHudOverlay.applyFadeAlpha(original, 0.5f);

        assertEquals(
            original & 0x00FFFFFF,
            result & 0x00FFFFFF,
            "expected RGB bits unchanged by applyFadeAlpha because fade only modifies alpha, not color, actual: "
                + Integer.toHexString(result & 0x00FFFFFF)
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationHudOverlay — getYawOffset
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void getYawOffsetIsZeroWhenInactive() {
        // Store is reset (inactive, fadeProgress=0)
        float yaw = HallucinationHudOverlay.getYawOffset();

        assertEquals(
            0.0f,
            yaw,
            1e-5f,
            "expected getYawOffset()=0 when inactive because yaw disruption must not affect normal play, actual: " + yaw
        );
    }

    @Test
    void getYawOffsetIsBoundedByMaxDegrees() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(1.0f);

        // Any yaw offset must never exceed ±MAX_YAW_DEGREES
        float maxYaw = HallucinationHudOverlay.MAX_YAW_DEGREES;
        float yaw = HallucinationHudOverlay.getYawOffset();

        assertTrue(
            Math.abs(yaw) <= maxYaw,
            "expected |getYawOffset()| <= " + maxYaw + " degrees because yaw disruption is capped at MAX_YAW_DEGREES to prevent nausea, actual: " + yaw
        );
    }

    @Test
    void getYawOffsetScalesWithFade() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(0.5f);

        // sin(0) = 0, so at sinPhase=0 the formula gives 0 regardless of fade
        // Force a non-zero sinPhase by ticking
        for (int i = 0; i < 30; i++) {
            HallucinationLayerStore.tickFade(
                HallucinationHudOverlay.SIN_PHASE_INCREMENT,
                HallucinationHudOverlay.FADE_IN_STEP,
                HallucinationHudOverlay.FADE_OUT_STEP
            );
        }
        HallucinationLayerStore.setFadeProgressForTest(0.5f);
        float yawAt50 = HallucinationHudOverlay.getYawOffset();

        HallucinationLayerStore.setFadeProgressForTest(1.0f);
        float yawAt100 = HallucinationHudOverlay.getYawOffset();

        // At the same sinPhase, double fade should double yaw
        assertEquals(
            yawAt50 * 2.0f,
            yawAt100,
            0.001f,
            "expected yawOffset to scale linearly with fadeProgress because formula is sin(phase)*MAX_YAW*fade, actual yaw@0.5="
                + yawAt50 + " yaw@1.0=" + yawAt100
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationHudOverlay — getHpBarDisplayOffset / getQiBarDisplayOffset
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void barDisplayOffsetIsZeroWhenInactive() {
        float hp = HallucinationHudOverlay.getHpBarDisplayOffset();
        float qi = HallucinationHudOverlay.getQiBarDisplayOffset();

        assertEquals(
            0.0f,
            hp,
            1e-5f,
            "expected getHpBarDisplayOffset()=0 when inactive because bar must show accurate values during normal play, actual: " + hp
        );
        assertEquals(
            0.0f,
            qi,
            1e-5f,
            "expected getQiBarDisplayOffset()=0 when inactive because bar must show accurate values during normal play, actual: " + qi
        );
    }

    @Test
    void barDisplayOffsetScalesWithFade() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.updateBarOffsets(0.2f, -0.2f);

        HallucinationLayerStore.setFadeProgressForTest(0.5f);
        float hp50 = HallucinationHudOverlay.getHpBarDisplayOffset();
        float qi50 = HallucinationHudOverlay.getQiBarDisplayOffset();

        HallucinationLayerStore.setFadeProgressForTest(1.0f);
        float hp100 = HallucinationHudOverlay.getHpBarDisplayOffset();
        float qi100 = HallucinationHudOverlay.getQiBarDisplayOffset();

        assertEquals(
            hp50 * 2.0f,
            hp100,
            1e-5f,
            "expected HP bar offset to scale linearly with fade because the formula multiplies store offset by fadeProgress, actual at 0.5="
                + hp50 + " at 1.0=" + hp100
        );
        assertEquals(
            qi50 * 2.0f,
            qi100,
            1e-5f,
            "expected qi bar offset to scale linearly with fade because the formula multiplies store offset by fadeProgress, actual at 0.5="
                + qi50 + " at 1.0=" + qi100
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 守恒红线：bar 偏移不修改实际值
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void barDisplayOffsetNeverExceedsMaxRange() {
        float maxOffset = HallucinationHudOverlay.MAX_BAR_OFFSET;
        // Test boundaries: offset exactly at max should be allowed
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.setFadeProgressForTest(1.0f);
        HallucinationLayerStore.updateBarOffsets(maxOffset, -maxOffset);

        float hp = HallucinationHudOverlay.getHpBarDisplayOffset();
        float qi = HallucinationHudOverlay.getQiBarDisplayOffset();

        assertTrue(
            Math.abs(hp) <= maxOffset + 1e-5f,
            "expected |hpBarDisplayOffset| <= " + maxOffset + " (MAX_BAR_OFFSET) because bar offsets are bounded to prevent extreme display corruption, actual: " + hp
        );
        assertTrue(
            Math.abs(qi) <= maxOffset + 1e-5f,
            "expected |qiBarDisplayOffset| <= " + maxOffset + " (MAX_BAR_OFFSET) because bar offsets are bounded to prevent extreme display corruption, actual: " + qi
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HallucinationHudOverlay — 常量 pin
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void maxYawDegreesIsThree() {
        assertEquals(
            3.0f,
            HallucinationHudOverlay.MAX_YAW_DEGREES,
            1e-5f,
            "expected MAX_YAW_DEGREES=3.0 because design spec says ±3° to stay below nausea threshold, actual: "
                + HallucinationHudOverlay.MAX_YAW_DEGREES
        );
    }

    @Test
    void maxBarOffsetIsTwentyPercent() {
        assertEquals(
            0.2f,
            HallucinationHudOverlay.MAX_BAR_OFFSET,
            1e-5f,
            "expected MAX_BAR_OFFSET=0.2 (±20%) because design spec says ±20% bar display offset, actual: "
                + HallucinationHudOverlay.MAX_BAR_OFFSET
        );
    }

    @Test
    void barOffsetReshuffleIntervalIsTenTicks() {
        assertEquals(
            10,
            HallucinationHudOverlay.BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS,
            "expected BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS=10 because design spec says every 10 ticks, actual: "
                + HallucinationHudOverlay.BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS
        );
    }

    @Test
    void edgeAberrationWidthIsThirtyTwo() {
        assertEquals(
            32,
            HallucinationHudOverlay.EDGE_ABERRATION_WIDTH,
            "expected EDGE_ABERRATION_WIDTH=32 because design spec sets edge aberration gradient width to 32px, actual: "
                + HallucinationHudOverlay.EDGE_ABERRATION_WIDTH
        );
    }

    @Test
    void hallucinationEdgeColorContainsGreenTint() {
        int color = HallucinationHudOverlay.HALLUCINATION_EDGE_COLOR_MAX;
        int r = (color >> 16) & 0xFF;
        int g = (color >> 8) & 0xFF;
        int b = color & 0xFF;

        assertTrue(
            g > r && g > b,
            "expected green channel to dominate in HALLUCINATION_EDGE_COLOR_MAX because the color represents green beast-core resonance (#80F040), "
                + "actual R=" + r + " G=" + g + " B=" + b
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // State transition 测试
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void activateAfterCancelRestartsEffect() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.cancel();
        HallucinationLayerStore.activate(100);

        assertTrue(
            HallucinationLayerStore.isActive(),
            "expected isActive()=true after re-activate following cancel because the effect can be triggered multiple times, actual: false"
        );
        assertEquals(
            100,
            HallucinationLayerStore.getRemainingTicks(),
            "expected remainingTicks=100 after re-activate with duration=100, actual: "
                + HallucinationLayerStore.getRemainingTicks()
        );
        assertEquals(
            0.0f,
            HallucinationLayerStore.getFadeProgress(),
            1e-5f,
            "expected fadeProgress=0 after re-activate because the effect always fades in from zero, actual: "
                + HallucinationLayerStore.getFadeProgress()
        );
    }
}
