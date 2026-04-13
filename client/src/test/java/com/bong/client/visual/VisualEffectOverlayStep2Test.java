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
 * 覆盖 plan-vfx-v1 §3.2 第二批 HUD 叠色 + §4 灵压 jitter：
 *   MEDITATION_CALM / POISON_TINT / FROSTBITE / NEAR_DEATH_VIGNETTE / PRESSURE_JITTER。
 */
public class VisualEffectOverlayStep2Test {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void meditationCalmEmitsTintAndVignetteAtLowAlpha() {
        List<HudRenderCommand> commands = buildCommandsAt("meditation_calm", 1.0, 10_000L, 500L, 500L);
        assertEquals(2, commands.size());
        HudRenderCommand tint = commands.get(0);
        HudRenderCommand vignette = commands.get(1);
        assertTrue(tint.isScreenTint());
        assertTrue(vignette.isEdgeVignette());
        assertEquals(VisualEffectProfile.MEDITATION_CALM.baseColor(), tint.color() & 0x00FFFFFF);
        int alpha = (tint.color() >>> 24) & 0xFF;
        // 低 alpha cap 72，满强度刚开始 alpha 应 <= 72
        assertTrue(alpha > 0 && alpha <= VisualEffectProfile.MEDITATION_CALM.maxAlpha());
    }

    @Test
    void poisonTintSingleLayerEdgeVignetteAbsent() {
        List<HudRenderCommand> commands = buildCommandsAt("poison_tint", 1.0, 10_000L, 500L, 500L);
        assertEquals(1, commands.size());
        HudRenderCommand command = commands.get(0);
        assertTrue(command.isScreenTint());
        assertFalse(command.isEdgeVignette());
        assertEquals(VisualEffectProfile.POISON_TINT.baseColor(), command.color() & 0x00FFFFFF);
    }

    @Test
    void frostbiteTintPlusVignetteColorConsistent() {
        List<HudRenderCommand> commands = buildCommandsAt("frostbite", 1.0, 10_000L, 500L, 500L);
        assertEquals(2, commands.size());
        HudRenderCommand tint = commands.get(0);
        HudRenderCommand vignette = commands.get(1);
        assertTrue(tint.isScreenTint());
        assertTrue(vignette.isEdgeVignette());
        // 同 profile 的两层应共享颜色
        assertEquals(tint.color(), vignette.color());
        assertEquals(VisualEffectProfile.FROSTBITE.baseColor(), tint.color() & 0x00FFFFFF);
    }

    @Test
    void nearDeathVignetteOnlyNoScreenTint() {
        List<HudRenderCommand> commands = buildCommandsAt("near_death_vignette", 1.0, 20_000L, 500L, 500L);
        assertEquals(1, commands.size());
        HudRenderCommand command = commands.get(0);
        assertTrue(command.isEdgeVignette());
        assertFalse(command.isScreenTint());
        // 纯黑
        assertEquals(0x000000, command.color() & 0x00FFFFFF);
        int alpha = (command.color() >>> 24) & 0xFF;
        assertTrue(alpha > 0 && alpha <= VisualEffectProfile.NEAR_DEATH_VIGNETTE.maxAlpha());
    }

    @Test
    void pressureJitterEmitsNoHudCommands() {
        // 灵压晃动纯相机效果，HUD 层应返回空列表
        List<HudRenderCommand> commands = buildCommandsAt("pressure_jitter", 1.0, 5_000L, 0L, 100L);
        assertTrue(commands.isEmpty(), "PRESSURE_JITTER 不应发 HUD 命令");
    }

    @Test
    void newOverlaysFadeWithIntensityLikeExisting() {
        // 验证新 overlay 跟现有 profile 一样随 scaledIntensityAt 线性衰减
        VisualEffectState state = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("poison_tint", 1.0, 1_000L, 0L),
            0L,
            true
        );
        HudRenderCommand start = VisualEffectPlanner.buildCommands(
            state, 0L, FIXED_WIDTH, 220, 320, 180, true
        ).get(0);
        HudRenderCommand mid = VisualEffectPlanner.buildCommands(
            state, 500L, FIXED_WIDTH, 220, 320, 180, true
        ).get(0);
        List<HudRenderCommand> expired = VisualEffectPlanner.buildCommands(
            state, 1_100L, FIXED_WIDTH, 220, 320, 180, true
        );

        int startAlpha = (start.color() >>> 24) & 0xFF;
        int midAlpha = (mid.color() >>> 24) & 0xFF;
        assertTrue(startAlpha > midAlpha, "开始 alpha " + startAlpha + " 应 > 中途 " + midAlpha);
        assertTrue(expired.isEmpty(), "过期后 Planner 应返回空列表");
    }

    @Test
    void newEffectsHonorRetriggerWindow() {
        VisualEffectState first = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("frostbite", 1.0, 30_000L, 0L),
            0L,
            true
        );
        // FROSTBITE.retriggerWindowMillis = 1_500L，窗口内再触发应被忽略
        VisualEffectState withinWindow = VisualEffectController.acceptIncoming(
            first,
            VisualEffectState.create("frostbite", 0.3, 30_000L, 500L),
            500L,
            true
        );
        VisualEffectState afterWindow = VisualEffectController.acceptIncoming(
            first,
            VisualEffectState.create("frostbite", 0.3, 30_000L, 2_000L),
            2_000L,
            true
        );
        assertEquals(0L, withinWindow.startedAtMillis(), "窗口内保持原 state 不刷新");
        assertEquals(2_000L, afterWindow.startedAtMillis(), "窗口外应接受新触发");
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
