package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.ArmorProfileStore;
import com.bong.client.fauna.HallucinationHudOverlay;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.artifact.ArtifactState;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.BodyPartState;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.WoundLevel;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.model.bodyplan.PartAnchor;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.bong.client.state.SeasonState;
import com.bong.client.visual.season.SeasonVisuals;

import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * 左下人体与真元、体力条（§2.1）；窄屏自动避开快捷栏。
 *
 * <p>按当前快照投影逻辑图形、尺寸和颜色，不依赖 SVG 解析器或具体 UI 库。
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
    static final int ARTIFACT_INDICATOR_SIZE = 3;
    static final int ARTIFACT_INDICATOR_COLOR_FALLBACK = 0xFF808080;

    // plan-armor-v1 §5：破损护甲裂纹提示（同 layer，靠命令顺序实现 wound dot 覆盖）。
    static final int BROKEN_ARMOR_CRACK_COLOR = 0xFFB0B0B0;
    static final int BODY_PART_RESIST_FRAME_COLOR = 0xFF409CFF;
    static final int BODY_PART_WEAKEN_FRAME_COLOR = 0xFFFF5050;

    private MiniBodyHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        CombatHudState hud,
        PhysicalBody body,
        Map<EquipSlotType, InventoryItem> equipped,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(hud, body, equipped, nowMillis, screenWidth, screenHeight, null);
    }

    public static List<HudRenderCommand> buildCommands(
        CombatHudState hud,
        PhysicalBody body,
        Map<EquipSlotType, InventoryItem> equipped,
        long nowMillis,
        int screenWidth,
        int screenHeight,
        SeasonState seasonState
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (hud == null || !hud.active()) {
            return out;
        }
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int anchorX = MARGIN_X;
        int anchorY = anchorY(screenWidth, screenHeight);

        // Panel background
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill",
            anchorX,
            anchorY,
            PANEL_W,
            PANEL_H,
            PANEL_BG_COLOR
        ));

        appendSilhouette(out, anchorX, anchorY);
        appendBrokenArmorCracks(out, anchorX, anchorY, equipped);
        appendArtifactIndicator(out, anchorX, anchorY, equipped);
        appendWoundDots(out, anchorX, anchorY, body);
        appendCombatPillPartFrames(out, anchorX, anchorY);
        appendBars(out, anchorX, anchorY, hud, nowMillis, seasonState);

        return out;
    }

    /** 窄屏把人体抬到快捷栏上方；保持伤势锚点和条形比例不变。 */
    static int anchorY(int screenWidth, int screenHeight) {
        int quickWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        int leftSlot = (screenWidth - quickWidth) / 2
            - WeaponHotbarHudPlanner.SLOT_GAP_TO_HOTBAR - WeaponHotbarHudPlanner.SLOT_W;
        int bottom = screenHeight - MARGIN_Y;
        if (leftSlot < MARGIN_X + PANEL_W + MARGIN_X) {
            bottom = screenHeight - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN
                - 2 * QuickBarHudPlanner.SLOT_SIZE - QuickBarHudPlanner.UPPER_GAP - MARGIN_Y;
        }
        return Math.max(MARGIN_Y, bottom - PANEL_H);
    }

    private static void appendSilhouette(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY
    ) {
        int bx = anchorX + BODY_X_OFFSET;
        int by = anchorY + BODY_Y_OFFSET;

        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "body",
            bx, by, BODY_W, BODY_H, BODY_COLOR));
    }

    private static void appendWoundDots(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY,
        PhysicalBody body
    ) {
        if (body == null) return;

        int bx = anchorX + BODY_X_OFFSET;
        int by = anchorY + BODY_Y_OFFSET;

        for (BodyPart part : BodyPart.values()) {
            BodyPartState state = body.part(part);
            if (state == null) continue;
            WoundLevel level = state.wound();
            if (level == null || level == WoundLevel.INTACT) continue;

            int[] pos = locatePart(bx, by, part);
            int dotSize = dotSizeFor(level);
            int dotColor = dotColorFor(level);
            out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill",
                pos[0] - dotSize / 2,
                pos[1] - dotSize / 2,
                dotSize,
                dotSize,
                dotColor
            ));
        }
    }

    private static void appendCombatPillPartFrames(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY
    ) {
        EnumSet<BodyPart> resistParts = EnumSet.noneOf(BodyPart.class);
        EnumSet<BodyPart> weakenParts = EnumSet.noneOf(BodyPart.class);
        for (StatusEffectStore.Effect effect : StatusEffectStore.snapshot()) {
            addPartsFromStatus(effect.id(), "body_part_resist:", resistParts);
            addPartsFromStatus(effect.id(), "body_part_weaken:", weakenParts);
        }
        if (resistParts.isEmpty() && weakenParts.isEmpty()) {
            return;
        }

        int bx = anchorX + BODY_X_OFFSET;
        int by = anchorY + BODY_Y_OFFSET;
        for (BodyPart part : resistParts) {
            int[] pos = locatePart(bx, by, part);
            appendThickFrame(out, pos[0], pos[1], BODY_PART_RESIST_FRAME_COLOR);
        }
        for (BodyPart part : weakenParts) {
            int[] pos = locatePart(bx, by, part);
            appendDashedFrame(out, pos[0], pos[1], BODY_PART_WEAKEN_FRAME_COLOR);
        }
    }

    private static void addPartsFromStatus(String id, String prefix, EnumSet<BodyPart> out) {
        if (id == null || !id.startsWith(prefix)) {
            return;
        }
        switch (id.substring(prefix.length())) {
            case "head" -> {
                out.add(BodyPart.HEAD);
                out.add(BodyPart.NECK);
            }
            case "chest" -> out.add(BodyPart.CHEST);
            case "abdomen" -> out.add(BodyPart.ABDOMEN);
            case "arm_l" -> {
                out.add(BodyPart.LEFT_UPPER_ARM);
                out.add(BodyPart.LEFT_FOREARM);
                out.add(BodyPart.LEFT_HAND);
            }
            case "arm_r" -> {
                out.add(BodyPart.RIGHT_UPPER_ARM);
                out.add(BodyPart.RIGHT_FOREARM);
                out.add(BodyPart.RIGHT_HAND);
            }
            case "leg_l" -> {
                out.add(BodyPart.LEFT_THIGH);
                out.add(BodyPart.LEFT_CALF);
                out.add(BodyPart.LEFT_FOOT);
            }
            case "leg_r" -> {
                out.add(BodyPart.RIGHT_THIGH);
                out.add(BodyPart.RIGHT_CALF);
                out.add(BodyPart.RIGHT_FOOT);
            }
            default -> {
            }
        }
    }

    private static void appendThickFrame(List<HudRenderCommand> out, int cx, int cy, int color) {
        appendBorder(out, cx - 4, cy - 4, 8, 8, color);
        appendBorder(out, cx - 3, cy - 3, 6, 6, color);
    }

    private static void appendDashedFrame(List<HudRenderCommand> out, int cx, int cy, int color) {
        int x = cx - 4;
        int y = cy - 4;
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y, 3, 1, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x + 5, y, 3, 1, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y + 7, 3, 1, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x + 5, y + 7, 3, 1, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y, 1, 3, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y + 5, 1, 3, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x + 7, y, 1, 3, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x + 7, y + 5, 1, 3, color));
    }

    private static void appendBrokenArmorCracks(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY,
        Map<EquipSlotType, InventoryItem> equipped
    ) {
        if (equipped == null || equipped.isEmpty()) return;

        EnumSet<BodyPart> cracked = EnumSet.noneOf(BodyPart.class);
        if (isBrokenArmor(equipped.get(EquipSlotType.HEAD))) {
            cracked.add(BodyPart.HEAD);
        }
        if (isBrokenArmor(equipped.get(EquipSlotType.CHEST))) {
            cracked.add(BodyPart.CHEST);
            cracked.add(BodyPart.ABDOMEN);
        }
        if (isBrokenArmor(equipped.get(EquipSlotType.LEGS))) {
            cracked.add(BodyPart.LEFT_THIGH);
            cracked.add(BodyPart.LEFT_CALF);
            cracked.add(BodyPart.RIGHT_THIGH);
            cracked.add(BodyPart.RIGHT_CALF);
        }
        if (isBrokenArmor(equipped.get(EquipSlotType.FEET))) {
            cracked.add(BodyPart.LEFT_FOOT);
            cracked.add(BodyPart.RIGHT_FOOT);
        }

        if (cracked.isEmpty()) return;

        int bx = anchorX + BODY_X_OFFSET;
        int by = anchorY + BODY_Y_OFFSET;
        for (BodyPart part : cracked) {
            int[] pos = locatePart(bx, by, part);
            appendCrackGlyph(out, pos[0], pos[1]);
        }
    }

    private static void appendArtifactIndicator(
        List<HudRenderCommand> out,
        int anchorX,
        int anchorY,
        Map<EquipSlotType, InventoryItem> equipped
    ) {
        if (equipped == null || equipped.isEmpty()) return;

        InventoryItem item = equipped.get(EquipSlotType.MAIN_HAND);
        if (item == null || item.isEmpty()) {
            item = equipped.get(EquipSlotType.OFF_HAND);
        }
        if (item == null || item.isEmpty()) {
            return;
        }

        ArtifactState artifact = item.artifactState().orElse(null);
        if (artifact == null) {
            return;
        }

        int x = anchorX + BODY_X_OFFSET + BODY_W + 1;
        int y = anchorY + BODY_Y_OFFSET + 1;
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill",
            x,
            y,
            ARTIFACT_INDICATOR_SIZE,
            ARTIFACT_INDICATOR_SIZE,
            artifact.indicatorColor() == 0 ? ARTIFACT_INDICATOR_COLOR_FALLBACK : artifact.indicatorColor()
        ));
    }

    private static boolean isBrokenArmor(InventoryItem item) {
        if (item == null || item.isEmpty()) return false;
        return ArmorProfileStore.isArmor(item.itemId()) && item.durability() <= 0.0;
    }

    private static void appendCrackGlyph(List<HudRenderCommand> out, int cx, int cy) {
        // A tiny zigzag, sized for 1/2 mini-body scale. Pure rects keeps planner testable.
        int[][] pts = new int[][]{
            {0, -3},
            {1, -2},
            {0, -1},
            {-1, 0},
            {0, 1},
            {1, 2},
            {0, 3},
            {-2, 1},
            {2, -1}
        };
        for (int[] p : pts) {
            out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill",
                cx + p[0],
                cy + p[1],
                1,
                1,
                BROKEN_ARMOR_CRACK_COLOR
            ));
        }
    }

    // Wound marker positions (relative to silhouette top-left).
    //
    // plan-race-system-v1 P2 major 修复 —— 三级换轨，按优先级：
    //   1. layout.hudAnchorFor(part)：mini HUD 专用第二锚点组（本面板 30×75 粗网格，
    //      宽高比 0.40），humanoid.json 的 hud_anchors 原样抽取自本面板改造前的硬编码
    //      表，走这条路径与旧表逐像素相等（见 MiniBodyHudPlannerGeometryTest）。
    //   2. layout.anchorFor(part)：主锚点组（BodyInspectComponent 168×236 精细画布，
    //      宽高比 0.71）按本面板 BODY_W×BODY_H 线性缩放推导——仅当该 layout 没有配置
    //      hud_anchors 时才走这条路径（未来非人 plan 的常态，没有另一份权威 mini HUD
    //      像素表可抽取，缩放推导是唯一选择，可能有几像素漂移，可接受）。
    //   3. fallbackLocatePart：store 缺当前 layout，或 layout 两组锚点都未声明该部位
    //      时的仅视觉 fallback（本面板改造前的原硬编码表，逐值保留）。
    private static int[] locatePart(int bx, int by, BodyPart part) {
        BodyPlanLayout layout = BodyPlanLayoutStore.current();
        if (layout != null) {
            String partId = part.name().toLowerCase(Locale.ROOT);
            PartAnchor hudAnchor = layout.hudAnchorFor(partId);
            if (hudAnchor != null) {
                return scaledPoint(bx, by, hudAnchor);
            }
            PartAnchor anchor = layout.anchorFor(partId);
            if (anchor != null) {
                return scaledPoint(bx, by, anchor);
            }
        }
        return fallbackLocatePart(bx, by, part);
    }

    private static int[] scaledPoint(int bx, int by, PartAnchor anchor) {
        int px = (int) Math.round(anchor.point().x() * BODY_W);
        int py = (int) Math.round(anchor.point().y() * BODY_H);
        return new int[]{bx + px, by + py};
    }

    /** 内建常量保底（layout 缺失 / 未声明该部位时的仅视觉 fallback）。全部按 1/2 缩放。 */
    static int[] fallbackLocatePart(int bx, int by, BodyPart part) {
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
        long nowMillis,
        SeasonState seasonState
    ) {
        int qiX = anchorX + BAR_X_OFFSET;
        int staminaX = qiX + BAR_W + BAR_GAP;
        int barTop = anchorY + BAR_Y_OFFSET;

        // plan-fauna-stitched-beast-v1 P3 M3 修复：幻觉激活时对 qi/stamina bar 显示值施加偏移。
        // 守恒红线：仅改 displayRatio，绝不写回 hud.qiPercent() / hud.staminaPercent() 实际值。
        float qiDisplayOffset = HallucinationHudOverlay.getQiBarDisplayOffset();
        float staminaDisplayOffset = HallucinationHudOverlay.getHpBarDisplayOffset(); // stamina 借用 hp offset
        float qiDisplayRatio = Math.max(0f, Math.min(1f, hud.qiPercent() * (1.0f + qiDisplayOffset)));
        float staminaDisplayRatio = Math.max(0f, Math.min(1f, hud.staminaPercent() * (1.0f + staminaDisplayOffset)));

        appendBar(out, qiX, barTop, qiDisplayRatio, SeasonVisuals.qiBarColor(QI_FILL_COLOR, seasonState, nowMillis), nowMillis);
        appendBar(out, staminaX, barTop, staminaDisplayRatio, STAMINA_FILL_COLOR, nowMillis);
    }

    private static void appendBar(
        List<HudRenderCommand> out,
        int x,
        int topY,
        float fillRatio,
        int fillColor,
        long nowMillis
    ) {
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, topY, BAR_W, BAR_H, BAR_TRACK_COLOR));

        int fillHeight = Math.max(0, Math.min(BAR_H, Math.round(fillRatio * BAR_H)));
        if (fillHeight > 0) {
            int fillY = topY + (BAR_H - fillHeight);
            out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, fillY, BAR_W, fillHeight, fillColor));
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
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y, w, 1, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y + h - 1, w, 1, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x, y, 1, h, color));
        out.add(HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "fill", x + w - 1, y, 1, h, color));
    }

    // ==================== Test-only geometry accessors ====================
    // plan-race-system-v1 P2b — locatePart 是 private static，像素回归 pin 测试需要
    // 直接核验其输出（含 BodyPlanLayoutStore 已加载 / 未加载两条路径）。

    static int[] locatePartForTests(int bx, int by, BodyPart part) { return locatePart(bx, by, part); }
}
