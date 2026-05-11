package com.bong.client.hud;

import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;

public enum HudLayoutPreset {
    COMBAT(EnumSet.of(
        Widget.QI_RADAR,
        Widget.COMPASS,
        Widget.THREAT,
        Widget.MINI_BODY,
        Widget.BARS,
        Widget.TARGET,
        Widget.EVENT_STREAM,
        Widget.PROCESSING,
        Widget.CRITICAL
    )),
    EXPLORATION(EnumSet.of(
        Widget.COMPASS,
        Widget.QI_RADAR,
        Widget.ZONE,
        Widget.BARS,
        Widget.EVENT_STREAM,
        Widget.PROCESSING,
        Widget.BOTANY,
        Widget.CRITICAL
    )),
    CULTIVATION(EnumSet.of(
        Widget.QI_RADAR,
        Widget.BARS,
        Widget.EVENT_STREAM,
        Widget.PROCESSING,
        Widget.MERIDIAN,
        Widget.LINGTIAN,
        Widget.CRITICAL
    ));

    public static final long HIDE_MS = 200L;
    public static final long SHOW_MS = 300L;
    public static final long SHOW_DELAY_MS = 100L;

    private final EnumSet<Widget> defaultWidgets;

    HudLayoutPreset(EnumSet<Widget> defaultWidgets) {
        this.defaultWidgets = defaultWidgets;
    }

    public EnumSet<Widget> defaultWidgets() {
        return EnumSet.copyOf(defaultWidgets);
    }

    public static HudLayoutPreset fromMode(HudImmersionMode.Mode mode) {
        return switch (mode == null ? HudImmersionMode.Mode.PEACE : mode) {
            case COMBAT -> COMBAT;
            case CULTIVATION -> CULTIVATION;
            case PEACE -> EXPLORATION;
        };
    }

    public static List<HudRenderCommand> filter(
        List<HudRenderCommand> commands,
        HudImmersionMode.Mode mode,
        HudLayoutPreferenceStore.Density density,
        long nowMillis
    ) {
        List<HudRenderCommand> baselineFiltered = HudImmersionMode.filter(commands, mode);
        if (baselineFiltered.isEmpty()) {
            return baselineFiltered;
        }
        HudLayoutPreferenceStore.Density effectiveDensity =
            density == null ? HudLayoutPreferenceStore.Density.STANDARD : density;
        if (effectiveDensity == HudLayoutPreferenceStore.Density.MAXIMUM) {
            return baselineFiltered;
        }
        EnumSet<Widget> widgets = effectiveDensity == HudLayoutPreferenceStore.Density.MINIMAL
            ? EnumSet.of(Widget.ZONE, Widget.BARS, Widget.EVENT_STREAM, Widget.CRITICAL)
            : HudLayoutPreferenceStore.widgetsFor(fromMode(mode));
        List<HudRenderCommand> out = new ArrayList<>(baselineFiltered.size());
        for (HudRenderCommand command : baselineFiltered) {
            Widget widget = widgetFor(command.layer());
            if (widgets.contains(widget) || widget == Widget.ALWAYS) {
                out.add(applyPresetAlpha(command, widget, nowMillis));
            }
        }
        return List.copyOf(out);
    }

    public static double alphaForWidget(boolean showing, long elapsedMillis) {
        long elapsed = Math.max(0L, elapsedMillis);
        if (!showing) {
            return 1.0 - Math.min(1.0, elapsed / (double) HIDE_MS);
        }
        if (elapsed <= SHOW_DELAY_MS) {
            return 0.0;
        }
        return Math.min(1.0, (elapsed - SHOW_DELAY_MS) / (double) SHOW_MS);
    }

    private static HudRenderCommand applyPresetAlpha(HudRenderCommand command, Widget widget, long nowMillis) {
        if (command == null || widget == Widget.ALWAYS || widget == Widget.CRITICAL) {
            return command;
        }
        double alpha = alphaForWidget(true, HudImmersionMode.transitionElapsedMillis(nowMillis));
        return HudCommandAlpha.withAlpha(command, alpha);
    }

    static Widget widgetFor(HudRenderLayer layer) {
        if (layer == null) {
            return Widget.ALWAYS;
        }
        return switch (layer) {
            case BASELINE -> Widget.ALWAYS;
            case ZONE, HUD_VARIANT -> Widget.ZONE;
            case COMPASS -> Widget.COMPASS;
            case QI_RADAR -> Widget.QI_RADAR;
            case THREAT_INDICATOR, EDGE_FEEDBACK, NEAR_DEATH, TRIBULATION -> Widget.THREAT;
            case MINI_BODY, STAMINA_BAR, DERIVED_ATTR, STATUS_EFFECTS, POISON_TRAIT -> Widget.BARS;
            case QUICK_BAR, CAST_BAR, SPELL_VOLUME, CARRIER, JIEMAI_RING, VORTEX_CHARGE, VORTEX_COOLDOWN,
                VORTEX_BACKFIRE, VORTEX_TURBULENCE, DUGU_TAINT_WARNING, DUGU_TAINT_INDICATOR,
                DUGU_REVEAL_RISK, DUGU_SELF_CURE_PROGRESS, DUGU_SHROUD -> Widget.BARS;
            case TARGET_INFO -> Widget.TARGET;
            case EVENT_STREAM, TOAST -> Widget.EVENT_STREAM;
            case BOTANY -> Widget.BOTANY;
            case LINGTIAN_OVERLAY -> Widget.LINGTIAN;
            case PROCESSING_HUD, SEARCH_PROGRESS, TSY_EXTRACT, REALM_COLLAPSE -> Widget.PROCESSING;
            case MERIDIAN_OPEN -> Widget.MERIDIAN;
            case VISUAL, SPIRITUAL_SENSE, DAMAGE_FLOATER, FLIGHT_HUD, CONNECTION_STATUS -> Widget.CRITICAL;
            case YIDAO -> Widget.CRITICAL;
        };
    }

    public enum Widget {
        ALWAYS,
        ZONE,
        QI_RADAR,
        COMPASS,
        THREAT,
        MINI_BODY,
        BARS,
        TARGET,
        EVENT_STREAM,
        BOTANY,
        LINGTIAN,
        PROCESSING,
        MERIDIAN,
        CRITICAL
    }
}
