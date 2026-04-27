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
 *
 * <p>经脉层支持分组筛选（全部 / 手经 / 足经 / 奇经），
 * 使用多层 glow 描边绘制，点击选中脉以锁定详情面板。
 */
public class BodyInspectComponent extends BaseComponent {
    private static final int W = 168;           // 画布宽
    private static final int BODY_BOTTOM_Y = 192; // 足底 y（body 绘制上限）
    private static final int DETAIL_TOP = 196;    // 底部详情带顶部
    private static final int DETAIL_H = 40;       // 底部详情带高度
    private static final int H = DETAIL_TOP + DETAIL_H; // 236
    private static final int ICON_SIZE = 128;
    private static final int ITEM_RENDER_SIZE = 14;
    private static final int BODY_COLOR = 0x88222233;

    public enum Layer { PHYSICAL, MERIDIAN }

    /** 经脉筛选 — 缩减同屏显示数量，让间距加大 */
    public enum MeridianFilter {
        ALL("全部"), ARM("手经"), LEG("足经"), EXTRA("奇经");
        private final String label;
        MeridianFilter(String label) { this.label = label; }
        public String label() { return label; }

        /** 是否包含指定经脉 */
        public boolean includes(MeridianChannel ch) {
            return switch (this) {
                case ALL -> true;
                case ARM -> ch == MeridianChannel.LU || ch == MeridianChannel.HT || ch == MeridianChannel.PC
                         || ch == MeridianChannel.LI || ch == MeridianChannel.SI || ch == MeridianChannel.TE;
                case LEG -> ch == MeridianChannel.SP || ch == MeridianChannel.KI || ch == MeridianChannel.LR
                         || ch == MeridianChannel.ST || ch == MeridianChannel.BL || ch == MeridianChannel.GB;
                case EXTRA -> ch.family() == MeridianChannel.Family.EXTRAORDINARY;
            };
        }
    }

    private Layer activeLayer = Layer.PHYSICAL;
    private MeridianFilter meridianFilter = MeridianFilter.ALL;
    private PhysicalBody physicalBody;
    private MeridianBody meridianBody;
    private long tickCount;

    // Hover state
    private BodyPart hoveredPart;
    private MeridianChannel hoveredChannel;

    // Persistent selection (click to pin)
    private MeridianChannel selectedChannel;
    private final java.util.List<java.util.function.Consumer<MeridianChannel>> selectionListeners =
        new java.util.concurrent.CopyOnWriteArrayList<>();

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

    public MeridianFilter meridianFilter() { return meridianFilter; }
    public void setMeridianFilter(MeridianFilter filter) {
        this.meridianFilter = filter;
        // 若选中脉被过滤掉，则解除选中
        if (selectedChannel != null && !filter.includes(selectedChannel)) {
            selectedChannel = null;
            fireSelectionChanged();
        }
    }

    public MeridianChannel selectedChannel() { return selectedChannel; }
    public void setSelectedChannel(MeridianChannel ch) {
        if (this.selectedChannel == ch) return;
        this.selectedChannel = ch;
        fireSelectionChanged();
    }

    public void addSelectionListener(java.util.function.Consumer<MeridianChannel> l) {
        selectionListeners.add(l);
    }

    private void fireSelectionChanged() {
        for (var l : selectionListeners) l.accept(selectedChannel);
    }
    /** 当前用于显示详情的脉：优先 hover，其次 selected。 */
    public MeridianChannel focusedChannel() {
        return hoveredChannel != null ? hoveredChannel : selectedChannel;
    }

    public BodyPart hoveredPart() { return hoveredPart; }
    public MeridianChannel hoveredChannel() { return hoveredChannel; }

    // ==================== Applied items API ====================

    public void applyPhysicalItem(BodyPart part, InventoryItem item) { physicalApplied.put(part, item); }
    public InventoryItem removePhysicalItem(BodyPart part) { return physicalApplied.remove(part); }
    public InventoryItem physicalItemAt(BodyPart part) { return physicalApplied.get(part); }

    public void applyMeridianItem(MeridianChannel ch, InventoryItem item) { meridianApplied.put(ch, item); }
    public InventoryItem removeMeridianItem(MeridianChannel ch) { return meridianApplied.remove(ch); }
    public InventoryItem meridianItemAt(MeridianChannel ch) { return meridianApplied.get(ch); }

    // ==================== Hit detection (public, for InspectScreen) ====================

    public BodyPart bodyPartAtScreen(double screenX, double screenY) {
        int mx = (int) screenX - x, my = (int) screenY - y;
        if (mx < 0 || mx >= W || my < 0 || my >= H) return null;
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
        // 按线段距离命中 — 取最小距离的脉（且必须在 filter 范围内）
        MeridianChannel best = null;
        double bestDist = 6.0; // 命中阈值：6 像素以内
        int cx = W / 2;
        for (MeridianChannel ch : MeridianChannel.values()) {
            if (!meridianFilter.includes(ch)) continue;
            double d = distanceToPath(mx, my, cx, ch);
            if (d < bestDist) { bestDist = d; best = ch; }
        }
        return best;
    }

    // ==================== Selection ====================

    /** 在经脉层点击：若点中某脉，切换选中；再次点中同一脉则解除。 */
    public boolean clickSelectMeridian(double screenX, double screenY) {
        if (activeLayer != Layer.MERIDIAN) return false;
        MeridianChannel ch = channelAtScreen(screenX, screenY);
        if (ch == null) return false;
        selectedChannel = (ch == selectedChannel) ? null : ch;
        fireSelectionChanged();
        return true;
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
        // 背景带细边框
        ctx.fill(bx, by, bx + W, by + H, 0xDD0E0E14);
        ctx.fill(bx, by, bx + W, by + 1, 0x44FFFFFF);
        ctx.fill(bx, by + H - 1, bx + W, by + H, 0x44000000);

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
            ctx.drawTextWithShadow(tr, Text.literal("§7无体表数据"), x + 30, y + 90, 0xFF666666);
            return;
        }

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

        if (highlightedPart != null) {
            int[] r = bodyPartRect(highlightedPart);
            int hlColor = highlightValid ? 0x4444CC66 : 0x44CC4444;
            ctx.fill(cx + r[0], by + r[1], cx + r[2], by + r[3], hlColor);
        }

        for (var entry : physicalApplied.entrySet()) {
            drawAppliedIcon(ctx, cx, by, bodyPartAnchor(entry.getKey()), entry.getValue());
        }

        drawPhysicalStatus(ctx, x, by);
    }

    /** 体表层的人体剪影 — 每个部位按伤势着色 */
    private void drawBodySilhouette(OwoUIDrawContext ctx, int cx, int by, PhysicalBody pb) {
        for (BodyPart bp : BodyPart.values()) {
            int color = BODY_COLOR;
            if (pb != null) {
                BodyPartState state = pb.part(bp);
                if (state.wound() != WoundLevel.INTACT) {
                    int wc = state.wound().color();
                    int alpha = 0x88 + (int) ((1.0 - state.wound().functionRatio()) * 0x44);
                    color = (Math.min(0xFF, alpha) << 24) | (wc & 0x00FFFFFF);
                }
                if (state.wound().isSevered()) color = 0x33666666;
            }
            drawBodyPartFill(ctx, cx, by, bp, color);
        }
        // Head outline glow
        fillCircle(ctx, cx, by + 17, 12, 0x44666688);
    }

    private void drawBodyPartFill(OwoUIDrawContext ctx, int cx, int by, BodyPart bp, int color) {
        int[] r = bodyPartRect(bp);
        if (bp == BodyPart.HEAD) {
            fillCircle(ctx, cx, by + 17, 11, color);
        } else {
            ctx.fill(cx + r[0], by + r[1], cx + r[2], by + r[3], color);
        }
        if (physicalBody != null) {
            BodyPartState state = physicalBody.part(bp);
            if (state.bleedRate() > 0.01) {
                int pulse = (int) (Math.sin(tickCount * 0.2 + bp.ordinal()) * 30 + 30);
                int bleedAlpha = (int) (state.bleedRate() * 120) + pulse;
                int bleedColor = (Math.min(255, bleedAlpha) << 24) | 0xCC2222;
                if (bp == BodyPart.HEAD) fillCircle(ctx, cx, by + 17, 11, bleedColor);
                else ctx.fill(cx + r[0], by + r[1], cx + r[2], by + r[3], bleedColor);
            }
        }
    }

    private void drawPhysicalStatus(OwoUIDrawContext ctx, int bx, int by) {
        var tr = MinecraftClient.getInstance().textRenderer;
        drawDetailBandBackground(ctx, bx, by);

        int ey = by + DETAIL_TOP + 4;
        ctx.drawTextWithShadow(tr, Text.literal("§8肉体状态"), bx + 4, ey, 0xFF666666);
        ey += tr.fontHeight + 2;

        boolean anyStatus = false;
        PhysicalBody.MovementImpairment imp = physicalBody.worstLegImpairment();
        if (imp != PhysicalBody.MovementImpairment.NONE) {
            int impColor = imp == PhysicalBody.MovementImpairment.IMMOBILE ? 0xFFCC2222 : 0xFFCCAA44;
            ctx.drawTextWithShadow(tr, Text.literal("§7移动: " + imp.label()), bx + 4, ey, impColor);
            ey += tr.fontHeight + 1;
            anyStatus = true;
        }

        if (physicalBody.isBleeding()) {
            int pulse = (int) (Math.sin(tickCount * 0.15) * 40 + 40);
            ctx.drawTextWithShadow(tr, Text.literal("§c出血中"),
                bx + 4, ey, 0xFF000000 | (Math.min(255, 180 + pulse) << 16));
            ey += tr.fontHeight + 1;
            anyStatus = true;
        }

        if (!anyStatus) {
            ctx.drawTextWithShadow(tr, Text.literal("§8正常"), bx + 4, ey, 0xFF555555);
        }
    }

    /** 画底部详情带背景 + 顶部分隔线，给 physical/meridian 两层共用 */
    private void drawDetailBandBackground(OwoUIDrawContext ctx, int bx, int by) {
        int bandY = by + DETAIL_TOP;
        ctx.fill(bx, bandY - 1, bx + W, bandY, 0x33FFFFFF);           // 细分隔线
        ctx.fill(bx, bandY, bx + W, by + H, 0x88101018);               // 半透底
    }

    // ==================== Meridian Layer ====================

    private void drawMeridianLayer(OwoUIDrawContext ctx, int cx, int by, int hmx, int hmy) {
        hoveredPart = null;

        // Body silhouette (faded base)
        drawBodySilhouette(ctx, cx, by, physicalBody);

        if (meridianBody == null) {
            var tr = MinecraftClient.getInstance().textRenderer;
            ctx.drawTextWithShadow(tr, Text.literal("§7无经脉数据"), x + 30, y + 90, 0xFF666666);
            return;
        }

        // Hover detection — path-distance based
        hoveredChannel = null;
        double bestDist = 6.0;
        for (MeridianChannel ch : MeridianChannel.values()) {
            if (!meridianFilter.includes(ch)) continue;
            double d = distanceToPath(hmx, hmy, W / 2, ch);
            if (d < bestDist) { bestDist = d; hoveredChannel = ch; }
        }

        // Phase 1: 被过滤掉的脉 — 画极淡底色参考
        if (meridianFilter != MeridianFilter.ALL) {
            for (MeridianChannel ch : MeridianChannel.values()) {
                if (meridianFilter.includes(ch)) continue;
                ChannelState cs = meridianBody.channel(ch);
                if (cs == null) continue;
                drawMeridianGhost(ctx, cx, by, ch);
            }
        }

        // Phase 2: 显示范围内的脉 — glow 粗笔
        for (MeridianChannel ch : MeridianChannel.values()) {
            if (!meridianFilter.includes(ch)) continue;
            ChannelState cs = meridianBody.channel(ch);
            if (cs == null) continue;
            boolean hover = (ch == hoveredChannel);
            boolean selected = (ch == selectedChannel);
            drawMeridianGlow(ctx, cx, by, ch, cs, hover, selected);
        }

        // Highlight drag target
        if (highlightedChannel != null) {
            int[] a = meridianAnchor(highlightedChannel);
            int hlColor = highlightValid ? 0x4444CC66 : 0x44CC4444;
            fillCircle(ctx, cx + a[0], by + a[1], ITEM_RENDER_SIZE / 2 + 3, hlColor);
        }

        for (var entry : meridianApplied.entrySet()) {
            if (!meridianFilter.includes(entry.getKey())) continue;
            drawAppliedIcon(ctx, cx, by, meridianAnchor(entry.getKey()), entry.getValue());
        }

        // 底部详情带（焦点经脉信息）
        drawMeridianDetailInline(ctx, x, by);

        // 状态效果 badge 贴右上角
        drawStatusEffects(ctx, x, by);
    }

    /** 在画布底部区域直接绘制当前聚焦脉的详情（不另开面板）。 */
    private void drawMeridianDetailInline(OwoUIDrawContext ctx, int bx, int by) {
        drawDetailBandBackground(ctx, bx, by);

        var tr = MinecraftClient.getInstance().textRenderer;
        MeridianChannel focus = focusedChannel();

        int py = by + DETAIL_TOP + 3;
        if (focus == null || meridianBody == null) {
            ctx.drawTextWithShadow(tr, Text.literal("§8悬浮或点击经脉查看详情"),
                bx + 4, py + 12, 0xFF555555);
            return;
        }

        ChannelState cs = meridianBody.channel(focus);
        if (cs == null) {
            ctx.drawTextWithShadow(tr, Text.literal(focus.displayName()),
                bx + 4, py, focus.baseColor());
            ctx.drawTextWithShadow(tr, Text.literal("§7（未录入）"),
                bx + 4, py + 12, 0xFF666666);
            return;
        }

        // === Row 1: 经脉名 + 族 + 损伤（右对齐） ===
        String name = focus.displayName();
        ctx.drawTextWithShadow(tr, Text.literal(name), bx + 4, py, focus.baseColor());

        int afterName = bx + 4 + tr.getWidth(name) + 4;
        String fam = focus.family() == MeridianChannel.Family.REGULAR ? "§8正经" : "§d奇经";
        ctx.drawTextWithShadow(tr, Text.literal(fam), afterName, py, 0xFFAAAAAA);

        String dmg = cs.damage().label();
        int cracks = meridianBody.cracksFor(focus);
        String rightText = cracks > 0 ? (dmg + " · 裂 " + cracks) : dmg;
        int rightColor = cracks > 0 ? 0xFFFF5050 : cs.damage().color();
        ctx.drawTextWithShadow(tr, Text.literal(rightText),
            bx + W - 4 - tr.getWidth(rightText), py, rightColor);

        // === Row 2: 流量条 ===
        int row2Y = py + 12;
        String flowStr = String.format("%.0f/%.0f", cs.currentFlow(), cs.capacity());
        ctx.drawTextWithShadow(tr, Text.literal("§7流量"), bx + 4, row2Y, 0xFFAAAAAA);
        int numStart = bx + 4 + tr.getWidth("流量") + 4;
        ctx.drawTextWithShadow(tr, Text.literal("§f" + flowStr), numStart, row2Y, 0xFFDDDDDD);
        // bar 在右半边
        int barX = numStart + tr.getWidth(flowStr) + 4;
        int barW = bx + W - 4 - barX;
        if (barW > 20) {
            drawBar(ctx, barX, row2Y + 2, barW, 4, cs.flowRatio(), 0xFF44AACC, 0xFF223344);
        }

        // === Row 3: 污染 + 恢复（左右并列） ===
        int row3Y = py + 24;
        int half = (W - 8) / 2;

        // 左：污染
        if (cs.contamination() > 0.01) {
            String t = String.format("污 %.0f%%", cs.contamination() * 100);
            ctx.drawTextWithShadow(tr, Text.literal("§d" + t), bx + 4, row3Y, 0xFFCC88DD);
            int tw = tr.getWidth(t) + 4;
            if (half - tw > 10) {
                drawBar(ctx, bx + 4 + tw, row3Y + 2, half - tw - 4, 3,
                    cs.contamination(), 0xFF9944CC, 0xFF2A1A3A);
            }
        } else {
            ctx.drawTextWithShadow(tr, Text.literal("§8污染 无"), bx + 4, row3Y, 0xFF666666);
        }

        // 右：恢复 / 封闭
        int rightX = bx + 4 + half;
        if (cs.healProgress() > 0.01) {
            String t = String.format("恢 %.0f%%", cs.healProgress() * 100);
            ctx.drawTextWithShadow(tr, Text.literal("§a" + t), rightX, row3Y, 0xFF88CC88);
            int tw = tr.getWidth(t) + 4;
            if (half - tw > 10) {
                drawBar(ctx, rightX + tw, row3Y + 2, half - tw - 4, 3,
                    cs.healProgress(), 0xFF44AA66, 0xFF1A2A1F);
            }
        } else if (cs.blocked()) {
            ctx.drawTextWithShadow(tr, Text.literal("§c已封闭"), rightX, row3Y, 0xFFCC4444);
        } else {
            ctx.drawTextWithShadow(tr, Text.literal("§8恢复 —"), rightX, row3Y, 0xFF666666);
        }
    }

    /** 水平进度条（内联详情用） */
    private static void drawBar(OwoUIDrawContext ctx, int bx, int by, int bw, int bh, double ratio,
                                int fgColor, int bgColor) {
        if (bw <= 0) return;
        ratio = Math.max(0, Math.min(1, ratio));
        ctx.fill(bx, by, bx + bw, by + bh, bgColor);
        int fillW = (int) (bw * ratio);
        if (fillW > 0) ctx.fill(bx, by, bx + fillW, by + bh, fgColor);
    }

    // ==================== Meridian glow rendering ====================

    /** 未选中/未显示脉的参考线（极淡） */
    private void drawMeridianGhost(OwoUIDrawContext ctx, int cx, int by, MeridianChannel ch) {
        int[][] wp = MERIDIAN_PATHS.get(ch);
        if (wp == null) return;
        int color = 0x22FFFFFF;
        for (int i = 0; i < wp.length - 1; i++) {
            drawThickLine(ctx, cx + wp[i][0], by + wp[i][1], cx + wp[i + 1][0], by + wp[i + 1][1], color, 1);
        }
    }

    /** 显示脉 — 干净的单笔画：深色边 + 主色线 + 可选选中环。不发光。 */
    private void drawMeridianGlow(OwoUIDrawContext ctx, int cx, int by,
                                  MeridianChannel ch, ChannelState cs,
                                  boolean hover, boolean selected) {
        int[][] wp = MERIDIAN_PATHS.get(ch);
        if (wp == null) return;

        int baseRgb = cs.damage().color() & 0x00FFFFFF;
        boolean active = hover || selected;

        // Alpha / 粗度：默认纤细，激活时加粗提亮
        int mainAlpha;
        int thickness;
        if (active) {
            mainAlpha = 240;
            thickness = 3;
        } else {
            // 全部视图下更低调，专项筛选时稍亮
            double a = (meridianFilter == MeridianFilter.ALL ? 0.45 : 0.75)
                + cs.flowRatio() * 0.15;
            mainAlpha = (int) (a * 255);
            thickness = 2;
        }

        // 深色描边 (+1 px)：增加对比，不让线融进背景
        int edgeAlpha = Math.min(255, (int) (mainAlpha * 0.75));
        int edgeColor = (edgeAlpha << 24) | darken(baseRgb, 60);
        for (int i = 0; i < wp.length - 1; i++) {
            drawThickLine(ctx, cx + wp[i][0], by + wp[i][1],
                cx + wp[i + 1][0], by + wp[i + 1][1], edgeColor, thickness + 1);
        }

        // 主色线
        int mainColor = (mainAlpha << 24) | baseRgb;
        for (int i = 0; i < wp.length - 1; i++) {
            drawThickLine(ctx, cx + wp[i][0], by + wp[i][1],
                cx + wp[i + 1][0], by + wp[i + 1][1], mainColor, thickness);
        }

        // 激活时亮芯线（中央 1 px 更亮）
        if (active) {
            int coreColor = 0xFFFFFFFF;
            for (int i = 0; i < wp.length - 1; i++) {
                drawThickLine(ctx, cx + wp[i][0], by + wp[i][1],
                    cx + wp[i + 1][0], by + wp[i + 1][1], coreColor, 1);
            }
        }

        // 污染：沿路径散布紫点（仅达到阈值才显示）
        if (cs.contamination() > 0.05) {
            int dotAlpha = (int) (cs.contamination() * 200);
            int dotColor = (dotAlpha << 24) | 0x9944CC;
            for (int i = 0; i < wp.length - 1; i++) {
                int mxp = (wp[i][0] + wp[i + 1][0]) / 2;
                int myp = (wp[i][1] + wp[i + 1][1]) / 2;
                fillCircle(ctx, cx + mxp, by + myp, 1, dotColor);
            }
        }

        // 裂痕：沿路径画短红色横杠（数量 = cracksCount，均匀分布）
        int cracks = meridianBody == null ? 0 : meridianBody.cracksFor(ch);
        if (cracks > 0) {
            int maxMarkers = Math.min(cracks, Math.max(1, wp.length - 1));
            int crackColor = active ? 0xFFFF5050 : 0xCCCC4040;
            for (int k = 0; k < maxMarkers; k++) {
                // 插值到路径段：k/(maxMarkers+1) * (wp.length-1)
                double t = (double) (k + 1) / (maxMarkers + 1) * (wp.length - 1);
                int si = (int) Math.floor(t);
                double frac = t - si;
                if (si >= wp.length - 1) { si = wp.length - 2; frac = 1.0; }
                int ax = (int) (wp[si][0] + (wp[si + 1][0] - wp[si][0]) * frac);
                int ay = (int) (wp[si][1] + (wp[si + 1][1] - wp[si][1]) * frac);
                // 跟路径段接近垂直的短杠
                int dx = wp[si + 1][0] - wp[si][0];
                int dy = wp[si + 1][1] - wp[si][1];
                double len = Math.max(1.0, Math.hypot(dx, dy));
                int px = (int) Math.round(-dy / len * 2.0);
                int py = (int) Math.round(dx / len * 2.0);
                drawThickLine(ctx, cx + ax - px, by + ay - py,
                    cx + ax + px, by + ay + py, crackColor, 1);
            }
        }

        // 选中：终点位置画一个静态白环（无脉动、不闪烁）
        if (selected) {
            int[] end = wp[wp.length - 1];
            drawCircleOutline(ctx, cx + end[0], by + end[1], 4, 0xDDFFFFFF);
        }
    }

    /** 将基色调暗指定量（clamp 到 0） */
    private static int darken(int rgb, int amount) {
        int r = Math.max(0, ((rgb >> 16) & 0xFF) - amount);
        int g = Math.max(0, ((rgb >> 8) & 0xFF) - amount);
        int b = Math.max(0, (rgb & 0xFF) - amount);
        return (r << 16) | (g << 8) | b;
    }

    private static void drawCircleOutline(OwoUIDrawContext ctx, int cx, int cy, int r, int color) {
        for (int a = 0; a < 360; a += 15) {
            double rad = Math.toRadians(a);
            int px = cx + (int) (Math.cos(rad) * r);
            int py = cy + (int) (Math.sin(rad) * r);
            ctx.fill(px, py, px + 1, py + 1, color);
        }
    }

    // ==================== Shared rendering ====================

    private void drawAppliedIcon(OwoUIDrawContext ctx, int cx, int by, int[] anchor, InventoryItem item) {
        if (item == null || item.isEmpty()) return;
        int ax = cx + anchor[0] - ITEM_RENDER_SIZE / 2;
        int ay = by + anchor[1] - ITEM_RENDER_SIZE / 2;

        ctx.fill(ax - 1, ay - 1, ax + ITEM_RENDER_SIZE + 1, ay + ITEM_RENDER_SIZE + 1, 0x88000000);

        Identifier tex = GridSlotComponent.textureIdForItem(item);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        var m = ctx.getMatrices();
        m.push();
        m.translate(ax, ay, 50);
        m.scale((float) ITEM_RENDER_SIZE / ICON_SIZE, (float) ITEM_RENDER_SIZE / ICON_SIZE, 1f);
        ctx.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
        m.pop();
        RenderSystem.disableBlend();

        int bc = (0xAA << 24) | item.rarityColor();
        ctx.fill(ax - 1, ay - 1, ax + ITEM_RENDER_SIZE + 1, ay, bc);
        ctx.fill(ax - 1, ay + ITEM_RENDER_SIZE, ax + ITEM_RENDER_SIZE + 1, ay + ITEM_RENDER_SIZE + 1, bc);
        ctx.fill(ax - 1, ay, ax, ay + ITEM_RENDER_SIZE, bc);
        ctx.fill(ax + ITEM_RENDER_SIZE, ay, ax + ITEM_RENDER_SIZE + 1, ay + ITEM_RENDER_SIZE, bc);
    }

    // ==================== Tooltip (only for physical layer now) ====================

    public void drawTooltip(DrawContext ctx, int mx, int my) {
        if (activeLayer == Layer.PHYSICAL) drawPhysicalTooltip(ctx, mx, my);
        // 经脉层的详情由外部面板展示，不再走 tooltip
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

    // ==================== Status effects ====================

    private void drawStatusEffects(OwoUIDrawContext ctx, int bx, int by) {
        // 经脉层状态效果作为 badge 绘制在右上角（避免和底部详情带冲突）
        if (meridianBody == null) return;
        var effects = meridianBody.activeEffects();
        if (effects.isEmpty()) return;
        var tr = MinecraftClient.getInstance().textRenderer;
        int ey = by + 2;
        for (var effect : effects) {
            String txt = effect.name();
            int tw = tr.getWidth(txt) + 6;
            int ex = bx + W - tw - 2;
            ctx.fill(ex, ey, ex + tw, ey + tr.fontHeight + 1, 0xCC1A1A28);
            ctx.fill(ex, ey, ex + 2, ey + tr.fontHeight + 1, effect.color());
            ctx.drawTextWithShadow(tr, Text.literal("§7" + txt), ex + 4, ey + 1, 0xFFDDDDDD);
            ey += tr.fontHeight + 2;
        }
    }

    // ==================== Geometry tables ====================

    /** Body part bounding rect relative to (center_x, top_y) → [x1, y1, x2, y2]
     *  尺度相对原设计放大 1.2x (H=175→210)。 */
    private static int[] bodyPartRect(BodyPart bp) {
        return switch (bp) {
            case HEAD             -> new int[]{-11, 6, 11, 28};
            case NECK             -> new int[]{-4, 28, 4, 34};
            case CHEST            -> new int[]{-22, 34, 22, 66};
            case ABDOMEN          -> new int[]{-22, 66, 22, 98};
            case LEFT_UPPER_ARM   -> new int[]{-34, 36, -26, 72};
            case LEFT_FOREARM     -> new int[]{-36, 72, -28, 108};
            case LEFT_HAND        -> new int[]{-38, 105, -28, 115};
            case RIGHT_UPPER_ARM  -> new int[]{26, 36, 34, 72};
            case RIGHT_FOREARM    -> new int[]{28, 72, 36, 108};
            case RIGHT_HAND       -> new int[]{28, 105, 38, 115};
            case LEFT_THIGH       -> new int[]{-19, 110, -7, 156};
            case LEFT_CALF        -> new int[]{-18, 156, -8, 186};
            case LEFT_FOOT        -> new int[]{-22, 186, -6, 192};
            case RIGHT_THIGH      -> new int[]{7, 110, 19, 156};
            case RIGHT_CALF       -> new int[]{8, 156, 18, 186};
            case RIGHT_FOOT       -> new int[]{6, 186, 22, 192};
        };
    }

    private static int[] bodyPartAnchor(BodyPart bp) {
        return switch (bp) {
            case HEAD             -> new int[]{0, 10};
            case NECK             -> new int[]{0, 30};
            case CHEST            -> new int[]{0, 48};
            case ABDOMEN          -> new int[]{0, 82};
            case LEFT_UPPER_ARM   -> new int[]{-36, 53};
            case LEFT_FOREARM     -> new int[]{-40, 89};
            case LEFT_HAND        -> new int[]{-41, 110};
            case RIGHT_UPPER_ARM  -> new int[]{36, 53};
            case RIGHT_FOREARM    -> new int[]{40, 89};
            case RIGHT_HAND       -> new int[]{41, 110};
            case LEFT_THIGH       -> new int[]{-22, 132};
            case LEFT_CALF        -> new int[]{-22, 170};
            case LEFT_FOOT        -> new int[]{-19, 190};
            case RIGHT_THIGH      -> new int[]{22, 132};
            case RIGHT_CALF       -> new int[]{22, 170};
            case RIGHT_FOOT       -> new int[]{19, 190};
        };
    }

    /** 经脉 tooltip icon 锚点 — 取路径末端 */
    private static int[] meridianAnchor(MeridianChannel ch) {
        int[][] wp = MERIDIAN_PATHS.get(ch);
        if (wp == null || wp.length == 0) return new int[]{0, 0};
        return wp[wp.length - 1];
    }

    // ==================== Meridian path waypoints ====================

    /** 每条经脉的多段折线路径（cx 相对坐标），按画布 H=210 调教。 */
    private static final Map<MeridianChannel, int[][]> MERIDIAN_PATHS = new EnumMap<>(MeridianChannel.class);
    static {
        // ===== 手三阴 (左臂) — 从胸肩出发扇形展开到手部 =====
        // LU 肺经：胸前上 → 肩前外 → 前臂外侧 → 拇指端
        MERIDIAN_PATHS.put(MeridianChannel.LU, new int[][]{
            {-8, 40}, {-18, 50}, {-28, 74}, {-34, 100}, {-36, 112}
        });
        // HT 心经：腋下 → 肘内 → 腕尺 → 小指端（最内侧，贴体）
        MERIDIAN_PATHS.put(MeridianChannel.HT, new int[][]{
            {-16, 48}, {-22, 64}, {-26, 88}, {-28, 108}, {-26, 113}
        });
        // PC 心包经：胸中 → 肘中 → 腕中 → 中指端（中间路径）
        MERIDIAN_PATHS.put(MeridianChannel.PC, new int[][]{
            {-2, 50}, {-14, 66}, {-22, 86}, {-30, 107}, {-32, 113}
        });

        // ===== 手三阳 (右臂) — 镜像 =====
        MERIDIAN_PATHS.put(MeridianChannel.LI, new int[][]{
            {8, 40}, {18, 50}, {28, 74}, {34, 100}, {36, 112}
        });
        MERIDIAN_PATHS.put(MeridianChannel.SI, new int[][]{
            {16, 48}, {22, 64}, {26, 88}, {28, 108}, {26, 113}
        });
        MERIDIAN_PATHS.put(MeridianChannel.TE, new int[][]{
            {2, 50}, {14, 66}, {22, 86}, {30, 107}, {32, 113}
        });

        // ===== 足三阴 (左腿) — 从足端上行至腹 =====
        // SP 脾经：大趾 → 腿内侧中 → 腹侧
        MERIDIAN_PATHS.put(MeridianChannel.SP, new int[][]{
            {-17, 188}, {-14, 170}, {-11, 140}, {-8, 110}, {-14, 90}
        });
        // KI 肾经：足心 → 小腿内 → 大腿内 → 胸
        MERIDIAN_PATHS.put(MeridianChannel.KI, new int[][]{
            {-13, 190}, {-11, 170}, {-7, 140}, {-4, 105}, {-2, 72}
        });
        // LR 肝经：大趾外 → 腿内 → 胁
        MERIDIAN_PATHS.put(MeridianChannel.LR, new int[][]{
            {-15, 188}, {-12, 170}, {-9, 142}, {-6, 112}, {-10, 82}
        });

        // ===== 足三阳 (右腿) — 镜像 =====
        MERIDIAN_PATHS.put(MeridianChannel.ST, new int[][]{
            {17, 188}, {14, 170}, {11, 140}, {8, 110}, {14, 90}
        });
        MERIDIAN_PATHS.put(MeridianChannel.BL, new int[][]{
            {13, 190}, {11, 170}, {7, 140}, {4, 105}, {2, 72}
        });
        MERIDIAN_PATHS.put(MeridianChannel.GB, new int[][]{
            {15, 188}, {12, 170}, {9, 142}, {6, 112}, {10, 82}
        });

        // ===== 8 奇经 =====
        // 任脉：前正中，腹→咽
        MERIDIAN_PATHS.put(MeridianChannel.REN, new int[][]{
            {-3, 98}, {-3, 80}, {-3, 62}, {-3, 44}, {-3, 30}
        });
        // 督脉：后正中（右偏绘制以区分）
        MERIDIAN_PATHS.put(MeridianChannel.DU, new int[][]{
            {3, 98}, {3, 80}, {3, 62}, {3, 44}, {3, 30}
        });
        // 冲脉：深部正中
        MERIDIAN_PATHS.put(MeridianChannel.CHONG, new int[][]{
            {0, 94}, {0, 74}, {0, 54}, {0, 34}
        });
        // 带脉：腰间环行
        MERIDIAN_PATHS.put(MeridianChannel.DAI, new int[][]{
            {-20, 84}, {-10, 86}, {0, 87}, {10, 86}, {20, 84}
        });
        // 阴维：左侧躯干弧
        MERIDIAN_PATHS.put(MeridianChannel.YIN_WEI, new int[][]{
            {-12, 100}, {-16, 78}, {-16, 54}, {-10, 36}
        });
        // 阳维：右侧躯干弧
        MERIDIAN_PATHS.put(MeridianChannel.YANG_WEI, new int[][]{
            {12, 100}, {16, 78}, {16, 54}, {10, 36}
        });
        // 阴跷：内踝 → 内眼（腿内侧长链）
        MERIDIAN_PATHS.put(MeridianChannel.YIN_QIAO, new int[][]{
            {-8, 180}, {-6, 145}, {-5, 110}, {-4, 75}, {-2, 26}
        });
        // 阳跷：外踝 → 外眼
        MERIDIAN_PATHS.put(MeridianChannel.YANG_QIAO, new int[][]{
            {8, 180}, {6, 145}, {5, 110}, {4, 75}, {2, 26}
        });
    }

    // ==================== Hit detection ====================

    private boolean isOverBodyPart(BodyPart bp, int mx, int my) {
        if (bp == BodyPart.HEAD) return dist(mx, my, W / 2, 17) < 13;
        int[] r = bodyPartRect(bp);
        int cx = W / 2;
        return mx >= cx + r[0] && mx < cx + r[2] && my >= r[1] && my < r[3];
    }

    /** 鼠标到某条脉路径的最小距离（折线段距离）。 */
    private static double distanceToPath(int mx, int my, int cx, MeridianChannel ch) {
        int[][] wp = MERIDIAN_PATHS.get(ch);
        if (wp == null || wp.length < 2) return Double.MAX_VALUE;
        double best = Double.MAX_VALUE;
        for (int i = 0; i < wp.length - 1; i++) {
            double d = pointToSegmentDist(mx, my,
                cx + wp[i][0], wp[i][1],
                cx + wp[i + 1][0], wp[i + 1][1]);
            if (d < best) best = d;
        }
        return best;
    }

    private static double pointToSegmentDist(int px, int py, int x1, int y1, int x2, int y2) {
        double dx = x2 - x1, dy = y2 - y1;
        double len2 = dx * dx + dy * dy;
        if (len2 < 0.001) {
            double ddx = px - x1, ddy = py - y1;
            return Math.sqrt(ddx * ddx + ddy * ddy);
        }
        double t = ((px - x1) * dx + (py - y1) * dy) / len2;
        t = Math.max(0, Math.min(1, t));
        double cx = x1 + t * dx, cy = y1 + t * dy;
        double ddx = px - cx, ddy = py - cy;
        return Math.sqrt(ddx * ddx + ddy * ddy);
    }

    private static double dist(int x1, int y1, int x2, int y2) {
        return Math.sqrt((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2));
    }

    // ==================== Drawing primitives ====================

    /** 绘制从 (x1,y1) 到 (x2,y2) 的粗线，笔画宽度 thickness（以线段为中心）。 */
    private static void drawThickLine(OwoUIDrawContext ctx, int x1, int y1, int x2, int y2, int color, int thickness) {
        int dx = Math.abs(x2 - x1), dy = Math.abs(y2 - y1);
        int steps = Math.max(dx, dy);
        int half = thickness / 2;
        if (steps == 0) {
            ctx.fill(x1 - half, y1 - half, x1 - half + thickness, y1 - half + thickness, color);
            return;
        }
        float xi = (float) (x2 - x1) / steps;
        float yi = (float) (y2 - y1) / steps;
        float px = x1, py = y1;
        for (int i = 0; i <= steps; i++) {
            int ix = (int) px - half, iy = (int) py - half;
            ctx.fill(ix, iy, ix + thickness, iy + thickness, color);
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
