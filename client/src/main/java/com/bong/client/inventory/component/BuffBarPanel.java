package com.bong.client.inventory.component;

import com.bong.client.combat.inspect.StatusPanelExtension;
import com.bong.client.combat.StatusEffectIcons;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.hud.HudTextureProbe;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/**
 * Tarkov 背包检视界面内的 buff/状态效果横条 —— 补足"背包打开时看不到 buff"的缺口。
 *
 * <p>每帧直接读 {@link StatusEffectStore#snapshot()}（全部效果，不做 HUD 顶栏那种
 * Top-8 截断——背包界面空间富余，应该展示全部而非只挑优先级最高的 8 个）。无 buff
 * 时面板收起为 0×0，不占位、不留灰色空壳（HUD 沉浸原则：没有状态就不常驻显示）。
 *
 * <p>渲染走手绘 {@link BaseComponent#draw} 模式，与同目录 {@link StatusBarsPanel}/
 * {@link BottomInfoBar} 一致；通过 {@link StatusEffectIcons} 与 HUD 共用具体状态的 PNG，
 * 保留来源色边框、层数角标和有限时长的剩余比例条。
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
    // 背包条的有限时长按 30 秒归一化；持续效果不显示倒计时条。
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
        String texture = StatusEffectIcons.textureFor(e.id());
        if (HudTextureProbe.exists(texture)) {
            RenderSystem.enableBlend();
            RenderSystem.defaultBlendFunc();
            try {
                int size = SLOT_SIZE - 4;
                ctx.drawTexture(new Identifier(texture), sx + 2, sy + 2,
                    size, size, 0, 0, 1, 1, 1, 1);
            } finally {
                RenderSystem.disableBlend();
            }
        } else {
            // 未知状态也不能被误称为“毒”：降级时显示它自己的名称首字。
            String name = e.displayName();
            String glyph = name.isBlank() ? "?" : name.substring(0, name.offsetByCodePoints(0, 1));
            ctx.drawTextWithShadow(tr, Text.literal(glyph),
                sx + (SLOT_SIZE - tr.getWidth(glyph)) / 2, sy + (SLOT_SIZE - tr.fontHeight) / 2, 0xFFFFFFFF);
        }
        // 剩余时间比例条（底部 2px）
        float norm = remainingNorm(e.remainingMs());
        int barW = Math.max(0, Math.round((SLOT_SIZE - 4) * norm));
        if (barW > 0 && !e.indefinite()) {
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

    /** 状态详情与背包条使用同一份持续时间、来源和驱散信息。 */
    static String tooltipText(StatusEffectStore.Effect e) {
        return StatusPanelExtension.tooltipFor(e);
    }

    // ─── 测试专用访问器 ───────────────────────────────────────────────

    StatusEffectStore.Effect hoveredEffectForTest() { return hoveredEffect; }

    int currentWidthForTest() { return currentWidth; }

    int currentHeightForTest() { return currentHeight; }
}
