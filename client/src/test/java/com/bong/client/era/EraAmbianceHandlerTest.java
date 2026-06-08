package com.bong.client.era;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-era-state-v1 P3 — 时代天象 client handler 单测。
 *
 * <p>覆盖：
 * <ul>
 *   <li>三时代 JSON 解析（happy path + 正确字段值）</li>
 *   <li>reset/unknown 时代解析</li>
 *   <li>v 版本不符时返回 null</li>
 *   <li>JSON 格式错误容错</li>
 *   <li>hex 颜色解析（#RRGGBB / 无前缀 / 错误格式）</li>
 *   <li>EraAmbiancePayload 字段 pin（isReset / hasSkyTint / hasAmbientSound）</li>
 *   <li>EraAmbianceState 线性插值（tick 推进后 fog_delta / sky_tint 趋近目标）</li>
 *   <li>EraAmbianceState.blendRgb 边界（t=0/1/中间值）</li>
 *   <li>EraAmbianceState.reset() 清零状态</li>
 * </ul>
 */
class EraAmbianceHandlerTest {

    @BeforeEach
    void resetState() {
        EraAmbianceState.reset();
    }

    @AfterEach
    void cleanUpState() {
        EraAmbianceState.reset();
    }

    // ── JSON 解析 — 三时代 happy path ──────────────────────────────────────────

    @Test
    void handle_calamity_parses_correctly() {
        String json = """
            {
              "v": 1,
              "era_type": "calamity",
              "sky_tint_hex": "#4A1A1A",
              "fog_density_delta": 0.15,
              "ambient_sound_id": "ambient.weather.thunder",
              "transition_ticks": 1200
            }
            """;
        EraAmbiancePayload p = EraAmbianceHandler.handle(json);
        assertNotNull(p, "灾劫时代 JSON 解析不应返回 null");
        assertEquals("calamity", p.eraType(), "era_type 应为 calamity；实际=" + p.eraType());
        assertEquals(0x4A1A1A, p.skyTintRgb(),
            "sky_tint_hex=#4A1A1A 应解析为 0x4A1A1A；实际=" + Integer.toHexString(p.skyTintRgb()));
        assertTrue(Math.abs(p.fogDensityDelta() - 0.15) < 1e-9,
            "fog_density_delta 应为 0.15；实际=" + p.fogDensityDelta());
        assertEquals("ambient.weather.thunder", p.ambientSoundId(),
            "ambient_sound_id 应为 ambient.weather.thunder");
        assertEquals(1200, p.transitionTicks(), "transition_ticks 应为 1200");
    }

    @Test
    void handle_change_parses_correctly() {
        String json = """
            {
              "v": 1,
              "era_type": "change",
              "sky_tint_hex": "#1A3A4A",
              "fog_density_delta": 0.05,
              "ambient_sound_id": "block.water.ambient",
              "transition_ticks": 1200
            }
            """;
        EraAmbiancePayload p = EraAmbianceHandler.handle(json);
        assertNotNull(p, "变化时代 JSON 解析不应返回 null");
        assertEquals("change", p.eraType());
        assertEquals(0x1A3A4A, p.skyTintRgb(),
            "sky_tint_hex=#1A3A4A 应解析为 0x1A3A4A");
        assertTrue(Math.abs(p.fogDensityDelta() - 0.05) < 1e-9);
        assertEquals("block.water.ambient", p.ambientSoundId());
    }

    @Test
    void handle_deduction_parses_correctly() {
        String json = """
            {
              "v": 1,
              "era_type": "deduction",
              "sky_tint_hex": "#2A2A3A",
              "fog_density_delta": -0.05,
              "ambient_sound_id": "entity.enderman.ambient",
              "transition_ticks": 1200
            }
            """;
        EraAmbiancePayload p = EraAmbianceHandler.handle(json);
        assertNotNull(p, "演绎时代 JSON 解析不应返回 null");
        assertEquals("deduction", p.eraType());
        assertEquals(0x2A2A3A, p.skyTintRgb());
        assertTrue(Math.abs(p.fogDensityDelta() - (-0.05)) < 1e-9,
            "演绎时代 fog_density_delta 应为 -0.05；实际=" + p.fogDensityDelta());
        assertEquals("entity.enderman.ambient", p.ambientSoundId());
    }

    // ── reset/unknown 时代 ──────────────────────────────────────────────────────

    @Test
    void handle_unknown_era_is_reset() {
        String json = """
            {
              "v": 1,
              "era_type": "unknown",
              "sky_tint_hex": "#000000",
              "fog_density_delta": 0.0,
              "ambient_sound_id": "",
              "transition_ticks": 1200
            }
            """;
        EraAmbiancePayload p = EraAmbianceHandler.handle(json);
        assertNotNull(p, "Unknown 时代 JSON 应成功解析");
        assertTrue(p.isReset(), "era_type=unknown 应识别为 reset 态；实际 isReset=" + p.isReset());
        assertFalse(p.hasAmbientSound(), "reset 态 ambient_sound_id 为空，hasAmbientSound 应为 false");
    }

    // ── 版本不符 ────────────────────────────────────────────────────────────────

    @Test
    void handle_wrong_version_returns_null() {
        String json = """
            {"v": 2, "era_type": "calamity", "sky_tint_hex": "#4A1A1A",
             "fog_density_delta": 0.15, "ambient_sound_id": "", "transition_ticks": 1200}
            """;
        EraAmbiancePayload p = EraAmbianceHandler.handle(json);
        assertNull(p, "协议版本 v=2 不支持，应返回 null（不应接受未来版本的 payload）");
    }

    // ── 格式错误容错 ─────────────────────────────────────────────────────────────

    @Test
    void handle_blank_json_returns_null() {
        assertNull(EraAmbianceHandler.handle(""), "空字符串不应崩溃，应返回 null");
        assertNull(EraAmbianceHandler.handle("   "), "空白字符串不应崩溃，应返回 null");
        assertNull(EraAmbianceHandler.handle(null), "null 不应崩溃，应返回 null");
    }

    @Test
    void handle_invalid_json_returns_null() {
        assertNull(EraAmbianceHandler.handle("not json at all"),
            "非 JSON 字符串不应崩溃，应返回 null（容错）");
        assertNull(EraAmbianceHandler.handle("{broken json"),
            "损坏的 JSON 应返回 null");
    }

    // ── hex 颜色解析 ─────────────────────────────────────────────────────────────

    @Test
    void parse_hex_color_with_hash_prefix() {
        assertEquals(0x4A1A1A, EraAmbianceHandler.parseHexColorSafe("#4A1A1A"),
            "#4A1A1A 应解析为 0x4A1A1A（含前缀#）");
    }

    @Test
    void parse_hex_color_without_hash_prefix() {
        assertEquals(0x1A3A4A, EraAmbianceHandler.parseHexColorSafe("1A3A4A"),
            "1A3A4A 无前缀也应正确解析");
    }

    @Test
    void parse_hex_color_all_zeros() {
        assertEquals(0, EraAmbianceHandler.parseHexColorSafe("#000000"),
            "#000000 应解析为 0（baseline/reset 颜色）");
    }

    @Test
    void parse_hex_color_invalid_returns_zero() {
        assertEquals(0, EraAmbianceHandler.parseHexColorSafe("#GGGGGG"),
            "非法 hex 字符应 fallback 到 0；实际=" +
                Integer.toHexString(EraAmbianceHandler.parseHexColorSafe("#GGGGGG")));
        assertEquals(0, EraAmbianceHandler.parseHexColorSafe(""),
            "空字符串 fallback 到 0");
        assertEquals(0, EraAmbianceHandler.parseHexColorSafe(null),
            "null fallback 到 0");
    }

    @Test
    void parse_hex_color_8digit_strips_alpha() {
        // AARRGGBB 格式：去掉前 2 位 AA
        assertEquals(0x4A1A1A, EraAmbianceHandler.parseHexColorSafe("FF4A1A1A"),
            "8 位 AARRGGBB 格式应去掉 alpha 取 RRGGBB");
    }

    // ── EraAmbiancePayload 字段 ───────────────────────────────────────────────────

    @Test
    void payload_has_sky_tint_true_when_nonzero() {
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x4A1A1A, 0.15, "sound", 1200);
        assertTrue(p.hasSkyTint(), "skyTintRgb=0x4A1A1A 时 hasSkyTint 应为 true");
    }

    @Test
    void payload_has_sky_tint_false_when_zero() {
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x000000, 0.15, "sound", 1200);
        assertFalse(p.hasSkyTint(), "skyTintRgb=0x000000（baseline）时 hasSkyTint 应为 false");
    }

    @Test
    void payload_has_ambient_sound_true_when_nonempty() {
        EraAmbiancePayload p = new EraAmbiancePayload("change", 0x1A3A4A, 0.05, "block.water.ambient", 1200);
        assertTrue(p.hasAmbientSound(), "非空 ambient_sound_id 时 hasAmbientSound 应为 true");
    }

    @Test
    void payload_has_ambient_sound_false_when_empty() {
        EraAmbiancePayload p = new EraAmbiancePayload("unknown", 0, 0.0, "", 1200);
        assertFalse(p.hasAmbientSound(), "空 ambient_sound_id 时 hasAmbientSound 应为 false");
    }

    // ── EraAmbianceState 线性插值 ─────────────────────────────────────────────────

    @Test
    void state_fog_delta_at_tick_zero_is_zero() {
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x4A1A1A, 0.15, "sound", 1200);
        EraAmbianceState.accept(p);
        // tick 0：插值 t=0，fog_delta = 0.15 * 0.0 = 0.0
        double delta = EraAmbianceState.interpolateFogDensityDelta();
        assertTrue(Math.abs(delta) < 1e-9,
            "tick=0 时 fog_delta 应为 0（插值起点）；实际=" + delta);
    }

    @Test
    void state_fog_delta_at_half_transition_is_half_target() {
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x4A1A1A, 0.2, "sound", 100);
        EraAmbianceState.accept(p);
        // 推进 50 tick（50/100 = 0.5）
        for (int i = 0; i < 50; i++) {
            EraAmbianceState.tick();
        }
        double delta = EraAmbianceState.interpolateFogDensityDelta();
        // 期望 0.2 * 0.5 = 0.1（±0.01 容差）
        assertTrue(Math.abs(delta - 0.1) < 0.01,
            "50/100 tick 后 fog_delta 应约为 0.1（目标 0.2 的一半）；实际=" + delta);
    }

    @Test
    void state_fog_delta_after_full_transition_is_full_target() {
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x4A1A1A, 0.15, "sound", 10);
        EraAmbianceState.accept(p);
        // 推进 > transitionTicks（10 tick）
        for (int i = 0; i < 15; i++) {
            EraAmbianceState.tick();
        }
        double delta = EraAmbianceState.interpolateFogDensityDelta();
        assertTrue(Math.abs(delta - 0.15) < 1e-9,
            "transitionTicks 完成后 fog_delta 应等于目标值 0.15；实际=" + delta);
    }

    @Test
    void state_fog_delta_reset_era_returns_zero() {
        EraAmbiancePayload p = new EraAmbiancePayload("unknown", 0, 0.0, "", 1200);
        EraAmbianceState.accept(p);
        for (int i = 0; i < 100; i++) {
            EraAmbianceState.tick();
        }
        double delta = EraAmbianceState.interpolateFogDensityDelta();
        assertTrue(Math.abs(delta) < 1e-9,
            "reset 时代 fog_delta 应始终为 0；实际=" + delta);
    }

    @Test
    void state_sky_tint_at_full_transition_matches_target() {
        int targetRgb = 0x4A1A1A;
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", targetRgb, 0.15, "sound", 5);
        EraAmbianceState.accept(p);
        // 完成过渡
        for (int i = 0; i < 10; i++) {
            EraAmbianceState.tick();
        }
        int interpolated = EraAmbianceState.interpolateSkyTintRgb(0x000000);
        assertEquals(targetRgb, interpolated,
            "过渡完成后 interpolateSkyTintRgb 应等于目标值 0x4A1A1A；实际=" +
                Integer.toHexString(interpolated));
    }

    @Test
    void state_sky_tint_baseline_zero_no_modification() {
        // skyTintRgb=0x000000（baseline）时不修改 baseRgb
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x000000, 0.15, "sound", 1200);
        EraAmbianceState.accept(p);
        for (int i = 0; i < 1200; i++) {
            EraAmbianceState.tick();
        }
        int baseRgb = 0xB0C4DE; // zone profile 原值
        int interpolated = EraAmbianceState.interpolateSkyTintRgb(baseRgb);
        assertEquals(baseRgb, interpolated,
            "skyTintRgb=0x000000 时不应修改 baseRgb；期望=" +
                Integer.toHexString(baseRgb) + "，实际=" + Integer.toHexString(interpolated));
    }

    @Test
    void state_reset_clears_all() {
        EraAmbiancePayload p = new EraAmbiancePayload("calamity", 0x4A1A1A, 0.15, "sound", 10);
        EraAmbianceState.accept(p);
        for (int i = 0; i < 10; i++) {
            EraAmbianceState.tick();
        }
        EraAmbianceState.reset();
        assertNull(EraAmbianceState.current(), "reset 后 current() 应为 null");
        assertTrue(Math.abs(EraAmbianceState.interpolateFogDensityDelta()) < 1e-9,
            "reset 后 fog_delta 应为 0");
    }

    // ── EraAmbianceState.blendRgb ────────────────────────────────────────────────

    @Test
    void blend_rgb_t0_returns_from() {
        int from = 0xB0C4DE;
        int to = 0x4A1A1A;
        int result = EraAmbianceState.blendRgb(from, to, 0.0);
        assertEquals(from, result,
            "t=0 时 blendRgb 应返回 from；期望=" + Integer.toHexString(from) +
                "，实际=" + Integer.toHexString(result));
    }

    @Test
    void blend_rgb_t1_returns_to() {
        int from = 0xB0C4DE;
        int to = 0x4A1A1A;
        int result = EraAmbianceState.blendRgb(from, to, 1.0);
        assertEquals(to, result,
            "t=1 时 blendRgb 应返回 to；期望=" + Integer.toHexString(to) +
                "，实际=" + Integer.toHexString(result));
    }

    @Test
    void blend_rgb_t_half_midpoint() {
        // from=0x000000, to=0x100000 → 50% = 0x080000
        int result = EraAmbianceState.blendRgb(0x000000, 0x100000, 0.5);
        int r = (result >> 16) & 0xFF;
        assertTrue(r >= 7 && r <= 9,
            "t=0.5 blendRgb R 通道应在 [7, 9]（round 浮动）；实际=" + r);
    }

    @Test
    void blend_rgb_clamped_outside_01() {
        // t=-1 应等同 t=0
        int from = 0x123456;
        int resultNeg = EraAmbianceState.blendRgb(from, 0xFFFFFF, -1.0);
        assertEquals(from, resultNeg,
            "t=-1 应 clamp 到 t=0，返回 from；实际=" + Integer.toHexString(resultNeg));
        // t=2 应等同 t=1
        int to = 0xFFFFFF;
        int resultOver = EraAmbianceState.blendRgb(from, to, 2.0);
        assertEquals(to, resultOver,
            "t=2 应 clamp 到 t=1，返回 to；实际=" + Integer.toHexString(resultOver));
    }

    // ── E2E：handle → accept → interpolate 完整链路 ────────────────────────────

    /**
     * E2E：server payload JSON → handle → EraAmbianceState → interpolate 验证完整链路通。
     * 对应 plan P3 要求"server emit EraChangedEvent → send_custom_payload → client handler 解析 JSON 成功"。
     */
    @Test
    void e2e_handle_then_interpolate_full_path() {
        // 模拟 server 发出的 Calamity 完整包
        String serverPayload = """
            {"v":1,"era_type":"calamity","sky_tint_hex":"#4A1A1A",
             "fog_density_delta":0.15,"ambient_sound_id":"ambient.weather.thunder",
             "transition_ticks":10}
            """;

        // 1. client handler 解析
        EraAmbiancePayload p = EraAmbianceHandler.handle(serverPayload);
        assertNotNull(p, "E2E：server payload 应成功解析；返回 null = 解析失败");

        // 2. state 已存储
        EraAmbiancePayload current = EraAmbianceState.current();
        assertNotNull(current, "E2E：EraAmbianceState.current() 应非 null（handler 已 accept）");
        assertEquals("calamity", current.eraType(), "E2E：current eraType 应为 calamity");

        // 3. 推进完渐变
        for (int i = 0; i < 15; i++) {
            EraAmbianceState.tick();
        }

        // 4. interpolate 达到目标值
        double fog = EraAmbianceState.interpolateFogDensityDelta();
        assertTrue(Math.abs(fog - 0.15) < 1e-9,
            "E2E：过渡完成后 fog_delta 应为 0.15；实际=" + fog);

        int skyTint = EraAmbianceState.interpolateSkyTintRgb(0x000000);
        assertEquals(0x4A1A1A, skyTint,
            "E2E：过渡完成后 skyTintRgb 应为 0x4A1A1A；实际=" + Integer.toHexString(skyTint));
    }
}
