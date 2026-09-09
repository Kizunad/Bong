package com.bong.client.menu;

import net.minecraft.client.font.MultilineText;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.narration.NarrationMessageBuilder;
import net.minecraft.client.gui.screen.narration.NarrationPart;
import net.minecraft.client.gui.widget.ScrollableWidget;
import net.minecraft.text.Text;

/** 复用原版滚动和焦点行为，完整保留断线说明。 */
public final class MainMenuReasonWidget extends ScrollableWidget {
    private final TextRenderer font;
    private MultilineText lines;
    private final int lineHeight;

    public MainMenuReasonWidget(Text reason, TextRenderer font, int screenWidth, int screenHeight) {
        super(0, 0, 1, 1, reason);
        this.font = font;
        lineHeight = font.fontHeight + 2;
        relayout(screenWidth, screenHeight);
    }

    public void relayout(int screenWidth, int screenHeight) {
        int nextWidth = Math.max(1, Math.min(360, screenWidth - 50));
        MultilineText nextLines = MultilineText.create(font, getMessage(), Math.max(1, nextWidth - getPaddingDoubled()));
        width = nextWidth;
        lines = nextLines;
        height = Math.min(Math.max(1, screenHeight - 100), getContentsHeight() + getPaddingDoubled());
        setX((screenWidth - width) / 2);
        setY((screenHeight - height - 48) / 2);
        setScrollY(getScrollY());
    }

    @Override
    protected int getContentsHeight() {
        return lines.count() * lineHeight;
    }

    @Override
    protected double getDeltaYPerScroll() {
        return lineHeight;
    }

    @Override
    protected void renderContents(DrawContext context, int mouseX, int mouseY, float delta) {
        lines.drawCenterWithShadow(context, getX() + width / 2, getY() + getPadding(),
            lineHeight, MainMenuBackdrop.MUTED);
    }

    @Override
    protected void drawBox(DrawContext context) {
        if (isFocused()) {
            context.drawBorder(getX(), getY(), width, height, 0x667D8A7F);
        }
    }

    @Override
    protected void appendClickableNarrations(NarrationMessageBuilder builder) {
        builder.put(NarrationPart.TITLE, getMessage());
    }
}
