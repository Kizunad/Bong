package com.bong.client.menu;

import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.Element;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ClickableWidget;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.OrderedText;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.Util;
import net.minecraft.util.math.MathHelper;

/** 固定取景的分层场景；主菜单不创建 ClientWorld 或临时实体。 */
final class MainMenuBackdrop {
    private static final Identifier BACKGROUND = texture("valley");
    private static final Identifier LEFT = texture("rock-left");
    private static final Identifier RIGHT = texture("rock-right");
    static final Identifier TITLE_FONT = new Identifier("bong", "menu-title");
    static final int PAPER = 0xFFE4E7DC;
    static final int MUTED = 0xFF9DAAA0;
    static final int RED = 0xFFB95440;
    private final long started = Util.getMeasuringTimeMs();
    private long enteredAt;
    private long lastFrame;
    private float lookX;
    private float lookY;
    private boolean motion = true;

    private static Identifier texture(String name) {
        return new Identifier("bong", "textures/gui/main_menu/" + name + ".png");
    }

    void motion(boolean value) {
        motion = value;
    }

    void beginEntering() {
        enteredAt = Util.getMeasuringTimeMs();
    }

    void returnToMenu() {
        enteredAt = 0;
    }

    void render(DrawContext context, int width, int height, int mouseX, int mouseY) {
        if (width <= 0 || height <= 0) {
            return;
        }
        long now = Util.getMeasuringTimeMs();
        float dt = Math.min(0.1f, lastFrame == 0 ? 0 : (now - lastFrame) / 1000f);
        lastFrame = now;
        float targetX = motion ? MathHelper.clamp(mouseX / (float) width - 0.5f, -0.5f, 0.5f) : 0;
        float targetY = motion ? MathHelper.clamp(mouseY / (float) height - 0.5f, -0.5f, 0.5f) : 0;
        float response = 1 - (float) Math.exp(-dt * 4);
        lookX += (targetX - lookX) * response;
        lookY += (targetY - lookY) * response;
        float progress = enteredAt == 0 || !motion ? 0 : MathHelper.clamp((now - enteredAt) / 1000f, 0, 1);
        float ease = progress * progress * (3 - 2 * progress);
        float cover = Math.max(width / 1536f, height / 1024f) * 1.055f;
        context.fill(0, 0, width, height, 0xFF1D2421);
        drawLayer(context, BACKGROUND, width, height, cover * (1 + ease * 0.045f), lookX * 3, lookY * 2);
        drawLayer(context, LEFT, width, height, cover * (1 + ease * 0.12f), lookX * 12 - ease * width * 0.18f, lookY * 7);
        drawLayer(context, RIGHT, width, height, cover * (1 + ease * 0.12f), lookX * 12 + ease * width * 0.18f, lookY * 7);
        if (motion) {
            drawDust(context, width, height, (now - started) / 1000.0);
        }
    }

    private static void drawLayer(DrawContext context, Identifier id, int width, int height, float scale, float x, float y) {
        context.getMatrices().push();
        context.getMatrices().translate((width - 1536 * scale) / 2 + x, (height - 1024 * scale) / 2 + y, 0);
        context.getMatrices().scale(scale, scale, 1);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.setShaderColor(1, 1, 1, 1);
        context.drawTexture(id, 0, 0, 0, 0, 1536, 1024, 1536, 1024);
        context.getMatrices().pop();
    }

    private static void drawDust(DrawContext context, int width, int height, double time) {
        // 固定小集合，按绝对时间计算，切屏不积累粒子或分配世界对象。
        for (int i = 0; i < 28; i++) {
            double seed = i * 2.399963;
            double speed = i % 7 == 0 ? -0.003 : 0.009;
            double horizontal = (i * 0.618034 + time * speed) % 1;
            if (horizontal < 0) horizontal += 1;
            int x = (int) (horizontal * width);
            int y = (int) ((0.28 + ((i * 0.381966) % 0.65)) * height + Math.sin(seed + time * 0.2) * 7);
            int alpha = (int) (22 + 36 * (0.5 + 0.5 * Math.sin(seed + time * 0.55)));
            context.fill(x, y, x + 1, y + 1, (alpha << 24) | 0xCCD5C2);
        }
    }

    static void drawBrand(DrawContext context, int x, int y) {
        var font = MinecraftClient.getInstance().textRenderer;
        Text title = Text.literal("末法残土").styled(style -> style.withFont(TITLE_FONT));
        float scale = Math.min(3.8f, 164f / Math.max(1, font.getWidth(title)));
        context.getMatrices().push();
        context.getMatrices().translate(x, y, 0);
        context.getMatrices().scale(scale, scale, 1);
        context.drawText(font, title, 0, 0, PAPER, false);
        context.getMatrices().pop();
        context.drawText(font, "MOFA CANTU", x + 2, y + 46, MUTED, false);
        context.fill(x + 2, y + 65, x + 26, y + 66, RED);
    }

    static void drawCommand(DrawContext context, ClickableWidget button, boolean selected) {
        drawCommandDecoration(context, button, selected);
        context.drawCenteredTextWithShadow(MinecraftClient.getInstance().textRenderer, button.getMessage(),
            button.getX() + button.getWidth() / 2, button.getY() + (button.getHeight() - 8) / 2,
            !button.active ? 0xFF626D66 : selected ? PAPER : MUTED);
    }

    static void drawCommandDecoration(DrawContext context, ClickableWidget button, boolean selected) {
        int x = button.getX();
        int y = button.getY();
        int width = button.getWidth();
        int height = button.getHeight();
        if (selected && button.active) {
            context.fill(x, y + 4, x + 2, y + height - 4, RED);
            context.fill(x + 8, y + height - 1, x + width, y + height, 0x667D8A7F);
        }
        if (selected && button.active) {
            context.drawText(MinecraftClient.getInstance().textRenderer, ">", x + width - 10,
                y + (height - 8) / 2, RED, false);
        }
    }

    static void renderConnectionText(DrawContext context, Screen screen, Text status, int mouseX, int mouseY) {
        context.fill(0, 0, screen.width, screen.height, 0x660E1712);
        var font = MinecraftClient.getInstance().textRenderer;
        context.drawCenteredTextWithShadow(font, Text.translatable("bong.menu.entering"), screen.width / 2, screen.height / 2 - 38, PAPER);
        int y = screen.height / 2 - 14;
        for (OrderedText line : font.wrapLines(status, Math.max(120, screen.width - 60))) {
            context.drawCenteredTextWithShadow(font, line, screen.width / 2, y, MUTED);
            y += font.fontHeight + 2;
        }
        drawNativeButtons(context, screen, mouseX, mouseY);
    }

    static void renderDisconnectedText(DrawContext context, Screen screen, MainMenuReasonWidget reason,
                                       int mouseX, int mouseY, float delta) {
        context.fill(0, 0, screen.width, screen.height, 0xBB0E1712);
        var font = MinecraftClient.getInstance().textRenderer;
        context.drawCenteredTextWithShadow(font, Text.translatable("bong.menu.disconnected"), screen.width / 2,
            Math.max(12, reason.getY() - 22), PAPER);
        reason.render(context, mouseX, mouseY, delta);
        drawNativeButtons(context, screen, mouseX, mouseY);
    }

    private static void drawNativeButtons(DrawContext context, Screen screen, int mouseX, int mouseY) {
        for (Element child : screen.children()) {
            if (child instanceof ButtonWidget widget && widget.visible) {
                drawCommand(context, widget, widget.isFocused() || widget.isMouseOver(mouseX, mouseY));
            }
        }
    }
}
