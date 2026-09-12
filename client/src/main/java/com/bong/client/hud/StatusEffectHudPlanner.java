package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.StatusEffectTimeline;
import com.bong.client.combat.store.StatusEffectTimeline.Item;
import com.bong.client.combat.store.StatusEffectTimeline.Phase;

import java.util.ArrayList;
import java.util.List;
import java.util.Set;

/** Centered status emblems, with serial arrivals and unframed contamination trails. */
public final class StatusEffectHudPlanner {
    public static final int SLOT_SIZE = 30;
    public static final int SLOT_GAP = 6;
    public static final int TOP_MARGIN = 10;
    public static final String ICON_BASE = "bong-client:textures/hud/effects/";
    private static final Set<String> ICON_IDS = Set.of(
        "bleeding", "stunned", "immobilized", "vortexcasting", "parryrecovery", "staggered",
        "disoriented", "voidcoreactive", "damagereduction", "breakthroughboost",
        "antispiritpressurepill", "qiregenboost", "insightflash", "woundheal", "body_part_resist",
        "speedboost", "staminarecovboost", "health_regen_boost", "mirror_concealment",
        "swordparrying", "shieldblocking", "spirit_treasure_perception", "cultivationacceleration",
        "extraordinarymeridianacceleration", "slowed", "damageamp", "humility",
        "insighthallucination", "frailty", "qicappermminus", "contaminationboost", "body_part_weaken",
        "staminacrash", "qidrainforstamina", "legstrain", "qi_regen_paused",
        "mirror_exposed", "resonancelocked", "qiregenslowed", "damagevulnerability", "alchemy_buff", "exhausted"
    );

    private StatusEffectHudPlanner() {}

    static String iconPathFor(String id) {
        String key = id == null ? "" : id.split(":", 2)[0];
        return ICON_IDS.contains(key) ? ICON_BASE + key + ".png" : null;
    }

    public static List<HudRenderCommand> buildCommands(
        int width, int height, long nowMs, HudTextHelper.WidthMeasurer measurer
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (width <= 0 || height <= 0) return out;
        int capacity = Math.max(1, Math.min(StatusEffectStore.TOP_BAR_LIMIT, (width - 28) / 24));
        var frame = StatusEffectStore.presentation(nowMs, capacity);
        int size = Math.max(1, Math.min(SLOT_SIZE, (width - 28) / Math.max(1, frame.items().size()) - SLOT_GAP));
        int step = size + SLOT_GAP;
        int y = TOP_MARGIN;

        // Fractional occupancy moves every settled icon symmetrically as one joins or leaves.
        double occupied = frame.items().stream().mapToDouble(StatusEffectHudPlanner::weight).sum();
        double cursor = width / 2.0 - occupied * step / 2;
        Item arrival = null;
        double arrivalX = width / 2.0;
        for (Item item : frame.items()) {
            double weight = weight(item);
            double cx = cursor + weight * step / 2;
            cursor += weight * step;
            if (item.phase() == Phase.ENTERING) {
                arrival = item;
                arrivalX = cx;
            } else {
                double alpha = item.phase() == Phase.EXITING ? weight : warningAlpha(item, nowMs);
                int drawnSize = (int) Math.round(size * (item.phase() == Phase.EXITING ? .8 + .2 * weight : 1));
                drawEmblem(out, item, cx, y + size / 2.0, drawnSize, alpha, nowMs, true, measurer);
            }
        }
        if (frame.waiting() > 0) {
            centeredText(out, "+" + frame.waiting(), Math.min(width - 12, cursor + 12), y + 10,
                0xFFD7DCD0, 24, measurer);
        }
        if (arrival != null) {
            double t = travel(arrival);
            int large = Math.min(64, Math.max(size, height / 4));
            double startY = Math.min(y + 74, height * .27);
            double cx = mix(width / 2.0, arrivalX, t);
            double cy = mix(startY, y + size / 2.0, t);
            int drawnSize = (int) Math.round(mix(large, size, t));
            double alpha = Math.min(1, arrival.ageMs() / 120.0) * warningAlpha(arrival, nowMs);
            drawEmblem(out, arrival, cx, cy, drawnSize, alpha, nowMs, false, measurer);
            if (height >= 220 && t < .65) {
                centeredText(out, arrival.effect().displayName(), cx, (int) (cy + drawnSize / 2.0 + 5),
                    tint(0xFFE8EBDD, alpha * (1 - t / .65)), width - 24, measurer);
            }
        }
        return out;
    }

    private static double travel(Item item) {
        return smooth((item.ageMs() - StatusEffectTimeline.HOLD_MS)
            / (double) (StatusEffectTimeline.ENTER_MS - StatusEffectTimeline.HOLD_MS));
    }

    private static double weight(Item item) {
        return switch (item.phase()) {
            case ENTERING -> travel(item);
            case EXITING -> 1 - smooth(item.ageMs() / (double) StatusEffectTimeline.EXIT_MS);
            default -> 1;
        };
    }

    private static double warningAlpha(Item item, long nowMs) {
        if (item.remainingMs() > StatusEffectTimeline.WARNING_MS) return 1;
        return .4 + .6 * (.5 + .5 * Math.cos(nowMs * Math.PI * 2 / 650));
    }

    private static void drawEmblem(List<HudRenderCommand> out, Item item, double cx, double cy,
                                  int size, double alpha, long nowMs, boolean settled,
                                  HudTextHelper.WidthMeasurer measurer) {
        if (alpha < .02 || size < 1) return;
        int x = (int) Math.round(cx - size / 2.0);
        int y = (int) Math.round(cy - size / 2.0);
        out.add(HudRenderCommand.svg(HudRenderLayer.STATUS_EFFECTS, "socket", x, y, size, size,
            tint(0xFFFFFFFF, alpha)));
        out.add(HudRenderCommand.svg(HudRenderLayer.STATUS_EFFECTS, "rim", x, y, size, size,
            tint(item.effect().sourceColor() | 0xFF000000, alpha * .85)));
        String icon = iconPathFor(item.effect().id());
        int inset = Math.max(2, size / 7);
        if (icon != null) {
            out.add(HudRenderCommand.texture(HudRenderLayer.STATUS_EFFECTS, icon,
                x + inset, y + inset, size - 2 * inset, size - 2 * inset, tint(0xFFFFFFFF, alpha)));
        } else {
            out.add(HudRenderCommand.svg(HudRenderLayer.STATUS_EFFECTS, "unknown",
                x + inset, y + inset, size - 2 * inset, size - 2 * inset,
                tint(item.effect().sourceColor() | 0xFF000000, alpha)));
        }
        drawInvasion(out, item.effect().id(), x, y, size, nowMs, alpha);
        if (!settled) return;
        int barWidth = (int) Math.round((size - 8) * item.remainingFraction());
        if (barWidth > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x + 4, y + size - 3,
                barWidth, 1, tint(0xFFD1D8C5, alpha)));
        }
        if (item.effect().stacks() >= 2) {
            centeredText(out, "×" + Math.min(99, item.effect().stacks()), cx + size / 3.0,
                y + size - 10, tint(0xFFF1D897, alpha), size, measurer);
        }
        if (item.remainingMs() <= StatusEffectTimeline.WARNING_MS && item.phase() != Phase.EXITING) {
            centeredText(out, Long.toString((item.remainingMs() + 999) / 1000), cx, y + size + 3,
                tint(0xFFFFC1A4, alpha), size, measurer);
        }
    }

    private static void drawInvasion(List<HudRenderCommand> out, String id, int x, int y,
                                     int size, long nowMs, double alpha) {
        if (!id.equals("bleeding") && !id.equals("contaminationboost")) return;
        boolean blood = id.equals("bleeding");
        for (int i = 0; i < 2; i++) {
            double cycle = ((nowMs % 2400) / 2400.0 + i * .5) % 1;
            double fade = Math.sin(cycle * Math.PI) * alpha;
            int extra = (int) Math.round(size * (blood ? .12 : .5) * cycle);
            int extent = size + extra * 2;
            out.add(HudRenderCommand.svg(HudRenderLayer.STATUS_EFFECTS, blood ? "blood" : "taint",
                x - extra, y - extra + (blood ? (int) (cycle * size * .2) : 0), extent, extent,
                tint(blood ? 0xFFE26761 : 0xFFB3C46F, fade * .8)));
        }
    }

    private static void centeredText(List<HudRenderCommand> out, String text, double cx, int y,
                                     int color, int maxWidth, HudTextHelper.WidthMeasurer measurer) {
        String clipped = HudTextHelper.clipToWidth(text, maxWidth, measurer);
        if (!clipped.isEmpty() && (color >>> 24) > 3) {
            out.add(HudRenderCommand.text(HudRenderLayer.STATUS_EFFECTS, clipped,
                (int) Math.round(cx - measurer.measure(clipped) / 2.0), y, color));
        }
    }

    private static int tint(int color, double alpha) {
        return HudTextHelper.withAlpha(color, (int) Math.round((color >>> 24) * alpha));
    }

    private static double mix(double from, double to, double t) { return from + (to - from) * t; }
    private static double smooth(double t) {
        t = Math.max(0, Math.min(1, t));
        return t * t * (3 - 2 * t);
    }
}
