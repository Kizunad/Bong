package com.bong.client.ui.preview;

import com.bong.client.BongHud;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudWidgetWindows;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import com.bong.client.ui.window.UiWindowRuntime;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;

/** 复用生产 HUD 和窗口输入，验证布局还原不会释放库存窗口。 */
final class UiHudWindowPreviewScene implements UiPreviewScene {
    private boolean hudOnly;

    @Override public void installFixture(UiPreviewConfig config) {
        hudOnly = false;
        UiWindowRuntime.beginPreview();
        UiWindowRuntime.manager().reset();
        InventoryStateStore.replace(InventoryModel.empty());
        CombatHudStateStore.replaceAuthoritative(
            CombatHudState.createAuthoritative(.92f, .64f, .82f, DerivedAttrFlags.none(), true));
    }

    @Override public Screen createScreen() {
        return new InspectScreen(InventoryStateStore.snapshot()) {
            @Override public void render(DrawContext context, int mouseX, int mouseY, float delta) {
                if (!hudOnly) { super.render(context, mouseX, mouseY, delta); return; }
                context.fill(0, 0, width, height, 0xFF3A4641);
                context.draw();
                var client = MinecraftClient.getInstance();
                client.currentScreen = null;
                try { BongHud.render(context, delta, com.bong.client.hud.svg.SvgHudBackend.production()); }
                finally { client.currentScreen = this; }
            }
        };
    }
    @Override public String selectedTemplateId(Screen screen) { return "hud-widget"; }
    @Override public boolean isReady(Screen screen) { return ((InspectScreen) screen).windowHostReadyForPreview(); }
    @Override public boolean initializationFailed(Screen screen) { return ((InspectScreen) screen).windowHostFailedForPreview(); }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        var client = MinecraftClient.getInstance();
        var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        render(screen, context);
        var manager = UiWindowRuntime.manager();
        var business = manager.snapshot().get(0);
        for (var state : manager.snapshot()) manager.minimize(state.key());
        clickControl(screen, "workspace-hud");
        render(screen, context);
        var hud = UiWindowRuntime.hud();
        var widget = HudWidgetWindows.Widget.MINI_BODY;
        if (!hud.active(widget)) throw new IllegalStateException("预览没有生产人体 HUD 命令");
        clickControl(screen, widget.id(), "hud-visible");
        if (hud.visible(widget)) throw new IllegalStateException("列表复选框未隐藏 HUD");
        clickControl(screen, widget.id(), "hud-visible");
        if (!hud.visible(widget)) throw new IllegalStateException("列表复选框未恢复 HUD");
        var edit = UiWindowRuntime.workspaceControlBoundsForPreview(widget.id(), "hud-edit");
        var row = UiWindowRuntime.workspaceControlBoundsForPreview(widget.id());
        if (edit.x() + edit.width() > row.x() + row.width()) throw new IllegalStateException("HUD 编辑按钮被挤出列表");
        clickControl(screen, widget.id(), "hud-edit");
        render(screen, context);
        var state = manager.snapshot().get(manager.snapshot().size() - 1);
        var bounds = state.bounds();
        click(screen, bounds.x() + bounds.width() - 32, bounds.y() + 10);
        if (hud.visible(widget)) throw new IllegalStateException("锁定按钮未取消 HUD 显示");
        render(screen, context);
        click(screen, bounds.x() + bounds.width() - 32, bounds.y() + 10);
        if (!hud.visible(widget)) throw new IllegalStateException("锁定按钮未恢复 HUD 显示");
        double x = bounds.x() + 8, y = bounds.y() + 10;
        screen.mouseClicked(x, y, 0);
        screen.mouseDragged(x + 30, y + 20, 0, 30, 20);
        screen.mouseReleased(x + 30, y + 20, 0);
        render(screen, context);
        bounds = state.bounds();
        click(screen, bounds.x() + bounds.width() - 10, bounds.y() + 10);
        if (hud.visible(widget) || UiWindowRuntime.layoutHudCommands(BongHud.workspaceCommands()).stream()
            .anyMatch(command -> command.layer() == HudRenderLayer.MINI_BODY)) {
            throw new IllegalStateException("关闭 HUD 窗口后原面板仍在输出");
        }
        clickControl(screen, "workspace-reset");
        if (!hud.visible(widget) || business.scope().isClosed() || business.minimized()
            || !manager.contains(business.key())) throw new IllegalStateException("一键还原破坏了业务窗口或未恢复 HUD");
        for (var window : manager.snapshot()) if (HudWidgetWindows.widget(window.key()) == null) manager.minimize(window.key());
        UiWindowRuntime.openHud(widget);
        render(screen, context);
        if (shot.name().contains("gameplay")) hudOnly = true;
        else clickControl(screen, "workspace-hud");
        render(screen, context);
    }

    private static void clickControl(Screen screen, String... path) {
        var bounds = UiWindowRuntime.workspaceControlBoundsForPreview(path);
        click(screen, bounds.x() + bounds.width() / 2.0, bounds.y() + bounds.height() / 2.0);
    }

    private static void click(Screen screen, double x, double y) {
        screen.mouseClicked(x, y, 0);
        screen.mouseReleased(x, y, 0);
    }

    private static void render(Screen screen, DrawContext context) {
        screen.render(context, -1, -1, 0);
        context.draw();
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        if (hudOnly) {
            var client = MinecraftClient.getInstance();
            try (var pixels = net.minecraft.client.util.ScreenshotRecorder.takeScreenshot(client.getFramebuffer())) {
                int background = pixels.getColor(0, 0) & 0xFFFFFF;
                int tint = com.bong.client.visual.season.SeasonVisuals.skyTintArgb(
                    com.bong.client.state.SeasonStateStore.snapshot(), System.currentTimeMillis());
                // 生产 HUD 保留季节全屏叠色，背景像素应包含这一层。
                double alpha = (tint >>> 24) / 255.0;
                for (int shift = 0; shift <= 16; shift += 8) {
                    int expected = (int) Math.round(((0x3A4641 >>> shift) & 255) * (1 - alpha)
                        + ((tint >>> shift) & 255) * alpha);
                    int actual = (background >>> (16 - shift)) & 255;
                    if (Math.abs(actual - expected) > 2) throw new IllegalStateException("HUD 帧包含非预期背景");
                }
                int changes = 0;
                for (int y = 0; y < pixels.getHeight(); y += 4) {
                    for (int x = 0; x < pixels.getWidth(); x += 4) {
                        if ((pixels.getColor(x, y) & 0xFFFFFF) != background) changes++;
                    }
                }
                if (changes < 40) throw new IllegalStateException("关闭工作台后的 HUD 帧为空");
            }
        }
        for (var window : UiWindowRuntime.manager().snapshot()) {
            if (window.minimized()) continue;
            var bounds = window.bounds();
            if (bounds.x() < 0 || bounds.y() < 0
                || bounds.x() + bounds.width() > shot.expectedLogicalWidth()
                || bounds.y() + bounds.height() > shot.expectedLogicalHeight() - 28) {
                throw new IllegalStateException("HUD 编辑窗口越出工作台: " + bounds);
            }
        }
    }

    @Override public void cleanup() {
        UiPreviewCleanup.run(
            () -> UiWindowRuntime.manager().reset(),
            () -> UiWindowRuntime.endPreview(),
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect()
        );
    }
}
