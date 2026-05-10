package com.bong.client.ui;

import net.minecraft.client.gui.screen.Screen;

import java.util.Objects;

public record TransitionConfig(
    Class<? extends Screen> screenClass,
    ScreenTransition.Type openTransition,
    int openDurationMs,
    ScreenTransition.Type closeTransition,
    int closeDurationMs,
    ScreenTransition.Easing easing,
    OverlayStyle overlayStyle,
    boolean externalCinematic
) {
    public static final TransitionConfig DEFAULT_FADE_200MS = new TransitionConfig(
        Screen.class,
        ScreenTransition.Type.FADE,
        200,
        ScreenTransition.Type.FADE,
        200,
        ScreenTransition.Easing.EASE_OUT_CUBIC,
        OverlayStyle.NONE,
        false
    );

    public TransitionConfig {
        screenClass = Objects.requireNonNull(screenClass, "screenClass");
        openTransition = openTransition == null ? ScreenTransition.Type.FADE : openTransition;
        closeTransition = closeTransition == null ? ScreenTransition.Type.FADE : closeTransition;
        easing = easing == null ? ScreenTransition.Easing.EASE_OUT_CUBIC : easing;
        overlayStyle = overlayStyle == null ? OverlayStyle.NONE : overlayStyle;
        openDurationMs = ScreenTransition.clampDuration(openTransition, openDurationMs);
        closeDurationMs = ScreenTransition.clampDuration(closeTransition, closeDurationMs);
    }

    public static TransitionConfig of(
        Class<? extends Screen> screenClass,
        ScreenTransition.Type openTransition,
        int openDurationMs,
        ScreenTransition.Type closeTransition,
        int closeDurationMs
    ) {
        return new TransitionConfig(
            screenClass,
            openTransition,
            openDurationMs,
            closeTransition,
            closeDurationMs,
            ScreenTransition.Easing.EASE_OUT_CUBIC,
            OverlayStyle.NONE,
            false
        );
    }

    public TransitionSpec openSpec() {
        return new TransitionSpec(openTransition, openDurationMs, easing, overlayStyle, externalCinematic);
    }

    public TransitionSpec closeSpec() {
        return new TransitionSpec(closeTransition, closeDurationMs, easing, overlayStyle, externalCinematic);
    }

    public enum OverlayStyle {
        NONE,
        FOG,
        VIGNETTE,
        PURPLE_TINT
    }

    public record TransitionSpec(
        ScreenTransition.Type type,
        int durationMs,
        ScreenTransition.Easing easing,
        OverlayStyle overlayStyle,
        boolean externalCinematic
    ) {
        public TransitionSpec {
            type = type == null ? ScreenTransition.Type.NONE : type;
            easing = easing == null ? ScreenTransition.Easing.EASE_OUT_CUBIC : easing;
            overlayStyle = overlayStyle == null ? OverlayStyle.NONE : overlayStyle;
            durationMs = ScreenTransition.clampDuration(type, durationMs);
        }

        public boolean animates() {
            return !externalCinematic && type != ScreenTransition.Type.NONE && durationMs > 0;
        }
    }
}
