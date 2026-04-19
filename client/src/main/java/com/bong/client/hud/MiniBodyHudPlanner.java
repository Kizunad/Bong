package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.BodyPartState;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.WoundLevel;

import java.util.ArrayList;
import java.util.List;

/**
 * Left-bottom mini-body + qi / stamina vertical bars (§2.1).
 *
 * <p>Anchored to the screen's bottom-left corner (keeps consistent offset
 * regardless of MC GUI scale). Emits a deterministic list of rects + text so it
 * is trivially unit-testable without touching the Minecraft draw context.
 */
public final class MiniBodyHudPlanner {
    static final int MARGIN_X = 6;
    static final int MARGIN_Y = 6;
    // §2.1 mini body 整体缩到 1/2 尺寸（140×160 → 70×80）。
    static final int PANEL_W = 70;
    static final int PANEL_H = 80;
    static final int PANEL_BG_COLOR = 0x52000000; // opacity 0.32

    // Silhouette layout (40×75 logical box).
    static final int BODY_X_OFFSET = 3;
    static final int BODY_Y_OFFSET = 3;
    static final int BODY_W = 30;
    static final int BODY_H = 75;
    static final int BODY_COLOR = 0xCC808080;

    // Vertical bars (8×65 each, to the right of silhouette).
    static final int BAR_W = 8;
    static final int BAR_H = 65;
    static final int BAR_GAP = 2;
    static final int BAR_X_OFFSET = BODY_X_OFFSET + BODY_W + 4;
    static final int BAR_Y_OFFSET = 9;
    static final int BAR_TRACK_COLOR = 0xCC202020;
    static final int QI_FILL_COLOR = 0xCC40C0E0;
    static final int STAMINA_FILL_COLOR = 0xCCE0C040;
    static final int BAR_FLASH_BORDER_COLOR = 0xFFFF6060;
    static final float LOW_THRESHOLD = 0.15f;

    private MiniBodyHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        CombatHudState hud,
        PhysicalBody body,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (hud == null || !hud.active()) {
            return out;
        }
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int anchorX = MARGIN_X;
        int anchorY = screenHeight - PANEL_H - MARGIN_Y;

        // Panel background
        out.add(HudRenderCommand.rect(
            HudRenderLayer.MINI_BODY,
            anchorX,
            anchorY,
            PANEL_W,
            PANEL_H,
            PANEL_BG_COLOR
        ));

        appendSilhouette(out, anchorX, anchorY, body);
        appendBars(out, anchorX, anchorY, hud, nowMillis);

        return out;
    }

    private static void appendSilhouette(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY,
        PhysicalBody body
    ) {
        int bx = anchorX + BODY_X_OFFSET;
        int by = anchorY + BODY_Y_OFFSET;

        // Head (top circle emulated by square — silhouette stays legible at HUD scale).
        int headSize = 8;
        out.add(HudRenderCommand.rect(
            HudRenderLayer.MINI_BODY,
            bx + (BODY_W - headSize) / 2,
            by,
            headSize,
            headSize,
            BODY_COLOR
        ));

        // Torso
        int torsoX = bx + 9;
        int torsoY = by + 9;
        int torsoW = 12;
        int torsoH = 25;
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, torsoX, torsoY, torsoW, torsoH, BODY_COLOR));

        // Arms
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, bx + 3, by + 10, 5, 22, BODY_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, bx + 22, by + 10, 5, 22, BODY_COLOR));

        // Legs
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, bx + 9, by + 35, 5, 35, BODY_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, bx + 16, by + 35, 5, 35, BODY_COLOR));

        if (body == null) {
            return;
        }

        for (BodyPart part : BodyPart.values()) {
            BodyPartState state = body.part(part);
            if (state == null) continue;
            WoundLevel level = state.wound();
            if (level == null || level == WoundLevel.INTACT) continue;

            int[] pos = locatePart(bx, by, part);
            int dotSize = dotSizeFor(level);
            int dotColor = dotColorFor(level);
            out.add(HudRenderCommand.rect(
                HudRenderLayer.MINI_BODY,
                pos[0] - dotSize / 2,
                pos[1] - dotSize / 2,
                dotSize,
                dotSize,
                dotColor
            ));
        }
    }

    // Wound marker positions (relative to silhouette top-left). 全部按 1/2 缩放。
    private static int[] locatePart(int bx, int by, BodyPart part) {
        return switch (part) {
            case HEAD -> new int[]{bx + BODY_W / 2, by + 4};
            case NECK -> new int[]{bx + BODY_W / 2, by + 9};
            case CHEST -> new int[]{bx + BODY_W / 2, by + 17};
            case ABDOMEN -> new int[]{bx + BODY_W / 2, by + 28};
            case LEFT_UPPER_ARM -> new int[]{bx + 6, by + 14};
            case LEFT_FOREARM -> new int[]{bx + 6, by + 23};
            case LEFT_HAND -> new int[]{bx + 6, by + 31};
            case RIGHT_UPPER_ARM -> new int[]{bx + 24, by + 14};
            case RIGHT_FOREARM -> new int[]{bx + 24, by + 23};
            case RIGHT_HAND -> new int[]{bx + 24, by + 31};
            case LEFT_THIGH -> new int[]{bx + 11, by + 41};
            case LEFT_CALF -> new int[]{bx + 11, by + 54};
            case LEFT_FOOT -> new int[]{bx + 11, by + 66};
            case RIGHT_THIGH -> new int[]{bx + 18, by + 41};
            case RIGHT_CALF -> new int[]{bx + 18, by + 54};
            case RIGHT_FOOT -> new int[]{bx + 18, by + 66};
        };
    }

    private static int dotSizeFor(WoundLevel level) {
        return switch (level) {
            case INTACT -> 0;
            case BRUISE -> 2;
            case ABRASION -> 3;
            case LACERATION -> 5;
            case FRACTURE -> 4;
            case SEVERED -> 6;
        };
    }

    private static int dotColorFor(WoundLevel level) {
        return switch (level) {
            case INTACT -> 0;
            case BRUISE -> 0xFFC08040;
            case ABRASION -> 0xFFFFCC40;
            case LACERATION -> 0xFFFF4040;
            case FRACTURE -> 0xFFA01818;
            case SEVERED -> 0xFF303030;
        };
    }

    private static void appendBars(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY,
        CombatHudState hud,
        long nowMillis
    ) {
        int qiX = anchorX + BAR_X_OFFSET;
        int staminaX = qiX + BAR_W + BAR_GAP;
        int barTop = anchorY + BAR_Y_OFFSET;

        appendBar(out, qiX, barTop, hud.qiPercent(), QI_FILL_COLOR, nowMillis);
        appendBar(out, staminaX, barTop, hud.staminaPercent(), STAMINA_FILL_COLOR, nowMillis);
    }

    private static void appendBar(
        List<HudRenderCommand> out,
        int x,
        int topY,
        float fillRatio,
        int fillColor,
        long nowMillis
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, x, topY, BAR_W, BAR_H, BAR_TRACK_COLOR));

        int fillHeight = Math.max(0, Math.min(BAR_H, Math.round(fillRatio * BAR_H)));
        if (fillHeight > 0) {
            int fillY = topY + (BAR_H - fillHeight);
            out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, x, fillY, BAR_W, fillHeight, fillColor));
        }

        // Low-threshold border flash: 500ms on / 500ms off blink.
        if (fillRatio < LOW_THRESHOLD && ((nowMillis / 500L) & 1L) == 0L) {
            appendBorder(out, x, topY, BAR_W, BAR_H, BAR_FLASH_BORDER_COLOR);
        }
    }

    private static void appendBorder(
        List<HudRenderCommand> out,
        int x,
        int y,
        int w,
        int h,
        int color
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, x, y, w, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, x, y + h - 1, w, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, x, y, 1, h, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, x + w - 1, y, 1, h, color));
    }
}
