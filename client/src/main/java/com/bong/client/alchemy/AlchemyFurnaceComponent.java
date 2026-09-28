package com.bong.client.alchemy;

import com.mojang.blaze3d.systems.RenderSystem;
import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongEntityRegistry;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.ui.model.ModelPreviewComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.util.ScissorStack;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;
import net.minecraft.util.math.Box;

import java.util.List;
import java.util.function.Consumer;
import java.util.function.Function;

/** 工位背景、真实丹炉模型和各部位悬浮提示共享同一套画布坐标。 */
public final class AlchemyFurnaceComponent extends ModelPreviewComponent {
    private static final Identifier BACKDROP = new Identifier("bong-client", "textures/gui/alchemy/workbench-v2.png");
    private final AlchemyWindows windows;
    private final Function<Part, List<String>> details;
    private final Consumer<Part> interact;
    private boolean hovered;
    private boolean draggingMaterial;
    private Part hoveredPart;
    private int detailScroll;
    private int panelLeft, panelTop, panelWidth, panelHeight;
    private final AlchemyWorkbenchEffects effects;
    private BongModeledEntity preview;
    private static final List<String> SHORTCUTS = List.of("I 起炉", "J 降火", "K 升火", "F 注元", "R 收取结果", "N 丹方", "H 炉记");
    private static final List<String> COMPACT_SHORTCUTS = List.of("I炉", "J↓", "K↑", "F元", "R取", "N方", "H记");

    record Artwork(int x, int y, int width, int height) {}

    // 布局：渲染、悬停、拖料共用完整画布。错误提示浮在画布上，不改变炉口位置。
    private Artwork artwork() {
        int artHeight = Math.max(1, height - footerHeight());
        double scale = Math.min(width / 768.0, artHeight / 512.0);
        int imageWidth = Math.max(1, (int) Math.round(768 * scale));
        int imageHeight = Math.max(1, (int) Math.round(512 * scale));
        return new Artwork(x + (width - imageWidth) / 2, y + (artHeight - imageHeight) / 2,
            imageWidth, imageHeight);
    }

    public com.bong.client.ui.window.UiWindowManager.Rect canvasBounds() {
        var art = artwork();
        return new com.bong.client.ui.window.UiWindowManager.Rect(art.x, art.y, art.width, art.height);
    }

    private int footerHeight() {
        var text = MinecraftClient.getInstance().textRenderer;
        int gap = compact() ? 6 : 16;
        int used = 0;
        int rows = 1;
        for (String shortcut : shortcuts()) {
            int size = text.getWidth(shortcut) + gap;
            if (used > 0 && used + size - gap > width - footerPadding() * 2) {
                rows++;
                used = 0;
            }
            used += size;
        }
        return rows * footerLineHeight() + footerPadding();
    }

    private boolean compact() { return width < 240 || height < 150; }

    private List<String> shortcuts() { return compact() ? COMPACT_SHORTCUTS : SHORTCUTS; }

    private int footerPadding() { return compact() ? 4 : 12; }

    private int footerLineHeight() { return compact() ? 11 : 15; }

    /** 相对于完整工位画布，不随窗口长宽比漂移。 */
    public enum Part {
        LID("炉盖", .46, .17, .20, .10),
        MOUTH("炉口", .40, .28, .30, .16),
        BODY("炉身", .40, .45, .25, .20),
        FIRE("火门", .43, .72, .22, .14),
        QI("引元处", .65, .44, .10, .17),
        OUTLET("出料处", .64, .64, .11, .14),
        INCENSE("香座", .06, .84, .24, .12),
        RECIPE("桌上丹方", .04, .32, .26, .23),
        JOURNAL("桌上炉记", .04, .59, .26, .23);

        private final String title;
        private final double left, top, width, height;

        Part(String title, double left, double top, double width, double height) {
            this.title = title;
            this.left = left;
            this.top = top;
            this.width = width;
            this.height = height;
        }
    }

    public AlchemyFurnaceComponent(AlchemyWindows windows, Function<Part, List<String>> details,
                                   Consumer<Part> interact) {
        this.windows = windows;
        this.details = details;
        this.interact = interact;
        effects = new AlchemyWorkbenchEffects(windows);
        // 工位保持炉体竖直，避免模型目录的俯仰角使丹炉看起来后仰。
        camera().focus(new Vec3d(.5, .5, .5), 1.2f, 25, 0, false);
        camera().settle();
    }

    public boolean hovered() { return hovered; }

    public Part hoveredPart() { return hoveredPart; }

    public void draggingMaterial(boolean dragging) {
        draggingMaterial = dragging;
    }

    public void cancelInput() {
        hovered = false;
        hoveredPart = null;
        detailScroll = 0;
        draggingMaterial = false;
    }

    public boolean furnaceAt(double mouseX, double mouseY) {
        // 桌面上的丹方和炉记也是独立部位，需要自己的悬浮信息，
        // 不能因为不属于炉体就被 hover 状态过滤掉。
        return partAt(mouseX, mouseY) != null;
    }

    public boolean mouthAt(double mouseX, double mouseY) {
        return partAt(mouseX, mouseY) == Part.MOUTH
            && (!detailsVisible() || !panelAt(mouseX, mouseY));
    }

    public boolean incenseAt(double mouseX, double mouseY) {
        return partAt(mouseX, mouseY) == Part.INCENSE
            && (!detailsVisible() || !panelAt(mouseX, mouseY));
    }

    // 输入：命中使用绝对屏幕坐标；owo 的局部事件坐标只在入口转换一次。
    private Part partAt(double mouseX, double mouseY) {
        var art = artwork();
        for (var part : Part.values()) {
            if (mouseX >= art.x + art.width * part.left && mouseX < art.x + art.width * (part.left + part.width)
                && mouseY >= art.y + art.height * part.top && mouseY < art.y + art.height * (part.top + part.height)) return part;
        }
        return null;
    }

    @Override public boolean onMouseDown(double mouseX, double mouseY, int button) {
        // 信息窗不把点击穿透给下方的取料口或引元处。
        if (draggingMaterial || detailsVisible() && panelAt(x + mouseX, y + mouseY)) return true;
        var part = partAt(x + mouseX, y + mouseY);
        if (button == 0 && part != null) {
            interact.accept(part);
            return true;
        }
        return false;
    }

    @Override public boolean onMouseScroll(double mouseX, double mouseY, double amount) {
        if (detailsVisible() && panelAt(x + mouseX, y + mouseY)) {
            detailScroll = Math.max(0, detailScroll - (int) (amount * 11));
            return true;
        }
        if (draggingMaterial) return true;
        if (partAt(x + mouseX, y + mouseY) == Part.FIRE) {
            windows.temperature(Math.signum(amount) * .02);
            return true;
        }
        if (hovered) {
            detailScroll = Math.max(0, detailScroll - (int) (amount * 11));
            return true;
        }
        return false;
    }

    private boolean panelAt(double mouseX, double mouseY) {
        return mouseX >= panelLeft && mouseX < panelLeft + panelWidth
            && mouseY >= panelTop && mouseY < panelTop + panelHeight;
    }

    private boolean detailsVisible() {
        return hovered && !draggingMaterial;
    }

    public void tick() {
        effects.tick(preview);
    }

    // 合成顺序：工房 → 地面和火焰 → 炉体 → 投料与结果 → 部位提示 → 操作提示。
    @Override public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        boolean overPanel = detailsVisible() && panelAt(mouseX, mouseY);
        if (!overPanel) {
            var next = partAt(mouseX, mouseY);
            if (next != hoveredPart) detailScroll = 0;
            hoveredPart = next;
        }
        hovered = overPanel || furnaceAt(mouseX, mouseY);
        if (!hovered) detailScroll = 0;
        context.draw();
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        ScissorStack.push(x, y, width, height, context.getMatrices());
        try {
            var art = artwork();
            context.fill(x, y, x + width, y + height, 0xFF191B18);
            // 余下边缘延续工房色调；完整的交互画布仍等比绘制，热区不随裁剪漂移。
            context.drawTexture(BACKDROP, x, y, width, height, 0, 0, 768, 512, 768, 512);
            context.fill(x, y, x + width, y + height, 0xAD121612);
            context.drawTexture(BACKDROP, art.x, art.y, art.width, art.height, 0, 0, 768, 512, 768, 512);
            effects.drawBehindFurnace(context, art);
            drawFurnace(context, mouseX, mouseY, partialTicks, delta, art);
            context.getMatrices().push();
            context.getMatrices().translate(0, 0, 120);
            try {
                effects.drawInFrontOfFurnace(context, art);
                drawMarkers(context);
                if (detailsVisible()) drawDetails(context);
                if (draggingMaterial && (hoveredPart == Part.MOUTH || hoveredPart == Part.INCENSE)) {
                    var target = hoveredPart == Part.INCENSE ? Part.INCENSE : Part.MOUTH;
                    context.drawRectOutline(art.x + (int) (art.width * target.left),
                        art.y + (int) (art.height * target.top), (int) (art.width * target.width),
                        (int) (art.height * target.height), 0xB8C1C7A2);
                }
                drawFooter(context);
            } finally {
                context.draw();
                context.getMatrices().pop();
            }
        } finally {
            context.draw();
            ScissorStack.pop();
        }
    }

    private void drawMarkers(OwoUIDrawContext context) {
        if (hoveredPart == null || draggingMaterial) return;
        var art = artwork();
        int px = art.x + (int) (art.width * (hoveredPart.left + hoveredPart.width / 2));
        int py = art.y + (int) (art.height * (hoveredPart.top + hoveredPart.height / 2));
        context.fill(px - 1, py - 1, px + 2, py + 2, 0xFFE1CDA2);
    }


    private void drawFooter(OwoUIDrawContext context) {
        var text = MinecraftClient.getInstance().textRenderer;
        int footer = y + height - footerHeight();
        context.fillGradient(x, footer - 12, x + width, y + height, 0, 0xF5141916);
        int padding = footerPadding();
        int gap = compact() ? 6 : 16;
        int left = x + padding, top = footer + padding / 2;
        var shortcuts = shortcuts();
        for (int index = 0; index < shortcuts.size(); index++) {
            String shortcut = shortcuts.get(index);
            int size = text.getWidth(shortcut) + gap;
            if (left > x + padding && left + size - gap > x + width - padding) {
                left = x + padding;
                top += footerLineHeight();
            }
            boolean available = index >= 5 || (index == 0 ? windows.ready() && !windows.model().furnace().hasSession()
                : index == 4 ? windows.model().furnace().hasSession() : windows.active());
            int color = available ? 0xFFD4C7AC : 0xFF737C72;
            context.drawText(text, shortcut.substring(0, 1), left, top, available ? 0xFFE4BA74 : color, false);
            context.drawText(text, shortcut.substring(1), left + text.getWidth(shortcut.substring(0, 1)), top, color, false);
            left += size;
        }
        var lines = text.wrapLines(Text.literal(windows.feedback()), width - 20);
        int cursor = footer - Math.min(2, lines.size()) * 12 - 6;
        if (!windows.feedback().isBlank()) {
            context.fill(x + 6, cursor - 4, x + width - 6, footer - 3, 0xEA30241D);
        }
        for (var line : lines.stream().limit(2).toList()) {
            context.drawText(text, line, x + 10, cursor, 0xFFF0AA88, false);
            cursor += 11;
        }
    }


    private void drawFurnace(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks,
                             float delta, Artwork art) {
        int oldX = x, oldY = y, oldWidth = width, oldHeight = height;
        try {
            x = art.x + (int) (art.width * .30);
            y = art.y + (int) (art.height * .02);
            width = (int) (art.width * .54);
            height = (int) (art.height * .98);
            super.draw(context, mouseX, mouseY, partialTicks, delta);
        } finally {
            x = oldX;
            y = oldY;
            width = oldWidth;
            height = oldHeight;
        }
    }

    @Override protected void drawBackdrop(OwoUIDrawContext context) {}

    @Override protected Box framingBounds(Box previous, Box current) {
        // 顶盖弹起属于动画，不能让镜头跟着缩小炉身并永久改变热区对应关系。
        return previous == null ? current : previous;
    }

    @Override protected void collectModel(MatrixStack matrices, VertexConsumerProvider vertices, float partialTicks) {
        var client = MinecraftClient.getInstance();
        if (preview == null) preview = BongEntityRegistry.type(BongEntityModelKind.ALCHEMY_FURNACE).create(client.world);
        if (preview == null) return;
        client.getEntityRenderDispatcher().getRenderer(preview).render(preview, 0, partialTicks,
            matrices, vertices, LightmapTextureManager.MAX_LIGHT_COORDINATE);
    }

    @Override public void close() {
        preview = null;
        super.close();
    }

    // 悬浮：每个部位独立取内容，贴近目标摆放。只有正文实际放不下时才允许滚动。
    private void drawDetails(OwoUIDrawContext context) {
        var text = MinecraftClient.getInstance().textRenderer;
        var lines = details.apply(hoveredPart);
        // 小笺的纸边与标题留白随窗口压缩；文字保持字号，正文滚动阅读。
        int margin = compact() ? 2 : 8;
        int inset = compact() ? 8 : 18;
        int titleTop = compact() ? 5 : 14;
        int bodyTop = compact() ? 19 : 32;
        int bottom = compact() ? 6 : 10;
        int contentWidth = lines.stream().mapToInt(text::getWidth).max().orElse(0);
        panelWidth = Math.max(1, Math.min(Math.max(112, contentWidth + inset * 2), Math.min(220, width - margin * 2)));
        var wrapped = lines.stream()
            .flatMap(line -> text.wrapLines(Text.literal(line), Math.max(1, panelWidth - inset * 2)).stream()).toList();
        int lineHeight = compact() ? 11 : 13;
        panelHeight = Math.max(1, Math.min(height - footerHeight() - margin * 2, bodyTop + bottom + wrapped.size() * lineHeight));
        var art = artwork();
        int anchorX = art.x + (int) (art.width * (hoveredPart.left + hoveredPart.width / 2));
        int anchorY = art.y + (int) (art.height * (hoveredPart.top + hoveredPart.height / 2));
        int preferredX = hoveredPart.left < .35 ? anchorX + 16 : anchorX - panelWidth - 16;
        int left = Math.max(x + margin, Math.min(x + width - panelWidth - margin, preferredX));
        int top = Math.max(y + margin, Math.min(y + height - footerHeight() - panelHeight - margin, anchorY - panelHeight / 2));
        panelLeft = left;
        panelTop = top;
        // 部位小笺与工房共用旧纸、墨色和朱砂，避免通用状态框覆盖场景。
        context.fill(left + margin, top + margin, left + panelWidth - 1, top + panelHeight + 2, 0x38080705);
        AlchemyPaperSurface.draw(context, AlchemyPaperSurface.JOURNAL, left, top, panelWidth, panelHeight, compact() ? 7 : 14);
        context.fill(left + inset, top + bodyTop - 5, left + panelWidth - inset, top + bodyTop - 4, 0x50744933);
        context.drawText(text, hoveredPart == null ? "炉况" : hoveredPart.title,
            left + inset, top + titleTop, 0xFF783C2C, false);
        int visibleHeight = Math.max(1, panelHeight - bodyTop - bottom);
        detailScroll = Math.min(detailScroll, Math.max(0, wrapped.size() * lineHeight - visibleHeight));
        ScissorStack.push(left + inset - 2, top + bodyTop, Math.max(1, panelWidth - inset * 2 + 4), visibleHeight, context.getMatrices());
        try {
            int cursor = top + bodyTop - detailScroll;
            for (var line : wrapped) {
                context.drawText(text, line, left + inset, cursor, 0xFF382F24, false);
                cursor += lineHeight;
            }
        } finally {
            context.draw();
            ScissorStack.pop();
        }
        if (wrapped.size() * lineHeight > visibleHeight) context.drawText(text, "⋮", left + panelWidth - inset - 6, top + titleTop, 0xFF785D40, false);
    }
}
