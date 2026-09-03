package com.bong.client.hud;

import com.bong.client.BongHud;
import com.bong.client.hud.svg.HudRenderBackend;
import net.minecraft.client.gui.DrawContext;

import java.util.Objects;

/** HUD Fabric 回调的组合对象：业务 HUD 只依赖被注入的表现接口。 */
public final class BongHudRenderer {
    private final HudRenderBackend backend;

    public BongHudRenderer(HudRenderBackend backend) {
        this.backend = Objects.requireNonNull(backend, "backend");
    }

    public void render(DrawContext context, float tickDelta) {
        BongHud.render(context, tickDelta, backend);
    }
}
