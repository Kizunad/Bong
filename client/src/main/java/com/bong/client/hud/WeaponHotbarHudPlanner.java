package com.bong.client.hud;

import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.WeaponEquippedStore;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-weapon-v1 §4.3：main_hand / off_hand 武器槽,贴 hotbar 左右两端。
 *
 * <p>QuickBarHudPlanner 已经画好双层 1-9 + F1-F9,本 planner 只负责武器槽。
 * 槽位内容:
 * <ul>
 *   <li>紫色边框(plan §2.2 法宝/武器标识)</li>
 *   <li>weapon_kind 汉字(剑/刀/杖/拳/枪/匕/弓)</li>
 *   <li>底部 durability 条(红→黑渐变)</li>
 * </ul>
 *
 * <p>空 slot 不渲染(plan §1.4:未解锁 / 未装备一律不画)。
 */
public final class WeaponHotbarHudPlanner {
    static final int SLOT_W = 24;
    static final int SLOT_GAP_TO_HOTBAR = 4;
    static final int BG_COLOR = 0xC0000000;
    static final int BORDER_COLOR = 0xFFC040FF;              // 紫:武器/法宝边框
    static final int DURABILITY_BG_COLOR = 0xFF330000;
    static final int DURABILITY_FG_FULL_COLOR = 0xFF60C040;  // 绿:> 50%
    static final int DURABILITY_FG_MID_COLOR = 0xFFC0A040;   // 黄:20-50%
    static final int DURABILITY_FG_LOW_COLOR = 0xFFC04040;   // 红:< 20%
    static final int DURABILITY_H = 2;
    static final int GLYPH_COLOR = 0xFFFFFFFF;

    private WeaponHotbarHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int hotbarWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        int hotbarLeftX = (screenWidth - hotbarWidth) / 2;

        int lowerY = screenHeight - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN - QuickBarHudPlanner.SLOT_SIZE;
        int upperY = lowerY - QuickBarHudPlanner.SLOT_SIZE - QuickBarHudPlanner.UPPER_GAP;
        int totalHeight = (lowerY + QuickBarHudPlanner.SLOT_SIZE) - upperY;

        EquippedWeapon mainHand = WeaponEquippedStore.get("main_hand");
        if (mainHand != null) {
            int x = hotbarLeftX - SLOT_GAP_TO_HOTBAR - SLOT_W;
            drawWeaponSlot(out, x, upperY, totalHeight, mainHand);
        }
        EquippedWeapon offHand = WeaponEquippedStore.get("off_hand");
        if (offHand != null) {
            int x = hotbarLeftX + hotbarWidth + SLOT_GAP_TO_HOTBAR;
            drawWeaponSlot(out, x, upperY, totalHeight, offHand);
        } else {
            // plan-shield-block-v1 §P3：盾牌 off_hand 槽优先于宝器槽（盾牌不是武器，独立 store）
            EquippedShield offHandShield = EquippedShieldStore.snapshot();
            if (offHandShield != null) {
                int x = hotbarLeftX + hotbarWidth + SLOT_GAP_TO_HOTBAR;
                drawShieldSlot(out, x, upperY, totalHeight, offHandShield);
            } else {
                EquippedTreasure offHandTreasure = TreasureEquippedStore.get("off_hand");
                if (offHandTreasure != null) {
                    int x = hotbarLeftX + hotbarWidth + SLOT_GAP_TO_HOTBAR;
                    drawTreasureSlot(out, x, upperY, totalHeight, offHandTreasure);
                }
            }
        }
        // plan-layered-equip-v1 P4（决议 #17）：two_hand 专槽删除——双手武器走 main_hand held（上方已渲染），
        // 不再单独查 two_hand 槽。

        return out;
    }

    private static void drawWeaponSlot(
        List<HudRenderCommand> out, int x, int y, int totalH, EquippedWeapon w
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, SLOT_W, totalH, BG_COLOR));
        appendBorder(out, x, y, SLOT_W, totalH, BORDER_COLOR);

        String glyph = kindToGlyph(w.weaponKind());
        // 居中:glyph ~ 6px 宽,放 slot center
        out.add(HudRenderCommand.text(
            HudRenderLayer.QUICK_BAR, glyph,
            x + SLOT_W / 2 - 3, y + totalH / 2 - 4,
            GLYPH_COLOR
        ));

        int durY = y + totalH - DURABILITY_H - 2;
        int durTrackW = SLOT_W - 4;
        int durFilledW = Math.round(w.durabilityRatio() * durTrackW);
        int durColor = durabilityColor(w.durabilityRatio());
        out.add(HudRenderCommand.rect(
            HudRenderLayer.QUICK_BAR, x + 2, durY, durTrackW, DURABILITY_H, DURABILITY_BG_COLOR
        ));
        if (durFilledW > 0) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.QUICK_BAR, x + 2, durY, durFilledW, DURABILITY_H, durColor
            ));
        }
    }

    /**
     * plan-shield-block-v1 §P3：渲染 off_hand 盾牌槽（耐久条 + 盾字标识）。
     * 耐久条逻辑与武器槽一致；外框采用蓝色区分（盾非武器）。
     */
    private static void drawShieldSlot(
        List<HudRenderCommand> out, int x, int y, int totalH, EquippedShield shield
    ) {
        final int SHIELD_BORDER_COLOR = 0xFF4080C0; // 蓝：盾牌专属边框
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, SLOT_W, totalH, BG_COLOR));
        appendBorder(out, x, y, SLOT_W, totalH, SHIELD_BORDER_COLOR);

        out.add(HudRenderCommand.text(
            HudRenderLayer.QUICK_BAR, "盾",
            x + SLOT_W / 2 - 3, y + totalH / 2 - 4,
            GLYPH_COLOR
        ));

        int durY = y + totalH - DURABILITY_H - 2;
        int durTrackW = SLOT_W - 4;
        int durFilledW = Math.round(shield.durabilityRatio() * durTrackW);
        int durColor = durabilityColor(shield.durabilityRatio());
        out.add(HudRenderCommand.rect(
            HudRenderLayer.QUICK_BAR, x + 2, durY, durTrackW, DURABILITY_H, DURABILITY_BG_COLOR
        ));
        if (durFilledW > 0) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.QUICK_BAR, x + 2, durY, durFilledW, DURABILITY_H, durColor
            ));
        }
    }

    private static void drawTreasureSlot(
        List<HudRenderCommand> out, int x, int y, int totalH, EquippedTreasure treasure
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, SLOT_W, totalH, BG_COLOR));
        appendBorder(out, x, y, SLOT_W, totalH, BORDER_COLOR);
        out.add(HudRenderCommand.itemTexture(
            HudRenderLayer.QUICK_BAR,
            treasure.templateId(),
            x + 2,
            y + totalH / 2 - 10,
            SLOT_W - 4
        ));
        out.add(HudRenderCommand.text(
            HudRenderLayer.QUICK_BAR,
            "宝",
            x + SLOT_W / 2 - 3,
            y + 4,
            GLYPH_COLOR
        ));
    }

    static int durabilityColor(float ratio) {
        if (ratio >= 0.5f) return DURABILITY_FG_FULL_COLOR;
        if (ratio >= 0.2f) return DURABILITY_FG_MID_COLOR;
        return DURABILITY_FG_LOW_COLOR;
    }

    static String kindToGlyph(String kind) {
        if (kind == null) return "?";
        return switch (kind) {
            case "sword" -> "剑";
            case "saber" -> "刀";
            case "staff" -> "杖";
            case "fist" -> "拳";
            case "spear" -> "枪";
            case "dagger" -> "匕";
            case "bow" -> "弓";
            default -> "?";
        };
    }

    private static void appendBorder(List<HudRenderCommand> out, int x, int y, int w, int h, int c) {
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, w, 1, c));
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y + h - 1, w, 1, c));
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, 1, h, c));
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x + w - 1, y, 1, h, c));
    }
}
