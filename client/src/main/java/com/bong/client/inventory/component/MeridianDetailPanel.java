package com.bong.client.inventory.component;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

/**
 * 经脉详情面板 — 竖向窄条，贴在 body 右侧。
 * 高度与 body canvas 对齐（210），不占用额外纵向空间。
 */
public class MeridianDetailPanel extends BaseComponent {
    public static final int WIDTH = 86;
    public static final int HEIGHT = 210;

    private MeridianBody body;
    private MeridianChannel focused;

    public MeridianDetailPanel() {
        this.sizing(Sizing.fixed(WIDTH), Sizing.fixed(HEIGHT));
    }

    public void setBody(MeridianBody body) { this.body = body; }
    public void setFocused(MeridianChannel ch) { this.focused = ch; }

    @Override
    public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY, float partialTicks, float delta) {
        int bx = x, by = y;

        // 背景
        ctx.fill(bx, by, bx + WIDTH, by + HEIGHT, 0xEE141420);
        ctx.fill(bx, by, bx + WIDTH, by + 1, 0x44FFFFFF);
        ctx.fill(bx, by + HEIGHT - 1, bx + WIDTH, by + HEIGHT, 0x44000000);
        ctx.fill(bx, by, bx + 1, by + HEIGHT, 0x33FFFFFF);
        ctx.fill(bx + WIDTH - 1, by, bx + WIDTH, by + HEIGHT, 0x33000000);

        TextRenderer tr = MinecraftClient.getInstance().textRenderer;

        if (focused == null || body == null) {
            ctx.drawTextWithShadow(tr, Text.literal("§8经脉详情"), bx + 6, by + 6, 0xFF888888);
            ctx.drawTextWithShadow(tr, Text.literal("§8悬浮或"), bx + 6, by + HEIGHT / 2 - 10, 0xFF666666);
            ctx.drawTextWithShadow(tr, Text.literal("§8点击脉"), bx + 6, by + HEIGHT / 2 + 2, 0xFF666666);
            return;
        }

        ChannelState cs = body.channel(focused);
        if (cs == null) {
            ctx.drawTextWithShadow(tr, Text.literal(focused.displayName()),
                bx + 5, by + 6, focused.baseColor());
            ctx.drawTextWithShadow(tr, Text.literal("§7（未录入）"),
                bx + 5, by + 22, 0xFF666666);
            return;
        }

        int cy = by + 5;

        // === 经脉名 ===
        String name = focused.displayName();
        // 中文经脉名 5 char ~30px，80 宽放得下整行；保险起见两行切分
        if (tr.getWidth(name) <= WIDTH - 10) {
            ctx.drawTextWithShadow(tr, Text.literal(name), bx + 5, cy, focused.baseColor());
            cy += 11;
        } else {
            int mid = name.length() / 2;
            ctx.drawTextWithShadow(tr, Text.literal(name.substring(0, mid)), bx + 5, cy, focused.baseColor());
            cy += 10;
            ctx.drawTextWithShadow(tr, Text.literal(name.substring(mid)), bx + 5, cy, focused.baseColor());
            cy += 10;
        }

        String fam = focused.family() == MeridianChannel.Family.REGULAR ? "§8正经" : "§d奇经";
        ctx.drawTextWithShadow(tr, Text.literal(fam), bx + 5, cy, 0xFFAAAAAA);
        cy += 12;

        // 分隔线
        ctx.fill(bx + 4, cy, bx + WIDTH - 4, cy + 1, 0x33FFFFFF);
        cy += 4;

        // === 损伤等级 ===
        ctx.drawTextWithShadow(tr, Text.literal(cs.damage().label()),
            bx + 5, cy, cs.damage().color());
        cy += 14;

        // 分隔
        ctx.fill(bx + 4, cy, bx + WIDTH - 4, cy + 1, 0x22FFFFFF);
        cy += 4;

        // === 流量 ===
        ctx.drawTextWithShadow(tr, Text.literal("§7流量"), bx + 5, cy, 0xFFAAAAAA);
        cy += 10;
        ctx.drawTextWithShadow(tr, Text.literal(
            String.format("§f%.0f§8/§7%.0f", cs.currentFlow(), cs.capacity())),
            bx + 5, cy, 0xFFAAAAAA);
        cy += 10;
        drawBar(ctx, bx + 5, cy, WIDTH - 10, 4, cs.flowRatio(),
            0xFF44AACC, 0xFF223344);
        cy += 8;

        // === 污染 ===
        if (cs.contamination() > 0.01) {
            ctx.drawTextWithShadow(tr, Text.literal(
                String.format("§7污染 §d%.0f%%", cs.contamination() * 100)),
                bx + 5, cy, 0xFFAAAAAA);
            cy += 10;
            drawBar(ctx, bx + 5, cy, WIDTH - 10, 3, cs.contamination(),
                0xFF9944CC, 0xFF2A1A3A);
            cy += 7;
        } else {
            ctx.drawTextWithShadow(tr, Text.literal("§8污染 无"), bx + 5, cy, 0xFF666666);
            cy += 11;
        }

        // === 恢复 / 封闭 ===
        if (cs.healProgress() > 0.01) {
            ctx.drawTextWithShadow(tr, Text.literal(
                String.format("§7恢复 §a%.0f%%", cs.healProgress() * 100)),
                bx + 5, cy, 0xFFAAAAAA);
            cy += 10;
            drawBar(ctx, bx + 5, cy, WIDTH - 10, 3, cs.healProgress(),
                0xFF44AA66, 0xFF1A2A1F);
            cy += 7;
        } else if (cs.blocked()) {
            ctx.drawTextWithShadow(tr, Text.literal("§c已封闭"), bx + 5, cy, 0xFFCC4444);
            cy += 11;
        } else {
            ctx.drawTextWithShadow(tr, Text.literal("§8恢复 —"), bx + 5, cy, 0xFF666666);
            cy += 11;
        }

        // === 描述（剩余空间自动换行） ===
        String desc = focused.description();
        if (desc != null && !desc.isEmpty() && cy < by + HEIGHT - 14) {
            cy += 3;
            ctx.fill(bx + 4, cy, bx + WIDTH - 4, cy + 1, 0x22FFFFFF);
            cy += 4;

            int maxW = WIDTH - 10;
            List<String> lines = wrapChinese(tr, desc, maxW);
            for (String line : lines) {
                if (cy > by + HEIGHT - 10) break;
                ctx.drawTextWithShadow(tr, Text.literal("§7" + line), bx + 5, cy, 0xFF999999);
                cy += 10;
            }
        }
    }

    /** 按像素宽度切分中文文本（CJK 按字，非 CJK 按字符） */
    private static List<String> wrapChinese(TextRenderer tr, String text, int maxW) {
        List<String> out = new ArrayList<>();
        StringBuilder cur = new StringBuilder();
        int curW = 0;
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            int cw = tr.getWidth(String.valueOf(c));
            if (curW + cw > maxW && cur.length() > 0) {
                out.add(cur.toString());
                cur.setLength(0);
                curW = 0;
            }
            cur.append(c);
            curW += cw;
        }
        if (cur.length() > 0) out.add(cur.toString());
        return out;
    }

    /** 水平进度条 */
    private static void drawBar(OwoUIDrawContext ctx, int bx, int by, int bw, int bh, double ratio,
                                int fgColor, int bgColor) {
        if (bw <= 0) return;
        ratio = Math.max(0, Math.min(1, ratio));
        ctx.fill(bx, by, bx + bw, by + bh, bgColor);
        int fillW = (int) (bw * ratio);
        if (fillW > 0) ctx.fill(bx, by, bx + fillW, by + bh, fgColor);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return WIDTH; }
    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return HEIGHT; }
}
