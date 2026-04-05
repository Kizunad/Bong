package com.bong.client;

import net.minecraft.client.gui.DrawContext;

public class BongHud {


    public static void render(DrawContext context, float tickDelta) {
        context.drawTextWithShadow(
            net.minecraft.client.MinecraftClient.getInstance().textRenderer,
            "Bong Client Connected",
            10,
            10,
            0xFFFFFF
        );
    }
}
