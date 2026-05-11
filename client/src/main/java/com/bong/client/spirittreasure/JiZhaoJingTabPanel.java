package com.bong.client.spirittreasure;

import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;

public final class JiZhaoJingTabPanel extends SpiritTreasureTabPanel {
    public JiZhaoJingTabPanel(SpiritTreasureState state, java.util.List<SpiritTreasureDialogue> dialogues) {
        super(state, dialogues);
    }

    @Override
    public void render(DrawContext context, TextRenderer renderer, int left, int top, int width, int height) {
        long nowMs = System.currentTimeMillis();
        JiZhaoJingMirrorRenderer.render(context, left + 12, top + 10, 72, state.affinity(), nowMs);
        context.drawTextWithShadow(renderer, Text.literal(state.displayName()), left + 96, top + 8, 0xFFF1F4F8);
        context.drawTextWithShadow(renderer, Text.literal(state.sourceLine()), left + 96, top + 20, 0xFF9AA3B2);
        context.drawTextWithShadow(
            renderer,
            Text.literal(state.equipped() ? "已装备" : "在囊中"),
            left + 96,
            top + 32,
            state.equipped() ? 0xFF9FD3A0 : 0xFFF1B15B
        );
        context.drawTextWithShadow(
            renderer,
            Text.literal(state.sleeping() ? "器灵沉睡" : "器灵清醒"),
            left + 96,
            top + 44,
            state.sleeping() ? 0xFFAAA8A8 : 0xFF9FD3A0
        );
        context.drawTextWithShadow(
            renderer,
            Text.literal("好感 " + Math.round(state.affinity() * 100.0) + "%"),
            left + 96,
            top + 56,
            0xFFD7E2F0
        );

        int bodyY = top + 94;
        drawWrapped(
            context,
            renderer,
            "器灵记忆",
            left + 12,
            bodyY,
            width - 24,
            0xFFF1F4F8,
            1
        );
        int y = bodyY + 12;
        if (dialogues.isEmpty()) {
            context.drawTextWithShadow(renderer, Text.literal("暂无器灵对话"), left + 12, y, 0xFF888888);
            return;
        }

        int limit = Math.min(dialogues.size(), 4);
        for (int i = dialogues.size() - limit; i < dialogues.size(); i++) {
            SpiritTreasureDialogue dialogue = dialogues.get(i);
            context.drawTextWithShadow(
                renderer,
                Text.literal("[" + dialogue.tone() + "] " + dialogue.text()),
                left + 12,
                y,
                0xFFD7E2F0
            );
            y += 12;
        }
    }
}
