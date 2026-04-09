package com.bong.client;

import com.bong.client.hud.BongHudOrchestrator;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

import java.util.List;

public class BongHud {
    private static final int HUD_TEXT_MAX_WIDTH = 220;

    public static void render(DrawContext context, float tickDelta) {
        MinecraftClient client = MinecraftClient.getInstance();
        long nowMillis = System.currentTimeMillis();
        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateStore.snapshot(),
            nowMillis,
            client.textRenderer::getWidth,
            HUD_TEXT_MAX_WIDTH,
            client.getWindow().getScaledWidth(),
            client.getWindow().getScaledHeight()
        );

        for (HudRenderCommand command : commands) {
            if (command.isText()) {
                context.drawTextWithShadow(client.textRenderer, command.text(), command.x(), command.y(), command.color());
                continue;
            }

            if (command.isToast()) {
                BongToast.render(
                    context,
                    client.textRenderer,
                    client.getWindow().getScaledWidth(),
                    client.getWindow().getScaledHeight(),
                    command
                );
            }
        }

        for (HudRenderCommand command : commands) {
            if (command.isScreenTint()) {
                context.fill(0, 0, client.getWindow().getScaledWidth(), client.getWindow().getScaledHeight(), command.color());
            }
        }
    }
}
