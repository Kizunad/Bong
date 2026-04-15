package com.bong.client.combat.inspect;

import com.bong.client.combat.store.StatusEffectStore;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * Groups {@link StatusEffectStore} entries by kind for the inspect "状态"
 * panel (plan §U2 §U5). Pure data transform — rendering is delegated to
 * whatever owo-lib layout hosts this panel (wired by {@code InspectScreen}).
 */
public final class StatusPanelExtension {

    public record Group(StatusEffectStore.Kind kind, List<StatusEffectStore.Effect> effects) {
        public Group {
            effects = effects == null ? List.of() : List.copyOf(effects);
        }
    }

    public static List<Group> groupedByKind() {
        List<StatusEffectStore.Effect> all = StatusEffectStore.snapshot();
        Map<StatusEffectStore.Kind, List<StatusEffectStore.Effect>> bins =
            new EnumMap<>(StatusEffectStore.Kind.class);
        for (StatusEffectStore.Kind k : StatusEffectStore.Kind.values()) {
            bins.put(k, new ArrayList<>());
        }
        for (StatusEffectStore.Effect e : all) {
            bins.get(e.kind()).add(e);
        }
        List<Group> result = new ArrayList<>();
        for (StatusEffectStore.Kind k : StatusEffectStore.Kind.values()) {
            List<StatusEffectStore.Effect> list = bins.get(k);
            if (list.isEmpty()) continue;
            result.add(new Group(k, list));
        }
        return result;
    }

    public static String tooltipFor(StatusEffectStore.Effect e) {
        if (e == null) return "";
        StringBuilder sb = new StringBuilder();
        sb.append(e.displayName());
        if (e.stacks() >= 2) sb.append(" \u00D7").append(e.stacks());
        sb.append('\n');
        if (!e.sourceLabel().isEmpty()) {
            sb.append("\u6765\u6e90: ").append(e.sourceLabel()).append('\n');
        }
        sb.append("\u5269\u4f59: ").append(formatMs(e.remainingMs())).append('\n');
        sb.append("\u9a71\u6563\u96be\u5ea6: ").append(e.dispelDifficulty()).append("/5");
        return sb.toString();
    }

    private static String formatMs(long ms) {
        if (ms <= 0L) return "0.0s";
        double s = ms / 1000.0;
        return String.format(java.util.Locale.ROOT, "%.1fs", s);
    }

    private StatusPanelExtension() {}
}
