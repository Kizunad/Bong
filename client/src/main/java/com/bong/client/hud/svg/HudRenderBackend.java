package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

/** HUD 表现后端的替换边界；业务状态和几何资源不从这里反向读取。 */
@FunctionalInterface
public interface HudRenderBackend {
    HudRenderBackend NOOP = (context, client, visibility) -> { };

    void render(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility
    );
}
