package com.bong.client.hud;

import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.TreasurePanelSync;
import com.bong.client.combat.WeaponEquippedStore;

import java.util.ArrayList;
import java.util.List;

/** 双排快捷栏两侧的持械印记：SVG 框与耐久刻痕承载真实装备 PNG。 */
public final class WeaponHotbarHudPlanner {
    static final int SLOT_W = 30;
    static final int SLOT_H = 44;
    static final int SLOT_GAP_TO_HOTBAR = 6;
    private static final int WEAPON_COLOR = 0xFFD4D6BD;
    private static final int SHIELD_COLOR = 0xFF96C6C0;
    private static final int TREASURE_COLOR = 0xFFE0BD81;

    private WeaponHotbarHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        List<HudRenderCommand> out = new ArrayList<>();
        int hotbarLeft = QuickBarHudPlanner.rowLeftX(screenWidth, QuickBarHudPlanner.TOTAL_SLOTS);
        int hotbarWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        int y = screenHeight - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN
            - QuickBarHudPlanner.SLOT_SIZE * 2 - QuickBarHudPlanner.UPPER_GAP;
        if (y < 4) return out;

        int left = hotbarLeft - SLOT_GAP_TO_HOTBAR - SLOT_W;
        EquippedWeapon mainHand = WeaponEquippedStore.get("main_hand");
        if (left >= 4 && isHudWeapon(mainHand)) {
            drawSlot(out, left, y, true, mainHand.templateId(), WEAPON_COLOR, mainHand.durabilityRatio());
        }

        int right = hotbarLeft + hotbarWidth + SLOT_GAP_TO_HOTBAR;
        if (right + SLOT_W > screenWidth - 4) return out;
        EquippedWeapon offHand = WeaponEquippedStore.get("off_hand");
        EquippedShield shield = EquippedShieldStore.snapshot();
        if (isHudWeapon(offHand)) {
            drawSlot(out, right, y, false, offHand.templateId(), WEAPON_COLOR, offHand.durabilityRatio());
        } else if (shield != null) {
            drawSlot(out, right, y, false, shield.templateId(), SHIELD_COLOR, shield.durabilityRatio());
        } else {
            // 副手持械法宝优先；无持械法宝时仍展示首个触发位，沿用既有装备契约。
            EquippedTreasure treasure = TreasureEquippedStore.get("off_hand");
            if (treasure == null) treasure = firstTriggerTreasure();
            if (treasure != null) {
                drawSlot(out, right, y, false, treasure.templateId(), TREASURE_COLOR, null);
            }
        }
        return out;
    }

    private static boolean isHudWeapon(EquippedWeapon weapon) {
        return weapon != null && !"tool".equals(weapon.weaponKind());
    }

    private static EquippedTreasure firstTriggerTreasure() {
        for (int i = 0; i < TreasurePanelSync.TREASURE_TRIGGER_CAP; i++) {
            EquippedTreasure treasure = TreasureEquippedStore.get(TreasurePanelSync.triggerSlotKey(i));
            if (treasure != null) return treasure;
        }
        return null;
    }

    private static void drawSlot(List<HudRenderCommand> out, int x, int y, boolean mainHand,
                                 String templateId, int accent, Float durability) {
        out.add(HudRenderCommand.svg(HudRenderLayer.QUICK_BAR,
            mainHand ? "hand-left" : "hand-right", x, y, SLOT_W, SLOT_H, 0xFFFFFFFF));
        out.add(HudRenderCommand.svg(HudRenderLayer.QUICK_BAR, "hand-crest",
            x + 8, y + 1, 14, 7, accent));
        out.add(HudRenderCommand.itemTexture(HudRenderLayer.QUICK_BAR, templateId, x + 3, y + 9, 24));
        if (durability == null) {
            out.add(HudRenderCommand.svg(HudRenderLayer.QUICK_BAR, "hand-crest",
                x + 8, y + 35, 14, 7, accent));
            return;
        }

        float ratio = Float.isFinite(durability) ? Math.max(0, Math.min(1, durability)) : 0;
        int color = ratio < .2f ? 0xFFDF796C : ratio < .5f ? 0xFFD1B475 : 0xFFA4C4A6;
        for (int i = 0; i < 6; i++) {
            float filled = Math.max(0, Math.min(1, ratio * 6 - i));
            int alpha = Math.round(36 + 219 * filled);
            out.add(HudRenderCommand.svg(HudRenderLayer.QUICK_BAR, "hand-tick",
                x + 3 + i * 4, y + 36, 4, 5, (alpha << 24) | (color & 0xFFFFFF)));
        }
        if (ratio < .2f) {
            out.add(HudRenderCommand.svg(HudRenderLayer.QUICK_BAR, "hand-fracture",
                x, y, SLOT_W, SLOT_H, 0xE6DF796C));
        }
    }
}
