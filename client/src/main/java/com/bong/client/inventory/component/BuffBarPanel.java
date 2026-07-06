package com.bong.client.inventory.component;

import com.bong.client.combat.inspect.StatusPanelExtension;
import com.bong.client.combat.store.StatusEffectStore;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;

/**
 * Tarkov 背包检视界面内的 buff/状态效果横条 —— 补足"背包打开时看不到 buff"的缺口。
 *
 * <p>每帧直接读 {@link StatusEffectStore#snapshot()}（全部效果，不做 HUD 顶栏那种
 * Top-8 截断——背包界面空间富余，应该展示全部而非只挑优先级最高的 8 个）。无 buff
 * 时面板收起为 0×0，不占位、不留灰色空壳（HUD 沉浸原则：没有状态就不常驻显示）。
 *
 * <p>渲染走手绘 {@link BaseComponent#draw} 模式，与同目录 {@link StatusBarsPanel}/
 * {@link BottomInfoBar} 一致；视觉语言（sourceColor 边框 + kind 区分色块 + 层数角标 +
 * 底部剩余时间比例条）对齐 HUD 侧 {@code StatusEffectHudPlanner}。
 *
 * <p>Tooltip 同样走手绘悬浮框，而非 owo 原生 {@code .tooltip()}——本面板不是由多个
 * owo 子组件拼成（每个 buff 槽只是同一个 draw() 调用里画的一块矩形，没有对应的
 * owo Component 对象可挂 tooltip），命中测试只能在 draw() 里用当帧真实 mouseX/mouseY
 * 现场算。{@link #drawTooltip} 因此在 {@code InspectScreen.render()} 里 super.render()
 * 之后单独调用，逃出 owo 组件裁剪区——与 {@link BodyInspectComponent#drawTooltip} 同一
 * 先例。悬浮目标（{@link #hoveredEffect}）在每帧 draw() 里从当前 snapshot 重新计算，
 * 不跨帧缓存引用，因此不会出现"指向已消失 buff"的悬空引用。
 */
public class BuffBarPanel extends BaseComponent {
    static final int SLOT_SIZE = 20;
    static final int SLOT_GAP = 3;
    private static final int TRACK_BG = 0xC0101820;
    private static final int STACK_BADGE_COLOR = 0xFFFFE080;
    private static final int REMAINING_BAR_COLOR = 0xFFFFFFFF;
    private static final int NEGATIVE_REMAINING_BAR_COLOR = 0xFFFF4040;
    // 与 HUD 顶栏 StatusEffectHudPlanner 相同的 30s 归一化上限，视觉语言对齐。
    private static final long REMAINING_BAR_CAP_MS = 30_000L;
    private static final int TOOLTIP_BG_OUTER = 0xEE111122;
    private static final int TOOLTIP_BG_INNER = 0xEE1A1A2A;
    private static final int TOOLTIP_TEXT_COLOR = 0xFFEEEEEE;

    private int currentWidth = 0;
    private int currentHeight = 0;
    private StatusEffectStore.Effect hoveredEffect;

    public BuffBarPanel() {
        this.sizing(Sizing.fixed(0), Sizing.fixed(0));
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        List<StatusEffectStore.Effect> effects = sortedEffects(StatusEffectStore.snapshot());
        applySizing(effects.size());
        hoveredEffect = effectAtScreen(effects, x, y, mouseX, mouseY);
        if (effects.isEmpty()) return;

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        int sx = x;
        for (StatusEffectStore.Effect e : effects) {
            drawSlot(context, textRenderer, sx, y, e);
            sx += SLOT_SIZE + SLOT_GAP;
        }
    }

    private void drawSlot(OwoUIDrawContext ctx, TextRenderer tr, int sx, int sy, StatusEffectStore.Effect e) {
        // 边框：来源色（沿用 HUD 视觉语言）
        ctx.fill(sx, sy, sx + SLOT_SIZE, sy + SLOT_SIZE, e.sourceColor());
        // 内部底色
        ctx.fill(sx + 1, sy + 1, sx + SLOT_SIZE - 1, sy + SLOT_SIZE - 1, TRACK_BG);
        // kind 区分色块 —— buff 图标资源尚未成套，用色块+字符区分类型，与 sourceColor 边框互补。
        ctx.fill(sx + 2, sy + 2, sx + SLOT_SIZE - 2, sy + SLOT_SIZE - 2, kindTint(e.kind()));
        String glyph = kindGlyph(e.kind());
        int gw = tr.getWidth(glyph);
        ctx.drawTextWithShadow(tr, Text.literal(glyph),
            sx + (SLOT_SIZE - gw) / 2, sy + (SLOT_SIZE - tr.fontHeight) / 2, 0xFFFFFFFF);
        // 剩余时间比例条（底部 2px）
        float norm = remainingNorm(e.remainingMs());
        int barW = Math.max(0, Math.round((SLOT_SIZE - 4) * norm));
        if (barW > 0) {
            int barColor = isNegativeKind(e.kind()) ? NEGATIVE_REMAINING_BAR_COLOR : REMAINING_BAR_COLOR;
            ctx.fill(sx + 2, sy + SLOT_SIZE - 3, sx + 2 + barW, sy + SLOT_SIZE - 1, barColor);
        }
        // 层数角标
        if (e.stacks() >= 2) {
            String stackText = "×" + Math.min(99, e.stacks());
            ctx.drawTextWithShadow(tr, Text.literal(stackText),
                sx + SLOT_SIZE - tr.getWidth(stackText) - 1, sy + SLOT_SIZE - 9, STACK_BADGE_COLOR);
        }
    }

    private void applySizing(int count) {
        int w = requiredWidth(count);
        int h = requiredHeight(count);
        if (w != currentWidth || h != currentHeight) {
            currentWidth = w;
            currentHeight = h;
            // owo-lib Sizing 是 Observable，改值会触发 notifyParentIfMounted 让父 FlowLayout
            // 重新 inflate（与 ItemTooltipPanel 动态高度同一手法）——buff 全部消失时宽高归零，
            // 界面上这一整行随之收起，不留灰色空壳。
            this.sizing(Sizing.fixed(currentWidth), Sizing.fixed(currentHeight));
        }
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return currentWidth; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return currentHeight; }

    /**
     * 悬浮信息框——须在 {@code InspectScreen.render()} 里 super.render() 之后调用（逃出 owo
     * 组件裁剪区），且要用同一帧 draw() 已经算出的 mouseX/mouseY，否则命中框和绘制框错位。
     */
    public void drawTooltip(DrawContext ctx, int mouseX, int mouseY) {
        StatusEffectStore.Effect e = hoveredEffect;
        if (e == null) return;
        String[] lines = tooltipText(e).split("\n", -1);
        TextRenderer tr = MinecraftClient.getInstance().textRenderer;
        int maxW = 0;
        for (String l : lines) maxW = Math.max(maxW, tr.getWidth(l));
        int tw = maxW + 8;
        int th = lines.length * (tr.fontHeight + 1) + 4;
        int tx = mouseX + 8;
        int ty = mouseY - th - 4;
        if (ty < 0) ty = mouseY + 12;

        ctx.fill(tx - 2, ty - 2, tx + tw + 2, ty + th + 2, TOOLTIP_BG_OUTER);
        ctx.fill(tx - 1, ty - 1, tx + tw + 1, ty + th + 1, TOOLTIP_BG_INNER);
        int cy = ty + 2;
        for (String l : lines) {
            ctx.drawTextWithShadow(tr, Text.literal(l), tx + 2, cy, TOOLTIP_TEXT_COLOR);
            cy += tr.fontHeight + 1;
        }
    }

    // ==================== 纯函数：供单测直接断言，不摸 MinecraftClient ====================

    static int requiredWidth(int count) {
        if (count <= 0) return 0;
        return count * SLOT_SIZE + (count - 1) * SLOT_GAP;
    }

    static int requiredHeight(int count) {
        return count <= 0 ? 0 : SLOT_SIZE;
    }

    /** 按 HUD 顶栏同款优先级排序（DoT &gt; Control &gt; Debuff &gt; Buff &gt; Unknown），但不截断 Top-8。 */
    static List<StatusEffectStore.Effect> sortedEffects(List<StatusEffectStore.Effect> all) {
        List<StatusEffectStore.Effect> sorted = new ArrayList<>(all);
        sorted.sort(Comparator
            .comparingInt((StatusEffectStore.Effect e) -> StatusEffectStore.rank(e.kind()))
            .thenComparing(StatusEffectStore.Effect::remainingMs));
        return sorted;
    }

    /** 命中测试：给定已排序的 effect 列表和面板锚点，鼠标落在第几个槽上就返回对应 effect，否则 null。 */
    static StatusEffectStore.Effect effectAtScreen(
        List<StatusEffectStore.Effect> sorted, int panelX, int panelY, int mouseX, int mouseY
    ) {
        if (mouseY < panelY || mouseY >= panelY + SLOT_SIZE) return null;
        int sx = panelX;
        for (StatusEffectStore.Effect e : sorted) {
            if (mouseX >= sx && mouseX < sx + SLOT_SIZE) return e;
            sx += SLOT_SIZE + SLOT_GAP;
        }
        return null;
    }

    static float remainingNorm(long remainingMs) {
        if (remainingMs <= 0L) return 0f;
        float norm = remainingMs / (float) REMAINING_BAR_CAP_MS;
        return Math.max(0f, Math.min(1f, norm));
    }

    static boolean isNegativeKind(StatusEffectStore.Kind kind) {
        return kind == StatusEffectStore.Kind.DOT
            || kind == StatusEffectStore.Kind.CONTROL
            || kind == StatusEffectStore.Kind.DEBUFF;
    }

    static String kindGlyph(StatusEffectStore.Kind kind) {
        return switch (kind) {
            case DOT -> "毒";
            case CONTROL -> "控";
            case BUFF -> "增";
            case DEBUFF -> "减";
            case UNKNOWN -> "?";
        };
    }

    static int kindTint(StatusEffectStore.Kind kind) {
        return switch (kind) {
            case DOT -> 0x80E04040;
            case CONTROL -> 0x80B060FF;
            case BUFF -> 0x8060D060;
            case DEBUFF -> 0x80FFA030;
            case UNKNOWN -> 0x80808080;
        };
    }

    /**
     * 拼装悬浮 tooltip 文案——复用 {@link StatusPanelExtension#tooltipFor} 的名字/来源/驱散难度行，
     * 只把其中"剩余: X.Xs"那一行换成本面板专属的 60 秒进位格式（&ge;60s 显示 "Xm Ys"）。刻意不去
     * 改共享函数 {@code StatusPanelExtension.formatMs}——那是 HUD 等其它调用方也在用的稳定契约，
     * 改了会连带影响 HUD 侧的显示。
     */
    static String tooltipText(StatusEffectStore.Effect e) {
        String base = StatusPanelExtension.tooltipFor(e);
        String[] lines = base.split("\n", -1);
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < lines.length; i++) {
            String line = lines[i];
            if (line.startsWith("剩余: ")) {
                line = "剩余: " + formatRemaining(e.remainingMs());
            }
            if (i > 0) sb.append('\n');
            sb.append(line);
        }
        return sb.toString();
    }

    /**
     * &lt;60s → "X.Xs"；&ge;60s → "Xm Ys"（分钟数不补零，如 "1m 5s"）。
     * 用 floor 而非四舍五入取十分位，避免 59950~59999ms 这类边界被舍入显示成 "60.0s"
     * ——逻辑上仍属于 &lt;60s 档，却打出一个和下一档格式撞脸的文案。
     */
    static String formatRemaining(long ms) {
        long clamped = Math.max(0L, ms);
        if (clamped < 60_000L) {
            double tenths = Math.floor(clamped / 100.0) / 10.0;
            return String.format(Locale.ROOT, "%.1fs", tenths);
        }
        long totalSeconds = clamped / 1000L;
        long minutes = totalSeconds / 60L;
        long seconds = totalSeconds % 60L;
        return minutes + "m " + seconds + "s";
    }

    // ─── 测试专用访问器 ───────────────────────────────────────────────

    StatusEffectStore.Effect hoveredEffectForTest() { return hoveredEffect; }

    int currentWidthForTest() { return currentWidth; }

    int currentHeightForTest() { return currentHeight; }
}
