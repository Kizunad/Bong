package com.bong.client.hud;

import com.bong.client.combat.CastState;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.inventory.model.InventoryItem;

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
    /// 槽位绑定后的填充色（轻微绿染表示「已配置 / 可触发」）。
    static final int SLOT_BOUND_FILL_COLOR = 0x4080FFCC;
    static final int QUICK_LABEL_COLOR = 0xFF80FFCC;
    static final int COMBAT_LABEL_COLOR = 0xFFFFFFFF;
    static final int SELECTED_BORDER_COLOR = 0xFFFFFFFF;
    static final int SPELL_BORDER_COLOR = 0xFFC040FF;
    static final int COOLDOWN_OVERLAY_COLOR = 0xC0555555;

    static final int CAST_BAR_HEIGHT = 3;
    static final int CAST_BAR_BG = 0xFF1A1000;
    static final int CAST_BAR_FG = 0xFFFFCC40;
    static final int CAST_BAR_INTERRUPT = 0xFFFF4040;

    /**
     * 物品图标在槽内的内边距：实际 icon 边长 = SLOT_SIZE - 2 * ICON_INSET。
     * 调小 → 图标占满槽（贴边），调大 → 图标缩小留更多边框空间。
     */
    public static int ICON_INSET = 2;

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
        return buildCommands(
            quickSlots, SkillBarConfig.empty(), selectedHotbarSlot, castState,
            List.of(), // back-compat overload — no native hotbar items
            nowMillis, screenWidth, screenHeight
        );
    }

    public static List<HudRenderCommand> buildCommands(
        QuickSlotConfig quickSlots,
        SkillBarConfig skillBar,
        int selectedHotbarSlot,
        CastState castState,
        List<InventoryItem> nativeHotbar,
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

        // Lower row (1-9 战斗栏) —— 之前只画边框，现在与 InventoryModel.hotbar 同步
        // 物品（MC vanilla 那边没接入 Bong items，所以底部 native hotbar 也是空）。
        appendCombatRow(out, selectedHotbarSlot, leftX, lowerY, nativeHotbar, skillBar, castState, nowMillis);

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

            // 已绑定 → 槽内淡绿色背景 + 物品 PNG 贴图（128×128 缩到 SLOT_SIZE×SLOT_SIZE 内）。
            out.add(HudRenderCommand.rect(
                HudRenderLayer.QUICK_BAR,
                x + 1, y + 1,
                SLOT_SIZE - 2, SLOT_SIZE - 2,
                SLOT_BOUND_FILL_COLOR
            ));
            int iconSize = SLOT_SIZE - 2 * ICON_INSET;
            out.add(HudRenderCommand.itemTexture(
                HudRenderLayer.QUICK_BAR, entry.itemId(), x + ICON_INSET, y + ICON_INSET, iconSize
            ));

            // Cooldown mask
            if (config.isOnCooldown(i, nowMillis)) {
                out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x + 1, y + 1, SLOT_SIZE - 2, SLOT_SIZE - 2, COOLDOWN_OVERLAY_COLOR));
            }

            // Cast bar for the active slot
            if (castState != null && castState.source() == CastState.Source.QUICK_SLOT && castState.slot() == i) {
                appendCastBar(out, x, y + SLOT_SIZE + 1, castState, nowMillis);
            }
        }
    }

    private static void appendCombatRow(
        List<HudRenderCommand> out,
        int selectedSlot,
        int leftX,
        int y,
        List<InventoryItem> nativeHotbar,
        SkillBarConfig skillBar,
        CastState castState,
        long nowMillis
    ) {
        SkillBarConfig skills = skillBar == null ? SkillBarConfig.empty() : skillBar;
        for (int i = 0; i < TOTAL_SLOTS; i++) {
            int x = leftX + i * (SLOT_SIZE + SLOT_GAP);
            int borderColor = (selectedSlot == i) ? SELECTED_BORDER_COLOR : 0x60FFFFFF;
            appendBorder(out, HudRenderLayer.QUICK_BAR, x, y, SLOT_SIZE, SLOT_SIZE, borderColor);

            SkillBarEntry skillEntry = skills.slot(i);
            if (skillEntry != null && skillEntry.kind() == SkillBarEntry.Kind.SKILL) {
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.QUICK_BAR,
                    x + 1, y + 1,
                    SLOT_SIZE - 2, SLOT_SIZE - 2,
                    0x404080FF
                ));
                String label = skillLabel(skillEntry.displayName(), skillEntry.id());
                out.add(HudRenderCommand.text(HudRenderLayer.QUICK_BAR, label, x + 4, y + 6, 0xFFE0D080));
                if (skills.isOnCooldown(i, nowMillis)) {
                    out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x + 1, y + 1, SLOT_SIZE - 2, SLOT_SIZE - 2, COOLDOWN_OVERLAY_COLOR));
                }
                if (castState != null && castState.source() == CastState.Source.SKILL_BAR && castState.slot() == i) {
                    appendCastBar(out, x, y + SLOT_SIZE + 1, castState, nowMillis);
                }
                continue;
            }

            InventoryItem item = (nativeHotbar != null && i < nativeHotbar.size())
                ? nativeHotbar.get(i)
                : null;
            if (item == null) continue;

            // 同 F-bar：淡白底 + 真实 PNG 贴图（缺 PNG 时 MC 会显示 missing_texture）。
            out.add(HudRenderCommand.rect(
                HudRenderLayer.QUICK_BAR,
                x + 1, y + 1,
                SLOT_SIZE - 2, SLOT_SIZE - 2,
                0x40FFFFFF
            ));
            int iconSize = SLOT_SIZE - 2 * ICON_INSET;
            out.add(HudRenderCommand.itemTexture(
                HudRenderLayer.QUICK_BAR, item.itemId(), x + ICON_INSET, y + ICON_INSET, iconSize
            ));
        }
    }

    private static String skillLabel(String displayName, String skillId) {
        String source = displayName == null || displayName.isBlank() ? skillId : displayName;
        if (source == null || source.isBlank()) return "技";
        String trimmed = source.trim();
        return trimmed.length() <= 2 ? trimmed : trimmed.substring(0, 1);
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
