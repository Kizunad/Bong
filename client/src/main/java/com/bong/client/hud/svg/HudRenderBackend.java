package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.HudRenderCommand;
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

    /** 返回 true 时，BongHud 会把该 command 的几何提交交给本后端。 */
    default boolean handles(HudRenderCommand command) {
        return false;
    }

    /** 按原 HUD command 顺序提交一个由本后端负责的几何命令。 */
    default void renderCommand(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        HudRenderCommand command
    ) {
    }
}
