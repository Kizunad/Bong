package com.bong.client.combat.juice;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.WoundsStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class WoundWorldVisualPlanner {
    private WoundWorldVisualPlanner() {
    }

    public static List<WoundCommand> plan(List<WoundsStore.Wound> wounds, List<StatusEffectStore.Effect> effects, boolean exhausted) {
        List<WoundCommand> out = new ArrayList<>();
        float contamination = 0f;
        if (wounds != null) {
            for (WoundsStore.Wound wound : wounds) {
                if (wound == null) {
                    continue;
                }
                contamination = Math.max(contamination, wound.infection());
                String kind = wound.kind() == null ? "" : wound.kind().toLowerCase(Locale.ROOT);
                String partId = wound.partId() == null ? "" : wound.partId();
                String normalizedPartId = partId.toLowerCase(Locale.ROOT);
                boolean fracture = "bone_fracture".equals(kind) && wound.severity() >= 0.5f;
                boolean severed = "severed".equals(kind) || "limb_severed".equals(kind) || "amputation".equals(kind);
                boolean lowerLimb = normalizedPartId.contains("leg") || normalizedPartId.contains("foot");
                if (fracture || severed) {
                    out.add(new WoundCommand(
                        partId,
                        fracture ? 5.0f : 0.0f,
                        tint(wound.kindColor(), fracture ? 0x26 : 0x40),
                        severed,
                        lowerLimb && (fracture || severed),
                        false,
                        false,
                        false
                    ));
                }
            }
        }
        contamination = Math.max(contamination, contaminationFromEffects(effects));
        if (contamination > 0.3f) {
            out.add(new WoundCommand("meridian_contamination", 0.0f, 0x4D2E4E2E, false, false, true, contamination > 0.7f, false));
        }
        // 虚脱踉跄：显式 exhausted 入参，或标准 debuff 快照里存在虚脱条目（id "exhausted"）。
        // 不再读 FullPowerStateStore——虚脱已收敛到标准 status，由 status_snapshot 驱动，
        // /reset 与到期都会从快照消失，踉跄随之停止。
        if (exhausted || hasExhaustedEffect(effects)) {
            out.add(new WoundCommand("exhausted", 0.0f, 0x33606060, false, false, false, false, true));
        }
        return List.copyOf(out);
    }

    private static boolean hasExhaustedEffect(List<StatusEffectStore.Effect> effects) {
        if (effects == null) {
            return false;
        }
        for (StatusEffectStore.Effect effect : effects) {
            if (effect != null
                && "exhausted".equals(effect.id())
                && effect.remainingMs() > 0L) {
                return true;
            }
        }
        return false;
    }

    private static float contaminationFromEffects(List<StatusEffectStore.Effect> effects) {
        if (effects == null || effects.isEmpty()) {
            return 0f;
        }
        float max = 0f;
        for (StatusEffectStore.Effect effect : effects) {
            if (effect == null) {
                continue;
            }
            String id = effect.id() == null ? "" : effect.id().toLowerCase(Locale.ROOT);
            if (id.contains("contam") || id.contains("poison") || id.contains("taint")) {
                max = Math.max(max, effect.stacks() >= 3 ? 0.75f : 0.45f);
            }
        }
        return max;
    }

    private static int tint(int rgb, int alpha) {
        return ((alpha & 0xFF) << 24) | (rgb & 0x00FFFFFF);
    }

    public record WoundCommand(
        String partId,
        float limbTiltDegrees,
        int tintArgb,
        boolean dripParticle,
        boolean limpAnimation,
        boolean meridianGlow,
        boolean coughAudio,
        boolean exhaustedStumble
    ) {
    }
}
