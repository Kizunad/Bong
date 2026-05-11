package com.bong.client.npc;

import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

public final class NpcReputationIndicator {
    public static final int WIDTH = 60;
    public static final int HEIGHT = 4;

    private NpcReputationIndicator() {
    }

    public static String labelFor(int reputation) {
        if (reputation > 50) {
            return "信任";
        }
        if (reputation < -50) {
            return "敌视";
        }
        if (reputation < 0) {
            return "提防";
        }
        return "中立";
    }

    public static int colorFor(int reputation) {
        if (reputation > 50) {
            return 0xFF4DAA66;
        }
        if (reputation < -50) {
            return 0xFFE05A47;
        }
        if (reputation < 0) {
            return 0xFFCC7A2B;
        }
        return 0xFF8C8C8C;
    }

    public static int fillWidth(int reputation) {
        double normalized = Math.max(-100, Math.min(100, reputation));
        return Math.max(1, (int) Math.round(WIDTH * ((normalized + 100.0) / 200.0)));
    }

    public static void draw(DrawContext context, TextRenderer textRenderer, int x, int y, int reputation) {
        context.drawTextWithShadow(textRenderer, "§7信誉 §f" + labelFor(reputation), x, y, 0xD8D8D8);
        int barY = y + 12;
        context.fill(x, barY, x + WIDTH, barY + HEIGHT, 0xAA101018);
        context.fill(x, barY, x + fillWidth(reputation), barY + HEIGHT, colorFor(reputation));
    }
}
