package com.bong.client.ui.preview;

import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.model.ModelPreviewCatalog;
import com.bong.client.ui.model.ModelPreviewComponent;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.UiWindowRuntime;
import io.wispforest.owo.ui.core.Component;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;

/** 用真实服务端权限和原生 Screen 输入验证模型目录，不注入权限、不改玩家数据。 */
final class UiModelWindowPreviewScene implements UiPreviewScene {
    private UiWindowManager.WindowState window;
    private boolean ordinary;

    @Override public boolean clientReady(MinecraftClient client) {
        return client.world != null && client.player != null && client.getNetworkHandler() != null
            && client.currentScreen == null && ScreenTransitionController.activeTransition() == null
            && client.getNetworkHandler().getCommandDispatcher().getRoot().getChild("ping") != null;
    }
    @Override public void installFixture(UiPreviewConfig config) { UiWindowRuntime.beginPreview(); }
    @Override public Screen createScreen() { return new InspectScreen(InventoryStateStore.snapshot()); }
    @Override public String selectedTemplateId(Screen screen) { return "model-preview"; }
    @Override public boolean isReady(Screen screen) { return ((InspectScreen) screen).windowHostReadyForPreview(); }
    @Override public boolean initializationFailed(Screen screen) { return ((InspectScreen) screen).windowHostFailedForPreview(); }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        var manager = UiWindowRuntime.manager();
        for (var state : manager.snapshot()) manager.close(state.key());
        ordinary = shot.name().equals("ordinary");
        require(UiWindowRuntime.canUseModelPreview() != ordinary, "当前登录用户与权限验收场景不匹配");
        UiWindowRuntime.openModelPreview();
        if (ordinary) {
            require(manager.snapshot().stream().noneMatch(state -> state.definition().equals(UiWindowRuntime.MODEL_PREVIEW)),
                "普通玩家仍能直接打开模型目录");
            render(screen);
            return;
        }
        window = manager.snapshot().stream().filter(state -> state.definition().equals(UiWindowRuntime.MODEL_PREVIEW))
            .findFirst().orElseThrow();
        manager.settleAt(window.key(), new UiWindowManager.Rect(8, 8,
            Math.min(620, shot.expectedLogicalWidth() - 16), shot.expectedLogicalHeight() - 44));
        render(screen);
        boolean gray = shot.name().startsWith("gray__");
        String target = (gray ? shot.name().substring(6) : shot.name()).replace("--", ":");
        if (!target.equals("player")) {
            var entry = ModelPreviewCatalog.entries().stream().filter(value -> value.id().equals(target)).findFirst().orElseThrow();
            var categories = component("model-categories");
            int ordinal = entry.category().ordinal();
            click(screen, categories.x() + (ordinal % 2) * categories.width() / 2.0 + 8,
                categories.y() + (ordinal / 2) * 22 + 8);
            var search = component("model-search");
            click(screen, search.x() + 8, search.y() + 8);
            for (char ch : target.toCharArray()) screen.charTyped(ch, 0);
            render(screen);
            var list = component("model-list");
            click(screen, list.x() + 12, list.y() + 12);
            render(screen);
        }
        var preview = (ModelPreviewComponent) component("model-preview");
        require(preview.option().id().equals(target), "分类、搜索或点击未切换模型：" + target);
        var controls = component("model-controls");
        click(screen, controls.x() + 8, controls.y() + 8);
        require(!preview.camera().autoRotate(), "停止旋转控件未生效");
        double x = preview.x() + preview.width() / 2.0, y = preview.y() + preview.height() / 2.0;
        float yaw = preview.camera().yaw();
        screen.mouseClicked(x, y, 0);
        screen.mouseDragged(x + 20, y + 10, 0, 20, 10);
        screen.mouseReleased(x + 20, y + 10, 0);
        require(preview.camera().yaw() > yaw, "局部坐标下的左键拖拽未生效");
        yaw = preview.camera().yaw();
        screen.mouseClicked(x, y, 1);
        screen.mouseDragged(x - 12, y, 1, -12, 0);
        screen.mouseReleased(x - 12, y, 1);
        require(preview.camera().yaw() < yaw, "右键拖拽没有跟随拖拽方向");
        screen.mouseScrolled(x, y, 2);
        require(preview.camera().zoom() > 1, "滚轮未缩放模型");
        click(screen, controls.x() + controls.width() * 5 / 6.0, controls.y() + 8);
        require(preview.camera().zoom() == 1, "重新取景未恢复完整模型");
        click(screen, controls.x() + controls.width() / 2.0, controls.y() + 8);
        require(!preview.material(), "材质开关未切换至灰模");
        if (!gray) click(screen, controls.x() + controls.width() / 2.0, controls.y() + 8);
        require(preview.material() != gray, "材质恢复未生效");
        manager.minimize(window.key());
        render(screen);
        manager.restore(window.key());
        render(screen);
        require(preview.option().id().equals(target), "最小化恢复后丢失模型选择");
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        if (ordinary) return;
        var preview = (ModelPreviewComponent) component("model-preview");
        require(preview.failure() == null, "模型渲染失败：" + preview.option().id());
        require(window.bounds().y() + window.bounds().height() <= shot.expectedLogicalHeight() - 28,
            "模型窗口遮住了底部工作台栏");
        for (String id : new String[]{"model-categories", "model-search", "model-list", "model-preview", "model-controls"}) {
            var c = component(id);
            require(c.width() > 0 && c.height() > 0 && c.x() >= window.bounds().x() && c.y() >= window.bounds().y()
                && c.x() + c.width() <= window.bounds().x() + window.bounds().width()
                && c.y() + c.height() <= window.bounds().y() + window.bounds().height(), "控件越出窗口：" + id);
        }
    }
    private Component component(String id) { return UiWindowRuntime.windowContentForPreview(window.key()).childById(Component.class, id); }
    private static void click(Screen screen, double x, double y) { screen.mouseClicked(x, y, 0); screen.mouseReleased(x, y, 0); }
    private static void render(Screen screen) {
        var client = MinecraftClient.getInstance();
        var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        screen.render(context, -1, -1, 0);
        context.draw();
    }
    private static void require(boolean condition, String message) { if (!condition) throw new IllegalStateException(message); }
    @Override public void cleanup() { UiWindowRuntime.manager().reset(); UiWindowRuntime.endPreview(); }
}
