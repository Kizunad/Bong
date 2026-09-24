package com.bong.client.combat.inspect;

import com.bong.client.combat.store.WoundsStore;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.BodyPartState;
import com.bong.client.inventory.model.PhysicalBody;
// EnumMap no longer needed.
import com.bong.client.inventory.model.WoundLevel;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.bong.client.inventory.state.PhysicalBodyStore;

import java.util.Locale;
import java.util.Map;

/**
 * Binds {@link WoundsStore} payload data into the inspect screen's existing
 * {@link PhysicalBodyStore} (plan §U1). Runs idempotently — can be called on
 * every wounds_snapshot update; a full {@link PhysicalBody} is rebuilt from
 * scratch so partial states vanish cleanly.
 */
public final class WoundLayerBinding {

    /**
     * plan-race-system-v1 P2b — 短别名兼容表：server 历史遗留过更短的 wire 缩写
     * （如 {@code l_thigh}），与当前 {@code BodyPlanLayout} 的规范 16 段展示 id
     * （{@code left_thigh}）不是同一字符串，layout 里查不到，必须单独兜底，
     * 否则老 wire 会被规范化校验（见 {@link #isDeclaredByCurrentLayout}）误伤。
     */
    private static final Map<String, BodyPart> LEGACY_ALIASES = Map.ofEntries(
        Map.entry("belly", BodyPart.ABDOMEN),
        Map.entry("l_upper_arm", BodyPart.LEFT_UPPER_ARM),
        Map.entry("l_forearm", BodyPart.LEFT_FOREARM),
        Map.entry("l_hand", BodyPart.LEFT_HAND),
        Map.entry("r_upper_arm", BodyPart.RIGHT_UPPER_ARM),
        Map.entry("r_forearm", BodyPart.RIGHT_FOREARM),
        Map.entry("r_hand", BodyPart.RIGHT_HAND),
        Map.entry("l_thigh", BodyPart.LEFT_THIGH),
        Map.entry("l_calf", BodyPart.LEFT_CALF),
        Map.entry("l_foot", BodyPart.LEFT_FOOT),
        Map.entry("r_thigh", BodyPart.RIGHT_THIGH),
        Map.entry("r_calf", BodyPart.RIGHT_CALF),
        Map.entry("r_foot", BodyPart.RIGHT_FOOT)
    );

    /**
     * server wire "part" id → client {@link BodyPart} enum。
     *
     * <p>plan-race-system-v1 P2b — 规范（非别名）id 的合法性改读
     * {@link BodyPlanLayoutStore} 当前 layout 声明的部位集合（{@code anchors} /
     * {@code part_display_map.display_segment_id}），不再是纯字符串硬编码白名单。
     * store 尚未收到任何 layout（首帧竞态）时退化为放行（交由
     * {@link BodyPart#valueOf} 兜底），保证不因竞态误伤既有渲染；非人形部位 id
     * （如 server 7→16 遗留映射里的 {@code back}）在 layout 已加载时若不被当前
     * plan 声明，同样安全落 {@code null}，调用方原样跳过（不渲染），不 crash。
     */
    public static BodyPart resolvePart(String wireId) {
        if (wireId == null) return null;
        String id = wireId.trim().toLowerCase(Locale.ROOT);
        BodyPart alias = LEGACY_ALIASES.get(id);
        if (alias != null) return alias;
        if (!isDeclaredByCurrentLayout(id)) return null;
        try {
            return BodyPart.valueOf(id.toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException e) {
            return null;
        }
    }

    private static boolean isDeclaredByCurrentLayout(String canonicalId) {
        BodyPlanLayout layout = BodyPlanLayoutStore.current();
        if (layout == null) return true;
        return layout.declaresPart(canonicalId);
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
