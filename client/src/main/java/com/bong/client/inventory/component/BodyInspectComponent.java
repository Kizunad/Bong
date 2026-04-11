package com.bong.client.inventory.component;

import com.bong.client.inventory.model.*;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.EnumMap;
import java.util.Map;

/**
 * 双层人体检视组件 — 体表层(肉体伤势) + 经脉层(气脉状态)。
 * 支持物品放置到身体部位/经脉上。
 */
public class BodyInspectComponent extends BaseComponent {
    private static final int W = 140;
    private static final int H = 175;
    private static final int ICON_SIZE = 128;
    private static final int ITEM_RENDER_SIZE = 14;
    private static final int BODY_COLOR = 0x88222233;
    private static final int DANTIAN_GLOW = 0xCC3388DD;

    public enum Layer { PHYSICAL, MERIDIAN }

    private Layer activeLayer = Layer.PHYSICAL;
    private PhysicalBody physicalBody;
    private MeridianBody meridianBody;
    private long tickCount;

    // Hover state
    private BodyPart hoveredPart;
    private MeridianChannel hoveredChannel;

    // Highlight (drag)
    private BodyPart highlightedPart;
    private MeridianChannel highlightedChannel;
    private boolean highlightValid;

    // 运行时物品（可被玩家拖拽修改，初始化时从模型同步）
    private final EnumMap<BodyPart, InventoryItem> physicalApplied = new EnumMap<>(BodyPart.class);
    private final EnumMap<MeridianChannel, InventoryItem> meridianApplied = new EnumMap<>(MeridianChannel.class);

    public BodyInspectComponent() {
        this.sizing(Sizing.fixed(W), Sizing.fixed(H));
    }

    // ==================== Data setters ====================

    public void setPhysicalBody(PhysicalBody body) {
        this.physicalBody = body;
        physicalApplied.clear();
        if (body != null) physicalApplied.putAll(body.allAppliedItems());
    }

    public void setMeridianBody(MeridianBody body) {
        this.meridianBody = body;
        meridianApplied.clear();
        if (body != null) meridianApplied.putAll(body.allAppliedItems());
    }

    public PhysicalBody physicalBody() { return physicalBody; }
    public MeridianBody meridianBody() { return meridianBody; }

    public Layer activeLayer() { return activeLayer; }
    public void setActiveLayer(Layer layer) { this.activeLayer = layer; }

    public BodyPart hoveredPart() { return hoveredPart; }
    public MeridianChannel hoveredChannel() { return hoveredChannel; }

    // ==================== Applied items API ====================

    // -- Physical layer --
    public void applyPhysicalItem(BodyPart part, InventoryItem item) { physicalApplied.put(part, item); }
    public InventoryItem removePhysicalItem(BodyPart part) { return physicalApplied.remove(part); }
    public InventoryItem physicalItemAt(BodyPart part) { return physicalApplied.get(part); }

    // -- Meridian layer --
    public void applyMeridianItem(MeridianChannel ch, InventoryItem item) { meridianApplied.put(ch, item); }
    public InventoryItem removeMeridianItem(MeridianChannel ch) { return meridianApplied.remove(ch); }
    public InventoryItem meridianItemAt(MeridianChannel ch) { return meridianApplied.get(ch); }

    // ==================== Hit detection (public, for InspectScreen) ====================

    public BodyPart bodyPartAtScreen(double screenX, double screenY) {
        int mx = (int) screenX - x, my = (int) screenY - y;
        if (mx < 0 || mx >= W || my < 0 || my >= H) return null;
        // Items first
        for (BodyPart bp : physicalApplied.keySet()) {
            if (isOverBodyPart(bp, mx, my)) return bp;
        }
        for (BodyPart bp : BodyPart.values()) {
            if (!physicalApplied.containsKey(bp) && isOverBodyPart(bp, mx, my)) return bp;
        }
        return null;
    }

    public MeridianChannel channelAtScreen(double screenX, double screenY) {
        int mx = (int) screenX - x, my = (int) screenY - y;
        if (mx < 0 || mx >= W || my < 0 || my >= H) return null;
        for (MeridianChannel ch : meridianApplied.keySet()) {
            if (isOverMeridian(ch, mx, my)) return ch;
        }
        for (MeridianChannel ch : MeridianChannel.values()) {
            if (!meridianApplied.containsKey(ch) && isOverMeridian(ch, mx, my)) return ch;
        }
        return null;
    }

    // ==================== Highlight (drag) ====================

    public void setPhysicalHighlight(BodyPart part, boolean valid) {
        this.highlightedPart = part; this.highlightValid = valid;
    }

    public void setMeridianHighlight(MeridianChannel ch, boolean valid) {
        this.highlightedChannel = ch; this.highlightValid = valid;
    }

    public void clearHighlight() {
        this.highlightedPart = null;
        this.highlightedChannel = null;
    }

    // ==================== Rendering ====================

    @Override
    public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY, float partialTicks, float delta) {
        tickCount++;
        int bx = x, by = y;
        ctx.fill(bx, by, bx + W, by + H, 0xCC111118);

        int cx = bx + W / 2;
        int hmx = mouseX - bx, hmy = mouseY - by;

        if (activeLayer == Layer.PHYSICAL) {
            drawPhysicalLayer(ctx, cx, by, hmx, hmy);
        } else {
            drawMeridianLayer(ctx, cx, by, hmx, hmy);
        }
    }

    // ==================== Physical Layer ====================

    private void drawPhysicalLayer(OwoUIDrawContext ctx, int cx, int by, int hmx, int hmy) {
        hoveredPart = null;
        hoveredChannel = null;

        if (physicalBody == null) {
            drawBodySilhouette(ctx, cx, by, null);
            var tr = MinecraftClient.getInstance().textRenderer;
            ctx.drawTextWithShadow(tr, Text.literal("§7无体表数据"), x + 30, y + 80, 0xFF666666);
            return;
        }

        // Detect hover
        for (BodyPart bp : physicalApplied.keySet()) {
            if (isOverBodyPart(bp, hmx, hmy)) { hoveredPart = bp; break; }
        }
        if (hoveredPart == null) {
            for (BodyPart bp : BodyPart.values()) {
                if (!physicalApplied.containsKey(bp) && isOverBodyPart(bp, hmx, hmy)) {
                    hoveredPart = bp; break;
                }
            }
        }

        drawBodySilhouette(ctx, cx, by, physicalBody);

        // Drag highlight
        if (highlightedPart != null) {
            int[] r = bodyPartRect(highlightedPart);
            int hlColor = highlightValid ? 0x4444CC66 : 0x44CC4444;
            ctx.fill(cx + r[0], by + r[1], cx + r[2], by + r[3], hlColor);
        }

        // Applied items
        for (var entry : physicalApplied.entrySet()) {
            drawAppliedIcon(ctx, cx, by, bodyPartAnchor(entry.getKey()), entry.getValue());
        }

        // Status summary at bottom
        drawPhysicalStatus(ctx, x, by);
    }

    /** 体表层的人体剪影 — 每个部位按伤势着色 */
    private void drawBodySilhouette(OwoUIDrawContext ctx, int cx, int by, PhysicalBody pb) {
        // 绘制每个身体部位并按伤势着色
        for (BodyPart bp : BodyPart.values()) {
            int color = BODY_COLOR;
            if (pb != null) {
                BodyPartState state = pb.part(bp);
                if (state.wound() != WoundLevel.INTACT) {
                    int wc = state.wound().color();
                    int alpha = 0x88 + (int) ((1.0 - state.wound().functionRatio()) * 0x44);
                    color = (Math.min(0xFF, alpha) << 24) | (wc & 0x00FFFFFF);
                }
                // 断肢特殊处理 — 虚线/半透明
                if (state.wound().isSevered()) {
                    color = 0x33666666;
                }
            }
            drawBodyPartFill(ctx, cx, by, bp, color);
        }
        // Head outline
        fillCircle(ctx, cx, by + 14, 10, 0x44666688);
    }

    private void drawBodyPartFill(OwoUIDrawContext ctx, int cx, int by, BodyPart bp, int color) {
        int[] r = bodyPartRect(bp);
        if (bp == BodyPart.HEAD) {
            fillCircle(ctx, cx, by + 14, 9, color);
        } else {
            ctx.fill(cx + r[0], by + r[1], cx + r[2], by + r[3], color);
        }
        // 出血效果
        if (physicalBody != null) {
            BodyPartState state = physicalBody.part(bp);
            if (state.bleedRate() > 0.01) {
                int pulse = (int) (Math.sin(tickCount * 0.2 + bp.ordinal()) * 30 + 30);
                int bleedAlpha = (int) (state.bleedRate() * 120) + pulse;
                int bleedColor = (Math.min(255, bleedAlpha) << 24) | 0xCC2222;
                if (bp == BodyPart.HEAD) fillCircle(ctx, cx, by + 14, 9, bleedColor);
                else ctx.fill(cx + r[0], by + r[1], cx + r[2], by + r[3], bleedColor);
            }
        }
    }

    private void drawPhysicalStatus(OwoUIDrawContext ctx, int bx, int by) {
        var tr = MinecraftClient.getInstance().textRenderer;
        int ey = by + H - 2;

        // 移速影响
        PhysicalBody.MovementImpairment imp = physicalBody.worstLegImpairment();
        if (imp != PhysicalBody.MovementImpairment.NONE) {
            ey -= tr.fontHeight + 1;
            int impColor = imp == PhysicalBody.MovementImpairment.IMMOBILE ? 0xFFCC2222 : 0xFFCCAA44;
            ctx.drawTextWithShadow(tr, Text.literal("§7移动: " + imp.label()), bx + 4, ey, impColor);
        }

        // 出血警告
        if (physicalBody.isBleeding()) {
            ey -= tr.fontHeight + 1;
            int pulse = (int) (Math.sin(tickCount * 0.15) * 40 + 40);
            ctx.drawTextWithShadow(tr, Text.literal("§c出血中"), bx + 4, ey, 0xFF000000 | (Math.min(255, 180 + pulse) << 16));
        }
    }

    // ==================== Meridian Layer ====================

    private void drawMeridianLayer(OwoUIDrawContext ctx, int cx, int by, int hmx, int hmy) {
        hoveredPart = null;
        hoveredChannel = null;

        // Body silhouette (always draw base)
        drawBodySilhouette(ctx, cx, by, physicalBody);

        if (meridianBody == null) {
            var tr = MinecraftClient.getInstance().textRenderer;
            ctx.drawTextWithShadow(tr, Text.literal("§7无经脉数据"), x + 30, y + 80, 0xFF666666);
            return;
        }

        // Detect hover (applied items first)
        for (MeridianChannel ch : meridianApplied.keySet()) {
            if (isOverMeridian(ch, hmx, hmy)) { hoveredChannel = ch; break; }
        }
        if (hoveredChannel == null) {
            for (MeridianChannel ch : MeridianChannel.values()) {
                if (!meridianApplied.containsKey(ch) && isOverMeridian(ch, hmx, hmy)) {
                    hoveredChannel = ch; break;
                }
            }
        }

        // Draw meridian lines
        for (MeridianChannel ch : MeridianChannel.values()) {
            ChannelState cs = meridianBody.channel(ch);
            if (cs == null) continue;
            drawMeridian(ctx, cx, by, ch, cs, ch == hoveredChannel);
        }

        // Drag highlight
        if (highlightedChannel != null) {
            int[] pos = meridianAnchor(highlightedChannel);
            int hlColor = highlightValid ? 0x4444CC66 : 0x44CC4444;
            fillCircle(ctx, cx + pos[0], by + pos[1], ITEM_RENDER_SIZE / 2 + 3, hlColor);
        }

        // Dantians
        drawDantians(ctx, cx, by);

        // Applied items
        for (var entry : meridianApplied.entrySet()) {
            drawAppliedIcon(ctx, cx, by, meridianAnchor(entry.getKey()), entry.getValue());
        }

        // Status effects
        drawStatusEffects(ctx, x, by);
    }

    // ==================== Shared rendering ====================

    private void drawAppliedIcon(OwoUIDrawContext ctx, int cx, int by, int[] anchor, InventoryItem item) {
        if (item == null || item.isEmpty()) return;
        int ax = cx + anchor[0] - ITEM_RENDER_SIZE / 2;
        int ay = by + anchor[1] - ITEM_RENDER_SIZE / 2;

        ctx.fill(ax - 1, ay - 1, ax + ITEM_RENDER_SIZE + 1, ay + ITEM_RENDER_SIZE + 1, 0x88000000);

        Identifier tex = new Identifier("bong-client", "textures/gui/items/" + item.itemId() + ".png");
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        var m = ctx.getMatrices();
        m.push();
        m.translate(ax, ay, 50);
        m.scale((float) ITEM_RENDER_SIZE / ICON_SIZE, (float) ITEM_RENDER_SIZE / ICON_SIZE, 1f);
        ctx.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
        m.pop();
        RenderSystem.disableBlend();

        // Rarity border
        int bc = (0xAA << 24) | item.rarityColor();
        ctx.fill(ax - 1, ay - 1, ax + ITEM_RENDER_SIZE + 1, ay, bc);
        ctx.fill(ax - 1, ay + ITEM_RENDER_SIZE, ax + ITEM_RENDER_SIZE + 1, ay + ITEM_RENDER_SIZE + 1, bc);
        ctx.fill(ax - 1, ay, ax, ay + ITEM_RENDER_SIZE, bc);
        ctx.fill(ax + ITEM_RENDER_SIZE, ay, ax + ITEM_RENDER_SIZE + 1, ay + ITEM_RENDER_SIZE, bc);
    }

    // ==================== Tooltip (drawn by InspectScreen at high z) ====================

    public void drawTooltip(DrawContext ctx, int mx, int my) {
        if (activeLayer == Layer.PHYSICAL) {
            drawPhysicalTooltip(ctx, mx, my);
        } else {
            drawMeridianTooltip(ctx, mx, my);
        }
    }

    private void drawPhysicalTooltip(DrawContext ctx, int mx, int my) {
        if (hoveredPart == null || physicalBody == null) return;
        BodyPartState state = physicalBody.part(hoveredPart);

        var tr = MinecraftClient.getInstance().textRenderer;
        String line1 = hoveredPart.displayName() + " — " + state.wound().label();
        String line2 = state.bleedRate() > 0.01 ? String.format("出血 %.0f%%", state.bleedRate() * 100) : "";
        String line3 = state.healProgress() > 0.01 ? String.format("恢复 %.0f%%", state.healProgress() * 100) : "";
        String line4 = state.splinted() ? "已上夹板" : "";
        InventoryItem applied = physicalApplied.get(hoveredPart);
        String line5 = applied != null ? "已用: " + applied.displayName() : "";

        drawTooltipBox(ctx, mx, my, tr, state.wound().color(),
            line1, line2, line3, line4, line5);
    }

    private void drawMeridianTooltip(DrawContext ctx, int mx, int my) {
        if (hoveredChannel == null || meridianBody == null) return;
        ChannelState cs = meridianBody.channel(hoveredChannel);
        if (cs == null) return;

        var tr = MinecraftClient.getInstance().textRenderer;
        String line1 = hoveredChannel.displayName() + " — " + cs.damage().label();
        String line2 = String.format("流量 %.0f/%.0f", cs.currentFlow(), cs.capacity());
        String line3 = cs.contamination() > 0.01 ? String.format("污染 %.0f%%", cs.contamination() * 100) : "";
        String line4 = cs.healProgress() > 0.01 ? String.format("恢复 %.0f%%", cs.healProgress() * 100) : "";
        InventoryItem applied = meridianApplied.get(hoveredChannel);
        String line5 = applied != null ? "已用: " + applied.displayName() : "";

        drawTooltipBox(ctx, mx, my, tr, cs.damage().color(),
            line1, line2, line3, line4, line5);
    }

    private void drawTooltipBox(DrawContext ctx, int mx, int my,
                                net.minecraft.client.font.TextRenderer tr, int titleColor,
                                String... lines) {
        int count = 0;
        int maxW = 0;
        for (String l : lines) {
            if (!l.isEmpty()) { count++; maxW = Math.max(maxW, tr.getWidth(l)); }
        }
        if (count == 0) return;
        int tw = maxW + 8, th = count * (tr.fontHeight + 1) + 6;
        int tx = mx + 8, ty = my - th - 4;
        if (ty < 0) ty = my + 12;

        ctx.fill(tx - 2, ty - 2, tx + tw + 2, ty + th + 2, 0xEE111122);
        ctx.fill(tx - 1, ty - 1, tx + tw + 1, ty + th + 1, 0xEE1A1A2A);

        int cy = ty + 2;
        int[] colors = {titleColor, 0xFFAAAAAA, 0xFF9944CC, 0xFF44AA66, 0xFF88CCFF};
        int ci = 0;
        for (String l : lines) {
            if (l.isEmpty()) { ci++; continue; }
            ctx.drawTextWithShadow(tr, Text.literal(l), tx + 2, cy, colors[Math.min(ci, colors.length - 1)]);
            cy += tr.fontHeight + 1;
            ci++;
        }
    }

    // ==================== Meridian rendering (from old MeridianBodyComponent) ====================

    private void drawMeridian(OwoUIDrawContext ctx, int cx, int by,
                              MeridianChannel ch, ChannelState cs, boolean hover) {
        int color = meridianColor(cs, hover);
        int thickness = hover ? 2 : 1;

        int contColor = 0;
        if (cs.contamination() > 0.05) {
            int alpha = (int) (cs.contamination() * 180);
            contColor = (alpha << 24) | 0x9944CC;
        }

        switch (ch) {
            case REN_MAI -> drawLine(ctx, cx - 1, by + 26, cx - 1, by + 85, color, thickness);
            case DU_MAI -> drawLine(ctx, cx + 2, by + 26, cx + 2, by + 85, color, thickness);
            case HEART -> {
                int hx = cx, hy = by + 42;
                drawLine(ctx, hx - 10, hy, hx + 10, hy, color, thickness);
                drawLine(ctx, hx, hy - 5, hx, hy + 5, color, thickness);
                drawLine(ctx, hx - 7, hy - 4, hx + 7, hy + 4, color, thickness);
                drawLine(ctx, hx - 7, hy + 4, hx + 7, hy - 4, color, thickness);
                if (cs.damage() != ChannelState.DamageLevel.INTACT) {
                    int pulse = (int) (Math.sin(tickCount * 0.15) * 40 + 40);
                    int pc = (pulse << 24) | (cs.damage().color() & 0x00FFFFFF);
                    fillCircle(ctx, hx, hy, 6, pc);
                }
            }
            case SPIRIT -> {
                drawLine(ctx, cx, by + 8, cx, by + 20, color, thickness);
                drawLine(ctx, cx - 5, by + 12, cx + 5, by + 12, color, thickness);
            }
            case ARM_YIN -> {
                drawLine(ctx, cx - 14, by + 36, cx - 25, by + 55, color, thickness);
                drawLine(ctx, cx - 25, by + 55, cx - 27, by + 90, color, thickness);
                if (contColor != 0) ctx.fill(cx - 30, by + 45, cx - 22, by + 85, contColor);
            }
            case ARM_YANG -> {
                drawLine(ctx, cx + 14, by + 36, cx + 25, by + 55, color, thickness);
                drawLine(ctx, cx + 25, by + 55, cx + 27, by + 90, color, thickness);
                if (contColor != 0) ctx.fill(cx + 22, by + 45, cx + 30, by + 85, contColor);
            }
            case LEG_YIN -> {
                drawLine(ctx, cx - 8, by + 88, cx - 11, by + 130, color, thickness);
                drawLine(ctx, cx - 11, by + 130, cx - 11, by + 155, color, thickness);
            }
            case LEG_YANG -> {
                drawLine(ctx, cx + 8, by + 88, cx + 11, by + 130, color, thickness);
                drawLine(ctx, cx + 11, by + 130, cx + 11, by + 155, color, thickness);
            }
            case LUNG -> {
                drawLine(ctx, cx - 12, by + 36, cx, by + 42, color, thickness);
                drawLine(ctx, cx, by + 42, cx + 12, by + 36, color, thickness);
            }
            case KIDNEY -> {
                drawLine(ctx, cx - 10, by + 72, cx, by + 80, color, thickness);
                drawLine(ctx, cx, by + 80, cx + 10, by + 72, color, thickness);
            }
            case LIVER -> {
                drawLine(ctx, cx + 4, by + 58, cx + 14, by + 65, color, thickness);
                drawLine(ctx, cx + 14, by + 65, cx + 8, by + 72, color, thickness);
            }
            case SPLEEN -> {
                drawLine(ctx, cx - 4, by + 60, cx - 14, by + 67, color, thickness);
                drawLine(ctx, cx - 14, by + 67, cx - 8, by + 74, color, thickness);
            }
        }
    }

    private int meridianColor(ChannelState cs, boolean hover) {
        if (cs.blocked()) return 0xFF333333;
        int base = cs.damage().color();
        if (hover) {
            int r = Math.min(255, ((base >> 16) & 0xFF) + 60);
            int g = Math.min(255, ((base >> 8) & 0xFF) + 60);
            int b = Math.min(255, (base & 0xFF) + 60);
            return 0xFF000000 | (r << 16) | (g << 8) | b;
        }
        int alpha = (int) (100 + cs.flowRatio() * 155);
        return (alpha << 24) | (base & 0x00FFFFFF);
    }

    private void drawDantians(OwoUIDrawContext ctx, int cx, int by) {
        if (meridianBody == null) return;
        drawDantianNode(ctx, cx, by + 14, meridianBody.dantian(MeridianBody.DantianTier.UPPER));
        drawDantianNode(ctx, cx, by + 45, meridianBody.dantian(MeridianBody.DantianTier.MIDDLE));
        drawDantianNode(ctx, cx, by + 78, meridianBody.dantian(MeridianBody.DantianTier.LOWER));
    }

    private void drawDantianNode(OwoUIDrawContext ctx, int nx, int ny, MeridianBody.DantianState ds) {
        if (ds == null) return;
        double ratio = ds.ratio();
        int pulse = (int) (Math.sin(tickCount * 0.08 + ny * 0.1) * 20 + 20);
        int ga = (int) (40 + ratio * 80 + pulse);
        fillCircle(ctx, nx, ny, 5, (ga << 24) | (DANTIAN_GLOW & 0x00FFFFFF));
        int ca = (int) (120 + ratio * 135);
        fillCircle(ctx, nx, ny, 3, (ca << 24) | (ratio > 0.5 ? 0x4488CC : 0x334466));
        if (ds.sealed()) ctx.fill(nx - 4, ny - 1, nx + 4, ny + 1, 0xCCCC2222);
    }

    private void drawStatusEffects(OwoUIDrawContext ctx, int bx, int by) {
        if (meridianBody == null) return;
        var effects = meridianBody.activeEffects();
        if (effects.isEmpty()) return;
        var tr = MinecraftClient.getInstance().textRenderer;
        int ey = by + H - 2 - effects.size() * (tr.fontHeight + 1);
        for (var effect : effects) {
            ctx.fill(bx + 2, ey - 1, bx + 4, ey + tr.fontHeight - 1, effect.color());
            ctx.drawTextWithShadow(tr, Text.literal("§7" + effect.name()), bx + 6, ey, 0xFF999999);
            ey += tr.fontHeight + 1;
        }
    }

    // ==================== Geometry tables ====================

    /** Body part bounding rect relative to (center_x, top_y) → [x1, y1, x2, y2] */
    private static int[] bodyPartRect(BodyPart bp) {
        return switch (bp) {
            case HEAD             -> new int[]{-9, 5, 9, 23};
            case NECK             -> new int[]{-3, 23, 3, 28};
            case CHEST            -> new int[]{-18, 28, 18, 55};
            case ABDOMEN          -> new int[]{-18, 55, 18, 82};
            case LEFT_UPPER_ARM   -> new int[]{-28, 30, -22, 60};
            case LEFT_FOREARM     -> new int[]{-30, 60, -24, 90};
            case LEFT_HAND        -> new int[]{-32, 88, -23, 96};
            case RIGHT_UPPER_ARM  -> new int[]{22, 30, 28, 60};
            case RIGHT_FOREARM    -> new int[]{24, 60, 30, 90};
            case RIGHT_HAND       -> new int[]{23, 88, 32, 96};
            case LEFT_THIGH       -> new int[]{-16, 92, -6, 130};
            case LEFT_CALF        -> new int[]{-15, 130, -7, 155};
            case LEFT_FOOT        -> new int[]{-18, 155, -5, 160};
            case RIGHT_THIGH      -> new int[]{6, 92, 16, 130};
            case RIGHT_CALF       -> new int[]{7, 130, 15, 155};
            case RIGHT_FOOT       -> new int[]{5, 155, 18, 160};
        };
    }

    /** Item icon anchor for body parts → [dx, dy] relative to center */
    private static int[] bodyPartAnchor(BodyPart bp) {
        return switch (bp) {
            case HEAD             -> new int[]{0, 8};
            case NECK             -> new int[]{0, 25};
            case CHEST            -> new int[]{0, 40};
            case ABDOMEN          -> new int[]{0, 68};
            case LEFT_UPPER_ARM   -> new int[]{-30, 44};
            case LEFT_FOREARM     -> new int[]{-33, 74};
            case LEFT_HAND        -> new int[]{-34, 92};
            case RIGHT_UPPER_ARM  -> new int[]{30, 44};
            case RIGHT_FOREARM    -> new int[]{33, 74};
            case RIGHT_HAND       -> new int[]{34, 92};
            case LEFT_THIGH       -> new int[]{-18, 110};
            case LEFT_CALF        -> new int[]{-18, 142};
            case LEFT_FOOT        -> new int[]{-16, 158};
            case RIGHT_THIGH      -> new int[]{18, 110};
            case RIGHT_CALF       -> new int[]{18, 142};
            case RIGHT_FOOT       -> new int[]{16, 158};
        };
    }

    private static int[] meridianAnchor(MeridianChannel ch) {
        return switch (ch) {
            case REN_MAI   -> new int[]{-8, 55};
            case DU_MAI    -> new int[]{8, 55};
            case HEART     -> new int[]{0, 42};
            case SPIRIT    -> new int[]{0, 6};
            case ARM_YIN   -> new int[]{-36, 65};
            case ARM_YANG  -> new int[]{36, 65};
            case LEG_YIN   -> new int[]{-20, 140};
            case LEG_YANG  -> new int[]{20, 140};
            case LUNG      -> new int[]{0, 33};
            case KIDNEY    -> new int[]{0, 80};
            case LIVER     -> new int[]{14, 64};
            case SPLEEN    -> new int[]{-14, 66};
        };
    }

    // ==================== Hit detection ====================

    private boolean isOverBodyPart(BodyPart bp, int mx, int my) {
        if (bp == BodyPart.HEAD) return dist(mx, my, W / 2, 14) < 11;
        int[] r = bodyPartRect(bp);
        int cx = W / 2;
        return mx >= cx + r[0] && mx < cx + r[2] && my >= r[1] && my < r[3];
    }

    private boolean isOverMeridian(MeridianChannel ch, int mx, int my) {
        int cx = W / 2;
        int hr = 8;
        return switch (ch) {
            case REN_MAI -> Math.abs(mx - cx + 1) < hr && my > 24 && my < 87;
            case DU_MAI -> Math.abs(mx - cx - 2) < hr && my > 24 && my < 87;
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
        int dx = Math.abs(x2 - x1), dy = Math.abs(y2 - y1);
        int steps = Math.max(dx, dy);
        if (steps == 0) { ctx.fill(x1, y1, x1 + thickness, y1 + thickness, color); return; }
        float xi = (float)(x2 - x1) / steps, yi = (float)(y2 - y1) / steps;
        float px = x1, py = y1;
        for (int i = 0; i <= steps; i++) {
            ctx.fill((int)px, (int)py, (int)px + thickness, (int)py + thickness, color);
            px += xi; py += yi;
        }
    }

    private static void fillCircle(OwoUIDrawContext ctx, int cx, int cy, int radius, int color) {
        for (int dy = -radius; dy <= radius; dy++) {
            int hw = (int) Math.sqrt(radius * radius - dy * dy);
            ctx.fill(cx - hw, cy + dy, cx + hw + 1, cy + dy + 1, color);
        }
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return W; }
    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return H; }
}
