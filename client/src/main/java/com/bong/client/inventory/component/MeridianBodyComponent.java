package com.bong.client.inventory.component;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;

/**
 * 人体经脉剪影组件 — 像素风人体 + 经脉线条 + 丹田节点。
 * 经脉颜色按损伤状态变化，支持 hover 查看详情。
 */
public class MeridianBodyComponent extends BaseComponent {
    private static final int W = 140;
    private static final int H = 175;

    // 人体剪影颜色
    private static final int BODY_COLOR = 0x88222233;

    // 丹田颜色
    private static final int DANTIAN_GLOW = 0xCC3388DD;

    private MeridianBody body;
    private MeridianChannel hoveredChannel;
    private long tickCount;

    public MeridianBodyComponent() {
        this.sizing(Sizing.fixed(W), Sizing.fixed(H));
    }

    public void setBody(MeridianBody body) {
        this.body = body;
    }

    public MeridianBody body() { return body; }

    public MeridianChannel hoveredChannel() { return hoveredChannel; }

    @Override
    public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY, float partialTicks, float delta) {
        tickCount++;
        int bx = x, by = y; // component origin

        // Background
        ctx.fill(bx, by, bx + W, by + H, 0xCC111118);

        if (body == null) {
            var tr = MinecraftClient.getInstance().textRenderer;
            ctx.drawTextWithShadow(tr, Text.literal("§7无经脉数据"), bx + 30, by + 80, 0xFF666666);
            return;
        }

        int cx = bx + W / 2; // body center X

        // === Draw body silhouette ===
        drawBodySilhouette(ctx, cx, by);

        // === Draw meridian lines ===
        hoveredChannel = null;
        for (MeridianChannel ch : MeridianChannel.values()) {
            ChannelState cs = body.channel(ch);
            if (cs == null) continue;
            boolean isHover = isMouseOverMeridian(ch, mouseX - bx, mouseY - by);
            if (isHover) hoveredChannel = ch;
            drawMeridian(ctx, cx, by, ch, cs, isHover);
        }

        // === Draw dantian nodes ===
        drawDantians(ctx, cx, by);

        // === Draw status effects ===
        drawStatusEffects(ctx, bx, by);

        // Tooltip is drawn by InspectScreen.render() to avoid owo-lib clipping
    }

    // ==================== Body Silhouette ====================

    private void drawBodySilhouette(OwoUIDrawContext ctx, int cx, int by) {
        // Head (inner fill + outer outline ring)
        fillCircle(ctx, cx, by + 14, 10, 0x44666688);
        fillCircle(ctx, cx, by + 14, 9, BODY_COLOR);

        // Neck
        ctx.fill(cx - 3, by + 23, cx + 3, by + 28, BODY_COLOR);

        // Shoulders
        ctx.fill(cx - 28, by + 28, cx + 28, by + 34, BODY_COLOR);

        // Torso
        ctx.fill(cx - 18, by + 34, cx + 18, by + 82, BODY_COLOR);

        // Left upper arm
        ctx.fill(cx - 28, by + 30, cx - 22, by + 60, BODY_COLOR);
        // Left forearm
        ctx.fill(cx - 30, by + 60, cx - 24, by + 90, BODY_COLOR);
        // Left hand
        ctx.fill(cx - 32, by + 88, cx - 23, by + 96, BODY_COLOR);

        // Right upper arm
        ctx.fill(cx + 22, by + 30, cx + 28, by + 60, BODY_COLOR);
        // Right forearm
        ctx.fill(cx + 24, by + 60, cx + 30, by + 90, BODY_COLOR);
        // Right hand
        ctx.fill(cx + 23, by + 88, cx + 32, by + 96, BODY_COLOR);

        // Pelvis
        ctx.fill(cx - 16, by + 82, cx + 16, by + 92, BODY_COLOR);

        // Left thigh
        ctx.fill(cx - 16, by + 92, cx - 6, by + 130, BODY_COLOR);
        // Left calf
        ctx.fill(cx - 15, by + 130, cx - 7, by + 155, BODY_COLOR);
        // Left foot
        ctx.fill(cx - 18, by + 155, cx - 5, by + 160, BODY_COLOR);

        // Right thigh
        ctx.fill(cx + 6, by + 92, cx + 16, by + 130, BODY_COLOR);
        // Right calf
        ctx.fill(cx + 7, by + 130, cx + 15, by + 155, BODY_COLOR);
        // Right foot
        ctx.fill(cx + 5, by + 155, cx + 18, by + 160, BODY_COLOR);
    }

    // ==================== Meridian Rendering ====================

    private void drawMeridian(OwoUIDrawContext ctx, int cx, int by,
                              MeridianChannel ch, ChannelState cs, boolean hover) {
        int color = meridianColor(cs, hover);
        int thickness = hover ? 2 : 1;

        // Contamination overlay
        int contaminationColor = 0;
        if (cs.contamination() > 0.05) {
            int alpha = (int) (cs.contamination() * 180);
            contaminationColor = (alpha << 24) | 0x9944CC;
        }

        switch (ch) {
            case REN_MAI -> {
                // Front center vertical line
                drawLine(ctx, cx - 1, by + 26, cx - 1, by + 85, color, thickness);
            }
            case DU_MAI -> {
                // Back center (offset right)
                drawLine(ctx, cx + 2, by + 26, cx + 2, by + 85, color, thickness);
            }
            case HEART -> {
                // Radial from chest center
                int hx = cx, hy = by + 42;
                drawLine(ctx, hx - 10, hy, hx + 10, hy, color, thickness);
                drawLine(ctx, hx, hy - 5, hx, hy + 5, color, thickness);
                drawLine(ctx, hx - 7, hy - 4, hx + 7, hy + 4, color, thickness);
                drawLine(ctx, hx - 7, hy + 4, hx + 7, hy - 4, color, thickness);
                // Pulsing if micro tear or worse
                if (cs.damage() != ChannelState.DamageLevel.INTACT) {
                    int pulse = (int) (Math.sin(tickCount * 0.15) * 40 + 40);
                    int pulseColor = (pulse << 24) | (cs.damage().color() & 0x00FFFFFF);
                    fillCircle(ctx, hx, hy, 6, pulseColor);
                }
            }
            case SPIRIT -> {
                // In head
                drawLine(ctx, cx, by + 8, cx, by + 20, color, thickness);
                drawLine(ctx, cx - 5, by + 12, cx + 5, by + 12, color, thickness);
            }
            case ARM_YIN -> {
                // Left arm path
                drawLine(ctx, cx - 14, by + 36, cx - 25, by + 55, color, thickness);
                drawLine(ctx, cx - 25, by + 55, cx - 27, by + 90, color, thickness);
                if (contaminationColor != 0) {
                    ctx.fill(cx - 30, by + 45, cx - 22, by + 85, contaminationColor);
                }
            }
            case ARM_YANG -> {
                // Right arm path
                drawLine(ctx, cx + 14, by + 36, cx + 25, by + 55, color, thickness);
                drawLine(ctx, cx + 25, by + 55, cx + 27, by + 90, color, thickness);
                if (contaminationColor != 0) {
                    ctx.fill(cx + 22, by + 45, cx + 30, by + 85, contaminationColor);
                }
            }
            case LEG_YIN -> {
                // Left leg path
                drawLine(ctx, cx - 8, by + 88, cx - 11, by + 130, color, thickness);
                drawLine(ctx, cx - 11, by + 130, cx - 11, by + 155, color, thickness);
            }
            case LEG_YANG -> {
                // Right leg path
                drawLine(ctx, cx + 8, by + 88, cx + 11, by + 130, color, thickness);
                drawLine(ctx, cx + 11, by + 130, cx + 11, by + 155, color, thickness);
            }
            case LUNG -> {
                // V-shape upper chest
                drawLine(ctx, cx - 12, by + 36, cx, by + 42, color, thickness);
                drawLine(ctx, cx, by + 42, cx + 12, by + 36, color, thickness);
            }
            case KIDNEY -> {
                // Lower abdomen, mirrored
                drawLine(ctx, cx - 10, by + 72, cx, by + 80, color, thickness);
                drawLine(ctx, cx, by + 80, cx + 10, by + 72, color, thickness);
            }
            case LIVER -> {
                // Mid abdomen right side
                drawLine(ctx, cx + 4, by + 58, cx + 14, by + 65, color, thickness);
                drawLine(ctx, cx + 14, by + 65, cx + 8, by + 72, color, thickness);
            }
            case SPLEEN -> {
                // Mid abdomen left side
                drawLine(ctx, cx - 4, by + 60, cx - 14, by + 67, color, thickness);
                drawLine(ctx, cx - 14, by + 67, cx - 8, by + 74, color, thickness);
            }
        }
    }

    private int meridianColor(ChannelState cs, boolean hover) {
        if (cs.blocked()) return 0xFF333333;

        int baseColor = cs.damage().color();
        if (hover) {
            // Brighten on hover
            int r = Math.min(255, ((baseColor >> 16) & 0xFF) + 60);
            int g = Math.min(255, ((baseColor >> 8) & 0xFF) + 60);
            int b = Math.min(255, (baseColor & 0xFF) + 60);
            return 0xFF000000 | (r << 16) | (g << 8) | b;
        }

        // Dim based on flow ratio
        double flow = cs.flowRatio();
        int alpha = (int) (100 + flow * 155); // 100~255
        return (alpha << 24) | (baseColor & 0x00FFFFFF);
    }

    // ==================== Dantian Nodes ====================

    private void drawDantians(OwoUIDrawContext ctx, int cx, int by) {
        drawDantianNode(ctx, cx, by + 14, body.dantian(MeridianBody.DantianTier.UPPER));  // 上丹田 (head)
        drawDantianNode(ctx, cx, by + 45, body.dantian(MeridianBody.DantianTier.MIDDLE)); // 中丹田 (chest)
        drawDantianNode(ctx, cx, by + 78, body.dantian(MeridianBody.DantianTier.LOWER));  // 下丹田 (abdomen)
    }

    private void drawDantianNode(OwoUIDrawContext ctx, int nx, int ny, MeridianBody.DantianState ds) {
        if (ds == null) return;

        double ratio = ds.ratio();
        int pulse = (int) (Math.sin(tickCount * 0.08 + ny * 0.1) * 20 + 20);

        // Outer glow
        int glowAlpha = (int) (40 + ratio * 80 + pulse);
        int glowColor = (glowAlpha << 24) | (DANTIAN_GLOW & 0x00FFFFFF);
        fillCircle(ctx, nx, ny, 5, glowColor);

        // Inner core
        int coreAlpha = (int) (120 + ratio * 135);
        int coreColor = (coreAlpha << 24) | (ratio > 0.5 ? 0x4488CC : 0x334466);
        fillCircle(ctx, nx, ny, 3, coreColor);

        // Sealed indicator
        if (ds.sealed()) {
            ctx.fill(nx - 4, ny - 1, nx + 4, ny + 1, 0xCCCC2222);
        }
    }

    // ==================== Status Effects ====================

    private void drawStatusEffects(OwoUIDrawContext ctx, int bx, int by) {
        var effects = body.activeEffects();
        if (effects.isEmpty()) return;

        var tr = MinecraftClient.getInstance().textRenderer;
        int ey = by + H - 2 - effects.size() * (tr.fontHeight + 1);

        for (var effect : effects) {
            ctx.fill(bx + 2, ey - 1, bx + 4, ey + tr.fontHeight - 1, effect.color());
            ctx.drawTextWithShadow(tr, Text.literal("§7" + effect.name()), bx + 6, ey, 0xFF999999);
            ey += tr.fontHeight + 1;
        }
    }

    // ==================== Hover Tooltip ====================

    public void drawMeridianTooltip(DrawContext ctx, int mx, int my) {
        if (hoveredChannel == null || body == null) return;
        MeridianChannel ch = hoveredChannel;
        ChannelState cs = body.channel(ch);
        if (cs == null) return;

        var tr = MinecraftClient.getInstance().textRenderer;

        String line1 = ch.displayName() + " — " + cs.damage().label();
        String line2 = String.format("流量 %.0f/%.0f", cs.currentFlow(), cs.capacity());
        String line3 = cs.contamination() > 0.01
            ? String.format("污染 %.0f%%", cs.contamination() * 100)
            : "";
        String line4 = cs.healProgress() > 0.01
            ? String.format("恢复 %.0f%%", cs.healProgress() * 100)
            : "";

        int lines = 2 + (line3.isEmpty() ? 0 : 1) + (line4.isEmpty() ? 0 : 1);
        int tw = Math.max(tr.getWidth(line1), tr.getWidth(line2)) + 8;
        int th = lines * (tr.fontHeight + 1) + 6;

        int tx = mx + 8;
        int ty = my - th - 4;
        if (ty < 0) ty = my + 12;

        ctx.fill(tx - 2, ty - 2, tx + tw + 2, ty + th + 2, 0xEE111122);
        ctx.fill(tx - 1, ty - 1, tx + tw + 1, ty + th + 1, 0xEE1A1A2A);

        int cy = ty + 2;
        ctx.drawTextWithShadow(tr, Text.literal(line1), tx + 2, cy, cs.damage().color());
        cy += tr.fontHeight + 1;
        ctx.drawTextWithShadow(tr, Text.literal(line2), tx + 2, cy, 0xFFAAAAAA);
        cy += tr.fontHeight + 1;
        if (!line3.isEmpty()) {
            ctx.drawTextWithShadow(tr, Text.literal(line3), tx + 2, cy, 0xFF9944CC);
            cy += tr.fontHeight + 1;
        }
        if (!line4.isEmpty()) {
            ctx.drawTextWithShadow(tr, Text.literal(line4), tx + 2, cy, 0xFF44AA66);
        }
    }

    // ==================== Hit detection for hover ====================

    private boolean isMouseOverMeridian(MeridianChannel ch, int mx, int my) {
        int cx = W / 2;
        int hitRadius = 8;

        return switch (ch) {
            case REN_MAI -> Math.abs(mx - cx + 1) < hitRadius && my > 24 && my < 87;
            case DU_MAI -> Math.abs(mx - cx - 2) < hitRadius && my > 24 && my < 87;
            case HEART -> dist(mx, my, cx, 42) < 12;
            case SPIRIT -> dist(mx, my, cx, 14) < 10;
            case ARM_YIN -> mx < cx - 14 && mx > cx - 35 && my > 30 && my < 95;
            case ARM_YANG -> mx > cx + 14 && mx < cx + 35 && my > 30 && my < 95;
            case LEG_YIN -> mx > cx - 18 && mx < cx - 4 && my > 85 && my < 158;
            case LEG_YANG -> mx > cx + 4 && mx < cx + 18 && my > 85 && my < 158;
            case LUNG -> Math.abs(my - 38) < 8 && Math.abs(mx - cx) < 16 && my < 44;
            case KIDNEY -> Math.abs(my - 76) < 8 && Math.abs(mx - cx) < 14;
            case LIVER -> mx > cx && mx < cx + 18 && my > 55 && my < 75;
            case SPLEEN -> mx < cx && mx > cx - 18 && my > 57 && my < 77;
        };
    }

    private static double dist(int x1, int y1, int x2, int y2) {
        return Math.sqrt((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2));
    }

    // ==================== Drawing primitives ====================

    private static void drawLine(OwoUIDrawContext ctx, int x1, int y1, int x2, int y2, int color, int thickness) {
        int dx = Math.abs(x2 - x1);
        int dy = Math.abs(y2 - y1);
        int steps = Math.max(dx, dy);
        if (steps == 0) {
            ctx.fill(x1, y1, x1 + thickness, y1 + thickness, color);
            return;
        }

        float xInc = (float) (x2 - x1) / steps;
        float yInc = (float) (y2 - y1) / steps;
        float px = x1, py = y1;

        for (int i = 0; i <= steps; i++) {
            ctx.fill((int) px, (int) py, (int) px + thickness, (int) py + thickness, color);
            px += xInc;
            py += yInc;
        }
    }

    private static void fillCircle(OwoUIDrawContext ctx, int cx, int cy, int radius, int color) {
        for (int dy = -radius; dy <= radius; dy++) {
            int halfWidth = (int) Math.sqrt(radius * radius - dy * dy);
            ctx.fill(cx - halfWidth, cy + dy, cx + halfWidth + 1, cy + dy + 1, color);
        }
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return W; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return H; }
}
