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
 * 双排快捷栏：上排快捷使用、下排战斗栏。SVG 槽框承载选中和施法来源提示，
 * 物品、技能图标与冷却遮罩沿用 GUI 提交，施法进度由独立残环呈现。
 *
 * <p>Geometry is centred on the screen bottom, mirroring MC&#x27;s native
 * hotbar.
 */
public final class QuickBarHudPlanner {
    public static final int SLOT_SIZE = 20;
    public static final int SLOT_GAP = 2;
    public static final int LOWER_BOTTOM_MARGIN = 22; // above native hotbar
    public static final int UPPER_GAP = 4;
    /** 两排共同占用的横向槽数，供侧边武器槽和状态提示避让。 */
    public static final int TOTAL_SLOTS = Math.max(QuickSlotConfig.SLOT_COUNT, SkillBarConfig.SLOT_COUNT);

    static final int COOLDOWN_OVERLAY_COLOR = 0xC0555555;

    /**
     * 物品图标在槽内的内边距：实际 icon 边长 = SLOT_SIZE - 2 * ICON_INSET。
     * 调小 → 图标占满槽（贴边），调大 → 图标缩小留更多边框空间。
     */
    public static int ICON_INSET = 3;

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
        // 旧重载：不做贴图存在性检查（保留历史行为：非空 iconTexture 即画，缺图由 MC 兜底）。
        return buildCommands(
            quickSlots, skillBar, selectedHotbarSlot, castState, nativeHotbar,
            nowMillis, screenWidth, screenHeight, null
        );
    }

    /**
     * @param skillTextureExists 战斗栏技能图标的贴图存在性谓词；非 null 时缺图回退到文字标签
     *                           （消除紫黑 missing-texture）；null 走旧行为。生产传 {@code HudTextureProbe::exists}。
     */
    public static List<HudRenderCommand> buildCommands(
        QuickSlotConfig quickSlots,
        SkillBarConfig skillBar,
        int selectedHotbarSlot,
        CastState castState,
        List<InventoryItem> nativeHotbar,
        long nowMillis,
        int screenWidth,
        int screenHeight,
        java.util.function.Predicate<String> skillTextureExists
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int lowerY = screenHeight - LOWER_BOTTOM_MARGIN - SLOT_SIZE;
        int upperY = lowerY - SLOT_SIZE - UPPER_GAP;

        // 每排按自身开放数量居中，扩格时两侧均匀展开，不占用未开放格的空位。
        appendQuickUseRow(out, quickSlots, castState,
            rowLeftX(screenWidth, QuickSlotConfig.SLOT_COUNT), upperY, nowMillis);

        // 下排同时呈现技能绑定与 InventoryModel.hotbar 中的物品。
        appendCombatRow(out, selectedHotbarSlot, rowLeftX(screenWidth, SkillBarConfig.SLOT_COUNT),
            lowerY, nativeHotbar, skillBar, castState, nowMillis, skillTextureExists);

        out.addAll(CastRingHudPlanner.buildCommands(castState, nowMillis, screenWidth, screenHeight));

        return out;
    }

    static int rowLeftX(int screenWidth, int slotCount) {
        int width = slotCount * SLOT_SIZE + Math.max(0, slotCount - 1) * SLOT_GAP;
        return (screenWidth - width) / 2;
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
        for (int i = 0; i < QuickSlotConfig.SLOT_COUNT; i++) {
            int x = leftX + i * (SLOT_SIZE + SLOT_GAP);
            appendSlotFrame(out, x, y, false, activeSlot(castState, CastState.Source.QUICK_SLOT, i), castState);

            QuickSlotEntry entry = config.slot(i);
            if (entry == null) {
                continue;
            }

            int iconSize = SLOT_SIZE - 2 * ICON_INSET;
            out.add(HudRenderCommand.itemTexture(
                HudRenderLayer.QUICK_BAR, entry.itemId(), x + ICON_INSET, y + ICON_INSET, iconSize
            ));

            // Cooldown mask
            if (config.isOnCooldown(i, nowMillis)) {
                appendCooldown(out, x, y);
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
        long nowMillis,
        java.util.function.Predicate<String> skillTextureExists
    ) {
        SkillBarConfig skills = skillBar == null ? SkillBarConfig.empty() : skillBar;
        for (int i = 0; i < SkillBarConfig.SLOT_COUNT; i++) {
            int x = leftX + i * (SLOT_SIZE + SLOT_GAP);
            appendSlotFrame(out, x, y, selectedSlot == i, activeSlot(castState, CastState.Source.SKILL_BAR, i), castState);

            SkillBarEntry skillEntry = skills.slot(i);
            if (skillEntry != null && skillEntry.kind() == SkillBarEntry.Kind.ITEM) {
                int iconSize = SLOT_SIZE - 2 * ICON_INSET;
                if (skillEntry.iconTexture() == null || skillEntry.iconTexture().isBlank()) {
                    out.add(HudRenderCommand.itemTexture(
                        HudRenderLayer.QUICK_BAR, skillEntry.id(), x + ICON_INSET, y + ICON_INSET, iconSize
                    ));
                } else {
                    out.add(HudRenderCommand.texture(
                        HudRenderLayer.QUICK_BAR,
                        skillEntry.iconTexture(),
                        x + ICON_INSET,
                        y + ICON_INSET,
                        iconSize,
                        iconSize,
                        0xFFFFFFFF
                    ));
                }
                if (skills.isOnCooldown(i, nowMillis)) {
                    appendCooldown(out, x, y);
                }
                continue;
            }

            if (skillEntry != null && skillEntry.kind() == SkillBarEntry.Kind.SKILL) {
                int iconSize = SLOT_SIZE - 2 * ICON_INSET;
                // 有谓词(生产)→ 存在性感知解析(缺图回退文字)；null(旧重载/部分测试)→ 旧行为。
                List<HudRenderCommand> iconCommands = skillTextureExists == null
                    ? LoadoutIconLayer.buildSkillIconCommands(
                        skillEntry, x + ICON_INSET, y + ICON_INSET, iconSize)
                    : LoadoutIconLayer.buildSkillIconCommands(
                        skillEntry, x + ICON_INSET, y + ICON_INSET, iconSize, skillTextureExists);
                if (iconCommands.isEmpty()) {
                    String label = skillLabel(skillEntry.displayName(), skillEntry.id());
                    out.add(HudRenderCommand.text(HudRenderLayer.QUICK_BAR, label, x + 4, y + 6, 0xFFE0D080));
                } else {
                    out.addAll(iconCommands);
                }
                if (skills.isOnCooldown(i, nowMillis)) {
                    appendCooldown(out, x, y);
                }
                continue;
            }

            InventoryItem item = (nativeHotbar != null && i < nativeHotbar.size())
                ? nativeHotbar.get(i)
                : null;
            if (item == null) continue;

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

    private static boolean activeSlot(CastState cast, CastState.Source source, int slot) {
        return cast != null && !cast.isIdle() && cast.source() == source && cast.slot() == slot;
    }

    private static void appendSlotFrame(List<HudRenderCommand> out, int x, int y,
                                        boolean selected, boolean casting, CastState cast) {
        int tint = casting && cast.phase() == CastState.Phase.INTERRUPT ? 0xFFFFA090 : 0xFFFFFFFF;
        out.add(HudRenderCommand.svg(HudRenderLayer.QUICK_BAR, selected || casting ? "selected" : "slot",
            x - 1, y - 2, SLOT_SIZE + 2, SLOT_SIZE + 4, tint));
    }

    private static void appendCooldown(List<HudRenderCommand> out, int x, int y) {
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x + ICON_INSET, y + ICON_INSET,
            SLOT_SIZE - 2 * ICON_INSET, SLOT_SIZE - 2 * ICON_INSET, COOLDOWN_OVERLAY_COLOR));
    }
}
