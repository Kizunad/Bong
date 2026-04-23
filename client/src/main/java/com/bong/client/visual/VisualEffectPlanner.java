package com.bong.client.visual;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;
import com.bong.client.state.VisualEffectState;

import java.util.List;

public final class VisualEffectPlanner {
    private static final int DEFAULT_SCREEN_WIDTH = 320;
    private static final int DEFAULT_SCREEN_HEIGHT = 180;
    private static final int WARNING_TEXT_Y_DIVISOR = 3;
    private static final int DECREE_TEXT_Y_DIVISOR = 4;

    private VisualEffectPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        VisualEffectState visualEffectState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        boolean enabled
    ) {
        VisualEffectState safeVisualEffectState = visualEffectState == null ? VisualEffectState.none() : visualEffectState;
        if (!enabled || safeVisualEffectState.isEmpty() || !safeVisualEffectState.isActiveAt(nowMillis)) {
            return List.of();
        }

        VisualEffectProfile profile = VisualEffectProfile.from(safeVisualEffectState);
        if (profile == null) {
            return List.of();
        }

        int alpha = alphaAt(safeVisualEffectState, nowMillis, profile);
        if (alpha <= 0) {
            return List.of();
        }

        return switch (profile) {
            case SYSTEM_WARNING -> buildWarningCommands(
                safeVisualEffectState,
                nowMillis,
                widthMeasurer,
                maxTextWidth,
                screenWidth,
                screenHeight,
                alpha,
                profile
            );
            case PERCEPTION -> List.of(HudRenderCommand.screenTint(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
            case ERA_DECREE -> buildCenteredTextCommands(
                widthMeasurer,
                maxTextWidth,
                screenWidth,
                screenHeight,
                alpha,
                profile,
                0,
                DECREE_TEXT_Y_DIVISOR
            );
            case BLOOD_MOON, ENLIGHTENMENT_FLASH, WEAPON_BREAK_FLASH -> List.of(HudRenderCommand.screenTint(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
            case DEMONIC_FOG -> List.of(HudRenderCommand.edgeVignette(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
            case TRIBULATION_PRESSURE -> List.of(
                HudRenderCommand.screenTint(
                    HudRenderLayer.VISUAL,
                    HudTextHelper.withAlpha(profile.baseColor(), alpha)
                ),
                HudRenderCommand.edgeVignette(
                    HudRenderLayer.VISUAL,
                    HudTextHelper.withAlpha(profile.baseColor(), alpha)
                )
            );
            // FOV 类效果纯通过 MixinGameRenderer 修改相机 FOV，HUD 层无需画任何东西
            case FOV_ZOOM_IN, FOV_STRETCH -> List.of();
            // 天劫仰视纯通过 MixinCamera 修改 pitch，HUD 层同样无需画任何东西
            case TRIBULATION_LOOK_UP -> List.of();
            // 入定淡青：淡色 tint + 边缘渐暗，营造"入定"的收敛感
            case MEDITATION_CALM -> List.of(
                HudRenderCommand.screenTint(
                    HudRenderLayer.VISUAL,
                    HudTextHelper.withAlpha(profile.baseColor(), alpha)
                ),
                HudRenderCommand.edgeVignette(
                    HudRenderLayer.VISUAL,
                    HudTextHelper.withAlpha(profile.baseColor(), alpha)
                )
            );
            // 中毒酸绿：单层 tint 即可，边缘不做装饰免喧宾夺主
            case POISON_TINT -> List.of(HudRenderCommand.screenTint(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
            // 寒毒冰蓝：tint + vignette 近似结霜边缘（结霜纹理留待资产）
            case FROSTBITE -> List.of(
                HudRenderCommand.screenTint(
                    HudRenderLayer.VISUAL,
                    HudTextHelper.withAlpha(profile.baseColor(), alpha)
                ),
                HudRenderCommand.edgeVignette(
                    HudRenderLayer.VISUAL,
                    HudTextHelper.withAlpha(profile.baseColor(), alpha)
                )
            );
            // 濒死：只画黑色 vignette，不做 tint 以免误读为"视野变黑"全屏
            case NEAR_DEATH_VIGNETTE -> List.of(HudRenderCommand.edgeVignette(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
            // 灵压晃动：纯相机抖动（低幅低频），走 MixinCamera+CameraShakeOffsets 同一管线
            case PRESSURE_JITTER -> List.of();
            // 受创后退：纯相机位移，走 MixinCamera TAIL 注入的 moveBy，HUD 层无输出
            case HIT_PUSHBACK -> List.of();
            // 水墨边框：纯贴图 overlay，alpha 跟随 scaledIntensity 淡入淡出
            case MEDITATION_INK_WASH -> List.of(HudRenderCommand.edgeInkWash(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
        };
    }

    static int alphaAt(VisualEffectState visualEffectState, long nowMillis, VisualEffectProfile profile) {
        return HudTextHelper.clampAlpha((int) Math.round(visualEffectState.scaledIntensityAt(nowMillis) * profile.maxAlpha()));
    }

    static int shakeOffset(VisualEffectState visualEffectState, long nowMillis) {
        long elapsedMillis = Math.max(0L, nowMillis - Math.max(0L, visualEffectState.startedAtMillis()));
        int amplitude = Math.max(1, (int) Math.round(visualEffectState.scaledIntensityAt(nowMillis) * 8.0));
        int reducedAmplitude = Math.max(1, amplitude / 2);
        return switch ((int) ((elapsedMillis / 75L) % 4L)) {
            case 0 -> amplitude;
            case 1 -> -amplitude;
            case 2 -> reducedAmplitude;
            default -> -reducedAmplitude;
        };
    }

    private static List<HudRenderCommand> buildWarningCommands(
        VisualEffectState visualEffectState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        int alpha,
        VisualEffectProfile profile
    ) {
        return buildCenteredTextCommands(
            widthMeasurer,
            maxTextWidth,
            screenWidth,
            screenHeight,
            alpha,
            profile,
            shakeOffset(visualEffectState, nowMillis),
            WARNING_TEXT_Y_DIVISOR
        );
    }

    private static List<HudRenderCommand> buildCenteredTextCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        int alpha,
        VisualEffectProfile profile,
        int xOffset,
        int yDivisor
    ) {
        if (widthMeasurer == null || profile.overlayLabel() == null || profile.overlayLabel().isBlank()) {
            return List.of();
        }

        int resolvedScreenWidth = normalizeScreenWidth(screenWidth);
        int resolvedScreenHeight = normalizeScreenHeight(screenHeight);
        int resolvedMaxTextWidth = normalizeMaxTextWidth(maxTextWidth, resolvedScreenWidth);
        String clippedLabel = HudTextHelper.clipToWidth(profile.overlayLabel(), resolvedMaxTextWidth, widthMeasurer);
        if (clippedLabel.isEmpty()) {
            return List.of();
        }

        int textWidth = Math.max(0, widthMeasurer.measure(clippedLabel));
        int centeredX = Math.max(0, (resolvedScreenWidth - textWidth) / 2);
        int maxX = Math.max(0, resolvedScreenWidth - textWidth);
        int x = Math.max(0, Math.min(maxX, centeredX + xOffset));
        int y = Math.max(18, resolvedScreenHeight / Math.max(1, yDivisor));
        return List.of(HudRenderCommand.text(
            HudRenderLayer.VISUAL,
            clippedLabel,
            x,
            y,
            HudTextHelper.withAlpha(profile.baseColor(), alpha)
        ));
    }

    private static int normalizeScreenWidth(int screenWidth) {
        return screenWidth > 0 ? screenWidth : DEFAULT_SCREEN_WIDTH;
    }

    private static int normalizeScreenHeight(int screenHeight) {
        return screenHeight > 0 ? screenHeight : DEFAULT_SCREEN_HEIGHT;
    }

    private static int normalizeMaxTextWidth(int maxTextWidth, int screenWidth) {
        if (maxTextWidth > 0) {
            return maxTextWidth;
        }
        return Math.max(80, screenWidth - 24);
    }
}
