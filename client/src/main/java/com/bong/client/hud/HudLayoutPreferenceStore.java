package com.bong.client.hud;

import java.util.EnumMap;
import java.util.EnumSet;
import java.util.Map;
import java.util.Set;

public final class HudLayoutPreferenceStore {
    public enum Density {
        MINIMAL,
        STANDARD,
        MAXIMUM
    }

    private static volatile Density density = Density.STANDARD;
    private static final Map<HudLayoutPreset, EnumSet<HudLayoutPreset.Widget>> OVERRIDES =
        new EnumMap<>(HudLayoutPreset.class);

    private HudLayoutPreferenceStore() {
    }

    public static Density density() {
        return density;
    }

    public static void setDensity(Density nextDensity) {
        density = nextDensity == null ? Density.STANDARD : nextDensity;
    }

    public static synchronized void overridePreset(HudLayoutPreset preset, Set<HudLayoutPreset.Widget> widgets) {
        if (preset == null) {
            return;
        }
        if (widgets == null) {
            OVERRIDES.remove(preset);
            return;
        }
        OVERRIDES.put(preset, widgets.isEmpty()
            ? EnumSet.noneOf(HudLayoutPreset.Widget.class)
            : EnumSet.copyOf(widgets));
    }

    static synchronized EnumSet<HudLayoutPreset.Widget> widgetsFor(HudLayoutPreset preset) {
        EnumSet<HudLayoutPreset.Widget> override = OVERRIDES.get(preset);
        return override == null ? preset.defaultWidgets() : EnumSet.copyOf(override);
    }

    public static synchronized void resetForTests() {
        density = Density.STANDARD;
        OVERRIDES.clear();
    }
}
