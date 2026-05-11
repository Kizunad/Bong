package com.bong.client.spirittreasure;

import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.OrderedText;
import net.minecraft.text.Text;

import java.util.List;

public abstract class SpiritTreasureTabPanel {
    protected final SpiritTreasureState state;
    protected final List<SpiritTreasureDialogue> dialogues;

    protected SpiritTreasureTabPanel(SpiritTreasureState state, List<SpiritTreasureDialogue> dialogues) {
        this.state = state;
        this.dialogues = List.copyOf(dialogues == null ? List.of() : dialogues);
    }

    public abstract void render(DrawContext context, TextRenderer renderer, int left, int top, int width, int height);

    protected static int drawWrapped(
        DrawContext context,
        TextRenderer renderer,
        String text,
        int x,
        int y,
        int maxWidth,
        int color,
        int maxLines
    ) {
        List<OrderedText> lines = renderer.wrapLines(Text.literal(text == null ? "" : text), maxWidth);
        int count = Math.min(lines.size(), maxLines);
        for (int i = 0; i < count; i++) {
            context.drawTextWithShadow(renderer, lines.get(i), x, y + i * 10, color);
        }
        return y + count * 10;
    }
}
