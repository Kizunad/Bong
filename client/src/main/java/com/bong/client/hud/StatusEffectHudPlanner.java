package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Top-center 8-slot status-effect strip (plan §U2 / §1 "HUD 顶部状态效果栏").
 * Icon border uses {@code source_color}; stacks &ge; 2 show ×N; remaining time
 * renders as a thin bottom progress bar.
 */
public final class StatusEffectHudPlanner {
    public static final int SLOT_SIZE = 18;
    public static final int SLOT_GAP = 3;
    public static final int TOP_MARGIN = 4;
    public static final int TRACK_BG = 0xC0101820;
    public static final int STACK_COLOR = 0xFFFFE080;
    public static final int REMAINING_BAR_COLOR = 0xFFFFFFFF;
    public static final int DEBUFF_REMAINING_BAR_COLOR = 0xFFFF4040;

    private StatusEffectHudPlanner() {}

    /** plan-cultivation-pacing-v1 P2.3: text color for cultivation acceleration indicator. */
    public static final int CULTIVATION_ACCEL_COLOR = 0xFF80E0A0;

    /** Texture namespace+path prefix for per-effect emblem icons. */
    public static final String ICON_BASE = "bong-client:textures/hud/effects/";

    /**
     * Effect ids that ship a dedicated emblem texture under {@link #ICON_BASE}.
     * Keys match server {@code status_effect_id()} output — most variants are
     * {@code format!("{:?}").to_ascii_lowercase()} (PascalCase → all-lowercase,
     * no underscore, e.g. {@code DamageReduction → "damagereduction"}); a few
     * explicit arms keep underscores ({@code qi_regen_paused}). Parameterized
     * ids ({@code body_part_resist:head}) resolve to their base key through
     * {@link #iconKey}, so all body parts share one emblem.
     */
    private static final java.util.Set<String> ICON_IDS = java.util.Set.of(
        // dot
        "bleeding",
        // control
        "stunned", "vortexcasting", "parryrecovery", "staggered", "disoriented",
        "voidcoreactive",
        // buff
        "damagereduction", "breakthroughboost", "antispiritpressurepill",
        "qiregenboost", "insightflash", "woundheal", "body_part_resist",
        "speedboost", "staminarecovboost", "mirror_concealment", "swordparrying",
        "spirit_treasure_perception", "cultivationacceleration",
        "extraordinarymeridianacceleration",
        // debuff
        "slowed", "damageamp", "humility", "insighthallucination", "frailty",
        "qicappermminus", "contaminationboost", "body_part_weaken", "staminacrash",
        "qidrainforstamina", "legstrain", "qi_regen_paused", "mirror_exposed",
        "resonancelocked", "qiregenslowed", "damagevulnerability",
        // unknown
        "alchemy_buff"
    );

    /** Strip a parameterized suffix ({@code body_part_resist:head → body_part_resist}). */
    static String iconKey(String id) {
        if (id == null) return "";
        int colon = id.indexOf(':');
        return colon >= 0 ? id.substring(0, colon) : id;
    }

    /** Texture path for an effect id, or {@code null} when no emblem ships for it. */
    static String iconPathFor(String id) {
        String key = iconKey(id);
        return ICON_IDS.contains(key) ? ICON_BASE + key + ".png" : null;
    }

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        // plan-cultivation-pacing-v1 P2.3: show cultivation acceleration multiplier above status slots.
        double accel = StatusEffectStore.cultivationAcceleration();
        if (accel > 1.001) {
            String label = String.format(Locale.US, "修炼 ×%.1f", accel); // "修炼 ×N.N"
            int labelWidth = label.length() * 6; // approximate glyph width
            int labelX = (screenWidth - labelWidth) / 2;
            int labelY = TOP_MARGIN;
            out.add(HudRenderCommand.text(
                HudRenderLayer.STATUS_EFFECTS,
                label,
                labelX,
                labelY,
                CULTIVATION_ACCEL_COLOR
            ));
        }

        List<StatusEffectStore.Effect> top = StatusEffectStore.topBar();
        if (top.isEmpty()) return out;

        int totalWidth = top.size() * SLOT_SIZE + (top.size() - 1) * SLOT_GAP;
        int x = (screenWidth - totalWidth) / 2;
        // Shift status slots down if cultivation label is shown.
        int y = (accel > 1.001) ? TOP_MARGIN + 12 : TOP_MARGIN;

        for (StatusEffectStore.Effect e : top) {
            // Border (2px frame in source color)
            int border = e.sourceColor();
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, y, SLOT_SIZE, SLOT_SIZE, border));
            // Inner background
            out.add(HudRenderCommand.rect(
                HudRenderLayer.STATUS_EFFECTS, x + 1, y + 1, SLOT_SIZE - 2, SLOT_SIZE - 2, TRACK_BG
            ));
            // Inner fill: a per-effect emblem texture when one ships, otherwise a
            // flat kind tint so an un-iconned effect still reads as its category.
            // The source-color border (above) carries the category either way.
            String iconPath = iconPathFor(e.id());
            if (iconPath != null) {
                out.add(HudRenderCommand.texture(
                    HudRenderLayer.STATUS_EFFECTS, iconPath,
                    x + 2, y + 2, SLOT_SIZE - 4, SLOT_SIZE - 4, 0xFFFFFFFF
                ));
            } else {
                int tint = tintForKind(e.kind());
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.STATUS_EFFECTS, x + 2, y + 2, SLOT_SIZE - 4, SLOT_SIZE - 4, tint
                ));
            }
            // Remaining-time bar (bottom 2px)
            long rem = e.remainingMs();
            float norm = remainingNorm(rem);
            int bar = Math.max(0, Math.round((SLOT_SIZE - 4) * norm));
            if (bar > 0) {
                int barColor = e.kind() == StatusEffectStore.Kind.DEBUFF
                    ? DEBUFF_REMAINING_BAR_COLOR
                    : REMAINING_BAR_COLOR;
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.STATUS_EFFECTS, x + 2, y + SLOT_SIZE - 3, bar, 1, barColor
                ));
            }
            // Stack count
            if (e.stacks() >= 2) {
                out.add(HudRenderCommand.text(
                    HudRenderLayer.STATUS_EFFECTS,
                    "\u00D7" + Math.min(99, e.stacks()),
                    x + SLOT_SIZE - 10,
                    y + SLOT_SIZE - 9,
                    STACK_COLOR
                ));
            }
            x += SLOT_SIZE + SLOT_GAP;
        }
        return out;
    }

    private static int tintForKind(StatusEffectStore.Kind kind) {
        return switch (kind) {
            case DOT -> 0x80E04040;
            case CONTROL -> 0x80B060FF;
            case BUFF -> 0x8060D060;
            case DEBUFF -> 0x80FFA030;
            case UNKNOWN -> 0x80808080;
        };
    }

    private static float remainingNorm(long remainingMs) {
        if (remainingMs <= 0L) return 0f;
        // Clamp to 30s; anything longer appears as full.
        float norm = remainingMs / 30_000f;
        if (norm > 1f) return 1f;
        if (norm < 0f) return 0f;
        return norm;
    }
}
