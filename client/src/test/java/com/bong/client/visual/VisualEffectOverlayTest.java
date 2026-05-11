package com.bong.client.visual;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudTextHelper;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 覆盖 plan-vfx-v1 §3.2 首批 4 个 HUD 叠色效果：
 *   BLOOD_MOON / DEMONIC_FOG / ENLIGHTENMENT_FLASH / TRIBULATION_PRESSURE。
 */
public class VisualEffectOverlayTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void bloodMoonProducesRedScreenTintWithinCappedAlpha() {
        List<HudRenderCommand> commands = buildCommandsAt("blood_moon", 1.0, 10_000L, 500L, 500L);

        assertEquals(1, commands.size());
        HudRenderCommand command = commands.get(0);
        assertTrue(command.isScreenTint());
        assertFalse(command.isEdgeVignette());
        assertEquals(VisualEffectProfile.BLOOD_MOON.baseColor(), command.color() & 0x00FFFFFF);
        int alpha = (command.color() >>> 24) & 0xFF;
        assertTrue(alpha > 0 && alpha <= VisualEffectProfile.BLOOD_MOON.maxAlpha());
    }

    @Test
    void demonicFogProducesEdgeVignetteNotScreenTint() {
        List<HudRenderCommand> commands = buildCommandsAt("demonic_fog", 1.0, 10_000L, 500L, 500L);

        assertEquals(1, commands.size());
        HudRenderCommand command = commands.get(0);
        assertTrue(command.isEdgeVignette());
        assertFalse(command.isScreenTint());
        assertEquals(VisualEffectProfile.DEMONIC_FOG.baseColor(), command.color() & 0x00FFFFFF);
    }

    @Test
    void enlightenmentFlashFadesSharply() {
        VisualEffectState acceptedState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("enlightenment_flash", 1.0, 1_500L, 0L),
            0L,
            true
        );

        HudRenderCommand startCommand = VisualEffectPlanner.buildCommands(
            acceptedState, 0L, FIXED_WIDTH, 220, 320, 180, true
        ).get(0);
        HudRenderCommand midCommand = VisualEffectPlanner.buildCommands(
            acceptedState, 750L, FIXED_WIDTH, 220, 320, 180, true
        ).get(0);
        List<HudRenderCommand> expired = VisualEffectPlanner.buildCommands(
            acceptedState, 1_600L, FIXED_WIDTH, 220, 320, 180, true
        );

        assertTrue(startCommand.isScreenTint());
        assertEquals(VisualEffectProfile.ENLIGHTENMENT_FLASH.baseColor(), startCommand.color() & 0x00FFFFFF);
        int startAlpha = (startCommand.color() >>> 24) & 0xFF;
        int midAlpha = (midCommand.color() >>> 24) & 0xFF;
        assertTrue(startAlpha > midAlpha, "启动瞬间 alpha 应高于衰减中途");
        assertTrue(expired.isEmpty(), "超过 duration 后不再发命令");
    }

    @Test
    void tribulationPressureEmitsBothScreenTintAndEdgeVignette() {
        List<HudRenderCommand> commands = buildCommandsAt("tribulation_pressure", 1.0, 8_000L, 500L, 500L);

        assertEquals(2, commands.size());
        HudRenderCommand tint = commands.get(0);
        HudRenderCommand vignette = commands.get(1);
        assertTrue(tint.isScreenTint());
        assertTrue(vignette.isEdgeVignette());
        // 同一 profile 触发，颜色应一致
        assertEquals(tint.color(), vignette.color());
        assertEquals(VisualEffectProfile.TRIBULATION_PRESSURE.baseColor(), tint.color() & 0x00FFFFFF);
    }

    @Test
    void armorBreakFlashProducesRedScreenTint() {
        List<HudRenderCommand> commands = buildCommandsAt("armor_break_flash", 1.0, 300L, 0L, 0L);

        assertEquals(1, commands.size());
        HudRenderCommand command = commands.get(0);
        assertTrue(command.isScreenTint());
        assertEquals(VisualEffectProfile.ARMOR_BREAK_FLASH.baseColor(), command.color() & 0x00FFFFFF);
    }

    @Test
    void newEffectsHonorRetriggerWindowLikeExistingOnes() {
        VisualEffectState first = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("blood_moon", 1.0, 30_000L, 0L),
            0L,
            true
        );
        // 重触发窗口内（BLOOD_MOON.retriggerWindowMillis = 3_000L）
        VisualEffectState withinWindow = VisualEffectController.acceptIncoming(
            first,
            VisualEffectState.create("blood_moon", 0.5, 30_000L, 1_000L),
            1_000L,
            true
        );
        // 窗口外
        VisualEffectState afterWindow = VisualEffectController.acceptIncoming(
            first,
            VisualEffectState.create("blood_moon", 0.5, 30_000L, 4_000L),
            4_000L,
            true
        );

        assertEquals(0L, withinWindow.startedAtMillis(), "窗口内应保留原 state 不刷新");
        assertEquals(4_000L, afterWindow.startedAtMillis(), "窗口外应接受新触发");
    }

    @Test
    void overlayQuadRendererHandlesZeroAlphaGracefully() {
        // 单独覆盖渲染端的空快照分支（不启动 MC 渲染器，只验证 alpha 分支）
        int alpha = OverlayQuadRenderer.alphaOf(0x80FF0000);
        assertEquals(0x80, alpha);
        assertEquals(0, OverlayQuadRenderer.alphaOf(0x00FF0000));
    }

    private static List<HudRenderCommand> buildCommandsAt(
        String wireName,
        double intensity,
        long durationMillis,
        long startMillis,
        long nowMillis
    ) {
        VisualEffectState acceptedState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create(wireName, intensity, durationMillis, startMillis),
            startMillis,
            true
        );
        return VisualEffectPlanner.buildCommands(
            acceptedState,
            nowMillis,
            FIXED_WIDTH,
            220,
            320,
            180,
            true
        );
    }
}
