package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.HudRenderCommand;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

/** HUD 表现后端的替换边界；业务状态和几何资源不从这里反向读取。 */
@FunctionalInterface
public interface HudRenderBackend {
    HudRenderBackend NOOP = (context, client, visibility) -> { };

    /** 在原命令位置提交图形，保留它与文字、物品、遮罩的前后关系。 */
    default void renderVector(DrawContext context, MinecraftClient client, HudRenderCommand command) {
    }

    void render(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility
    );
}
