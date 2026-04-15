package com.bong.client.hud;

import com.bong.client.combat.CastState;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;

import java.util.ArrayList;
import java.util.List;

/**
 * Two-row quick-bar renderer (§2.2). Produces:
 * <ul>
 *   <li>Upper row — F1-F9 custom quick-use slots (green label), including the
 *       cast-bar for the currently casting slot (§3.5).</li>
 *   <li>Lower row — 1-9 native combat hotbar outline (treasure-weapon purple
 *       border when applicable).</li>
 * </ul>
 *
 * <p>Geometry is centred on the screen bottom, mirroring MC&#x27;s native
 * hotbar.
 */
public final class QuickBarHudPlanner {
    public static final int SLOT_SIZE = 20;
    public static final int SLOT_GAP = 2;
    public static final int LOWER_BOTTOM_MARGIN = 22; // above native hotbar
    public static final int UPPER_GAP = 4;
    public static final int TOTAL_SLOTS = 9;

    static final int SLOT_BG_COLOR = 0xA0000000;
    static final int QUICK_LABEL_COLOR = 0xFF80FFCC;
    static final int COMBAT_LABEL_COLOR = 0xFFFFFFFF;
    static final int SELECTED_BORDER_COLOR = 0xFFFFFFFF;
    static final int SPELL_BORDER_COLOR = 0xFFC040FF;
    static final int COOLDOWN_OVERLAY_COLOR = 0xC0555555;

    static final int CAST_BAR_HEIGHT = 3;
    static final int CAST_BAR_BG = 0xFF1A1000;
    static final int CAST_BAR_FG = 0xFFFFCC40;
    static final int CAST_BAR_INTERRUPT = 0xFFFF4040;

    private QuickBarHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        QuickSlotConfig quickSlots,
        int selectedHotbarSlot,
        CastState castState,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int barWidth = TOTAL_SLOTS * SLOT_SIZE + (TOTAL_SLOTS - 1) * SLOT_GAP;
        int leftX = (screenWidth - barWidth) / 2;

        int lowerY = screenHeight - LOWER_BOTTOM_MARGIN - SLOT_SIZE;
        int upperY = lowerY - SLOT_SIZE - UPPER_GAP;

        // Upper row (F1-F9)
        appendQuickUseRow(out, quickSlots, castState, leftX, upperY, nowMillis);

        // Lower row (1-9) — frame only; MC renders the item; we add selection + spell tint.
        appendCombatRow(out, selectedHotbarSlot, leftX, lowerY);

        return out;
    }

    private static void appendQuickUseRow(
        List<HudRenderCommand> out,
        QuickSlotConfig cfg,
        CastState castState,
        int leftX,
        int y,
        long nowMillis
    ) {
        QuickSlotConfig config = cfg == null ? QuickSlotConfig.empty() : cfg;
        for (int i = 0; i < TOTAL_SLOTS; i++) {
            int x = leftX + i * (SLOT_SIZE + SLOT_GAP);
            out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, SLOT_SIZE, SLOT_SIZE, SLOT_BG_COLOR));
            appendBorder(out, HudRenderLayer.QUICK_BAR, x, y, SLOT_SIZE, SLOT_SIZE, QUICK_LABEL_COLOR);

            QuickSlotEntry entry = config.slot(i);
            if (entry == null) {
                continue;
            }

            // Cooldown mask
            if (config.isOnCooldown(i, nowMillis)) {
                out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x + 1, y + 1, SLOT_SIZE - 2, SLOT_SIZE - 2, COOLDOWN_OVERLAY_COLOR));
            }

            // Cast bar for the active slot
            if (castState != null && castState.slot() == i) {
                appendCastBar(out, x, y + SLOT_SIZE + 1, castState, nowMillis);
            }
        }
    }

    private static void appendCombatRow(List<HudRenderCommand> out, int selectedSlot, int leftX, int y) {
        for (int i = 0; i < TOTAL_SLOTS; i++) {
            int x = leftX + i * (SLOT_SIZE + SLOT_GAP);
            int borderColor = (selectedSlot == i) ? SELECTED_BORDER_COLOR : 0x60FFFFFF;
            appendBorder(out, HudRenderLayer.QUICK_BAR, x, y, SLOT_SIZE, SLOT_SIZE, borderColor);
        }
    }

    private static void appendCastBar(List<HudRenderCommand> out, int x, int y, CastState cast, long nowMs) {
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, SLOT_SIZE, CAST_BAR_HEIGHT, CAST_BAR_BG));
        if (cast.phase() == CastState.Phase.CASTING) {
            int width = Math.round(cast.progress(nowMs) * SLOT_SIZE);
            if (width > 0) {
                out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, width, CAST_BAR_HEIGHT, CAST_BAR_FG));
            }
        } else if (cast.phase() == CastState.Phase.INTERRUPT) {
            out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, SLOT_SIZE, CAST_BAR_HEIGHT, CAST_BAR_INTERRUPT));
        } else if (cast.phase() == CastState.Phase.COMPLETE) {
            out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, SLOT_SIZE, CAST_BAR_HEIGHT, CAST_BAR_FG));
        }
    }

    private static void appendBorder(
        List<HudRenderCommand> out,
        HudRenderLayer layer,
        int x,
        int y,
        int w,
        int h,
        int color
    ) {
        out.add(HudRenderCommand.rect(layer, x, y, w, 1, color));
        out.add(HudRenderCommand.rect(layer, x, y + h - 1, w, 1, color));
        out.add(HudRenderCommand.rect(layer, x, y, 1, h, color));
        out.add(HudRenderCommand.rect(layer, x + w - 1, y, 1, h, color));
    }
}
