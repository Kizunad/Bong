package com.bong.client.combat.inspect;

import com.bong.client.combat.store.WoundsStore;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.BodyPartState;
import com.bong.client.inventory.model.PhysicalBody;
// EnumMap no longer needed.
import com.bong.client.inventory.model.WoundLevel;
import com.bong.client.inventory.state.PhysicalBodyStore;

import java.util.Map;

/**
 * Binds {@link WoundsStore} payload data into the inspect screen's existing
 * {@link PhysicalBodyStore} (plan §U1). Runs idempotently — can be called on
 * every wounds_snapshot update; a full {@link PhysicalBody} is rebuilt from
 * scratch so partial states vanish cleanly.
 */
public final class WoundLayerBinding {

    /** Best-effort mapping from server wire "part" id → client BodyPart enum. */
    public static BodyPart resolvePart(String wireId) {
        if (wireId == null) return null;
        return switch (wireId.trim().toLowerCase(java.util.Locale.ROOT)) {
            case "head" -> BodyPart.HEAD;
            case "neck" -> BodyPart.NECK;
            case "chest" -> BodyPart.CHEST;
            case "abdomen", "belly" -> BodyPart.ABDOMEN;
            case "left_upper_arm", "l_upper_arm" -> BodyPart.LEFT_UPPER_ARM;
            case "left_forearm", "l_forearm" -> BodyPart.LEFT_FOREARM;
            case "left_hand", "l_hand" -> BodyPart.LEFT_HAND;
            case "right_upper_arm", "r_upper_arm" -> BodyPart.RIGHT_UPPER_ARM;
            case "right_forearm", "r_forearm" -> BodyPart.RIGHT_FOREARM;
            case "right_hand", "r_hand" -> BodyPart.RIGHT_HAND;
            case "left_thigh", "l_thigh" -> BodyPart.LEFT_THIGH;
            case "left_calf", "l_calf" -> BodyPart.LEFT_CALF;
            case "left_foot", "l_foot" -> BodyPart.LEFT_FOOT;
            case "right_thigh", "r_thigh" -> BodyPart.RIGHT_THIGH;
            case "right_calf", "r_calf" -> BodyPart.RIGHT_CALF;
            case "right_foot", "r_foot" -> BodyPart.RIGHT_FOOT;
            default -> null;
        };
    }

    /** Map a wounds-store wound to the coarser inspect {@link WoundLevel}. */
    public static WoundLevel toWoundLevel(WoundsStore.Wound w) {
        if (w == null) return WoundLevel.INTACT;
        if (w.state() == WoundsStore.HealingState.SCARRED && w.severity() < 0.2f) return WoundLevel.BRUISE;
        float s = w.severity();
        if ("bone_fracture".equals(w.kind()) && s >= 0.5f) return WoundLevel.FRACTURE;
        if (s >= 0.85f) return WoundLevel.SEVERED;
        if (s >= 0.55f) return WoundLevel.LACERATION;
        if (s >= 0.25f) return WoundLevel.ABRASION;
        if (s >= 0.05f) return WoundLevel.BRUISE;
        return WoundLevel.INTACT;
    }

    /** Build a fresh {@link PhysicalBody} snapshot from the current store. */
    public static PhysicalBody buildBody() {
        Map<String, WoundsStore.Wound> snapshot = WoundsStore.snapshot();
        PhysicalBody.Builder builder = PhysicalBody.builder();
        if (snapshot != null) {
            for (Map.Entry<String, WoundsStore.Wound> entry : snapshot.entrySet()) {
                BodyPart part = resolvePart(entry.getKey());
                if (part == null) continue;
                WoundsStore.Wound w = entry.getValue();
                WoundLevel level = toWoundLevel(w);
                double bleed = w.state() == WoundsStore.HealingState.BLEEDING ? w.severity() : 0.0;
                double heal = switch (w.state()) {
                    case HEALING -> 1.0 - w.severity();
                    case STANCHED -> 0.3 * (1.0 - w.severity());
                    case SCARRED -> 1.0;
                    case BLEEDING -> 0.0;
                };
                boolean splinted = level == WoundLevel.FRACTURE && w.state() != WoundsStore.HealingState.BLEEDING;
                builder.part(new BodyPartState(part, level, bleed, heal, splinted));
            }
        }
        return builder.build();
    }

    /** Push the current wounds into the inspect PhysicalBodyStore. */
    public static void apply() {
        PhysicalBodyStore.replace(buildBody());
    }

    private WoundLayerBinding() {}
}
