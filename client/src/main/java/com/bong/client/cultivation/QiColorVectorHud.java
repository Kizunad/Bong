package com.bong.client.cultivation;

import com.bong.client.inventory.model.MeridianBody;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public final class QiColorVectorHud extends BaseComponent {
    public static final int WIDTH = 168;
    public static final int HEIGHT = 74;
    private static final int BG = 0xEE101214;
    private static final int BORDER = 0xFF3A4658;
    private static final int TRACK = 0xFF25282C;
    private static final int TEXT = 0xFFE0E0E0;
    private static final int MUTED = 0xFF888888;
    private static final int HUNYUAN = 0xFF8EE8FF;
    private MeridianBody body;

    public QiColorVectorHud() {
        this.sizing(Sizing.fixed(WIDTH), Sizing.fixed(HEIGHT));
    }

    public void setBody(MeridianBody body) {
        this.body = body;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        context.fill(x, y, x + WIDTH, y + HEIGHT, BG);
        drawBorder(context);
        var tr = MinecraftClient.getInstance().textRenderer;
        context.drawTextWithShadow(tr, Text.literal("真元向量"), x + 6, y + 5, TEXT);

        if (body == null || body.qiColorPracticeWeights().isEmpty()) {
            context.drawTextWithShadow(tr, Text.literal("未记录"), x + 112, y + 5, MUTED);
            drawEmptyBars(context);
            return;
        }

        context.drawTextWithShadow(
            tr,
            Text.literal(body.qiColorHunyuan() ? "混元" : "未混"),
            x + 112,
            y + 5,
            body.qiColorHunyuan() ? HUNYUAN : MUTED
        );
        drawBars(context, body.qiColorPracticeWeights());
        context.drawTextWithShadow(
            tr,
            Text.literal(hunyuanDistanceText(body)),
            x + 6,
            y + 61,
            MUTED
        );
    }

    public static String hunyuanDistanceText(MeridianBody body) {
        if (body == null || body.qiColorPracticeWeights().isEmpty()) {
            return "缺 全部";
        }
        if (body.qiColorHunyuan()) {
            return "色种已齐";
        }
        List<ColorKind> missing = missingColors(body.qiColorPracticeWeights());
        if (missing.isEmpty()) {
            return "色种已齐";
        }
        StringBuilder sb = new StringBuilder("缺 ");
        for (int i = 0; i < missing.size() && i < 6; i++) {
            if (i > 0) sb.append("/");
            sb.append(missing.get(i).label());
        }
        if (missing.size() > 6) sb.append("…");
        return sb.toString();
    }

    public static List<ColorKind> missingColors(Map<ColorKind, Double> weights) {
        List<ColorKind> missing = new ArrayList<>();
        for (ColorKind color : ColorKind.values()) {
            double weight = weights == null ? 0.0 : weights.getOrDefault(color, 0.0);
            if (weight <= 0.0) {
                missing.add(color);
            }
        }
        return missing;
    }

    private void drawBars(OwoUIDrawContext context, Map<ColorKind, Double> weights) {
        double total = weights.values().stream()
            .filter(v -> v != null && Double.isFinite(v) && v > 0.0)
            .mapToDouble(Double::doubleValue)
            .sum();
        if (total <= 0.0) {
            drawEmptyBars(context);
            return;
        }
        int left = x + 6;
        int baseY = y + 54;
        int barMaxH = 30;
        int step = 16;
        int barW = 8;
        var tr = MinecraftClient.getInstance().textRenderer;
        ColorKind[] colors = ColorKind.values();
        for (int i = 0; i < colors.length; i++) {
            ColorKind color = colors[i];
            int bx = left + i * step;
            context.fill(bx, baseY - barMaxH, bx + barW, baseY, TRACK);
            double ratio = Math.max(0.0, Math.min(1.0, weights.getOrDefault(color, 0.0) / total));
            int fillH = (int) Math.round(ratio * barMaxH);
            if (fillH > 0) {
                context.fill(bx, baseY - fillH, bx + barW, baseY, color.argb());
            }
            context.drawTextWithShadow(tr, Text.literal(color.label()), bx, baseY + 2, MUTED);
        }
    }

    private void drawEmptyBars(OwoUIDrawContext context) {
        int left = x + 6;
        int baseY = y + 54;
        int step = 16;
        int barW = 8;
        for (int i = 0; i < ColorKind.values().length; i++) {
            int bx = left + i * step;
            context.fill(bx, baseY - 30, bx + barW, baseY, TRACK);
        }
    }

    private void drawBorder(OwoUIDrawContext context) {
        context.fill(x, y, x + WIDTH, y + 1, BORDER);
        context.fill(x, y + HEIGHT - 1, x + WIDTH, y + HEIGHT, BORDER);
        context.fill(x, y, x + 1, y + HEIGHT, BORDER);
        context.fill(x + WIDTH - 1, y, x + WIDTH, y + HEIGHT, BORDER);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return HEIGHT; }
}
