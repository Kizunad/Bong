package com.bong.client.ui.preview;

import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SkillConfigStore;
import com.bong.client.combat.inspect.SkillConfigSchemaRegistry;
import com.bong.client.combat.inspect.SkillConfigWindows;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.component.BodyInspectComponent;
import com.bong.client.practice.PracticeWindows;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.UiWindowRuntime;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import io.wispforest.owo.ui.core.Component;
import io.wispforest.owo.ui.component.LabelComponent;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.util.ScreenshotRecorder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.lwjgl.glfw.GLFW;

/** 原生输入覆盖窗口叠放、搜索、快捷绑定与配置草稿；不冒充服务端回执。 */
final class UiTechniqueWindowPreviewScene implements UiPreviewScene {
    private static final String SKILL = "zhenmai.sever_chain";
    private static final String DASH = "movement.dash", PREVIEW_DASH = "preview.dash_variant";
    private final List<JsonObject> requests = new ArrayList<>();
    private List<TechniquesListPanel.Technique> savedTechniques;
    private SkillBarConfig savedBar;
    private SkillSetSnapshot savedExperience;
    private Map<String, JsonObject> savedConfigs;

    @Override public boolean clientReady(MinecraftClient client) {
        return client.world != null && client.player != null && client.getNetworkHandler() != null
            && client.currentScreen == null && com.bong.client.ui.ScreenTransitionController.activeTransition() == null
            && client.getNetworkHandler().getCommandDispatcher().getRoot().getChild("ping") != null;
    }

    @Override public void installFixture(UiPreviewConfig config) {
        UiWindowRuntime.beginPreview();
        UiWindowRuntime.manager().reset();
        savedTechniques = TechniquesListPanel.snapshot();
        savedBar = SkillBarStore.snapshot();
        savedExperience = SkillSetStore.snapshot();
        savedConfigs = SkillConfigStore.snapshot();
        SkillBarStore.replace(SkillBarConfig.empty());
        SkillConfigStore.replace(Map.of(SKILL, SkillConfigSchemaRegistry.defaultConfig(SKILL)));
        TechniquesListPanel.replace(List.of(
            technique("sword.thrust", "刺", "向前刺出，集中力道贯穿目标。", "skill", .34f, List.of(),
                "bong-client:textures/gui/items/skill_scroll_sword_thrust.png"),
            technique(SKILL, "绝脉", "截断一条经脉，将反震之力沿断口释放。", "skill", .71f, List.of(),
                "bong-client:textures/gui/skill/zhenmai_sever_chain.png"),
            technique(DASH, "闪身步", "侧身避开来势，短促发力，迅速脱离原位。", "dash", .42f,
                List.of(new TechniquesListPanel.RequiredMeridian("kidney", .2f)),
                "bong-client:textures/hud/dash/sidestep.png"),
            technique(PREVIEW_DASH, "闪避候选 · 仅预览", "仅用于验证替换与对比窗口，不加入游戏功法目录。", "dash", .17f,
                List.of(), "bong-client:textures/hud/dash/sidestep.png")));
        SkillSetStore.replace(SkillSetSnapshot.of(Map.of(
            SkillId.COMBAT, new SkillSetSnapshot.Entry(3, 410, 800, 1710, 5, 0, 0),
            SkillId.CULTIVATION, new SkillSetSnapshot.Entry(2, 110, 400, 610, 5, 0, 0),
            SkillId.MINERAL, new SkillSetSnapshot.Entry(6, 930, 2000, 7230, 4, 0, 0),
            SkillId.ALCHEMY, new SkillSetSnapshot.Entry(1, 90, 200, 190, 5, 0, 0))));
        requests.clear();
        ClientRequestSender.setBackendForTests((channel, bytes) -> requests.add(
            JsonParser.parseString(new String(bytes, StandardCharsets.UTF_8)).getAsJsonObject()));
    }

    @Override public Screen createScreen() { return new InspectScreen(InventoryStateStore.snapshot()); }
    @Override public String selectedTemplateId(Screen screen) { return "practice-catalog"; }
    @Override public boolean isReady(Screen screen) { return ((InspectScreen) screen).windowHostReadyForPreview(); }
    @Override public boolean initializationFailed(Screen screen) { return ((InspectScreen) screen).windowHostFailedForPreview(); }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        if (shot.name().startsWith("practice-")) {
            preparePracticeShot(screen, shot);
            UiWindowRuntime.previewMotion(true);
            return;
        }
        var manager = UiWindowRuntime.manager();
        for (var state : manager.snapshot()) manager.close(state.key());
        UiWindowRuntime.openPractice();
        var window = state(com.bong.client.practice.PracticeWindows.CATALOG.windowType());
        manager.settleAt(window.key(), new UiWindowManager.Rect(4, 4,
            Math.min(400, shot.expectedLogicalWidth() - 8), shot.expectedLogicalHeight() - 36));
        render(screen);
        var search = component(window, "practice-search");
        click(screen, search);
        for (char character : "sever".toCharArray()) screen.charTyped(character, 0);
        require(UiWindowRuntime.hasKeyboardFocus(), "搜索框未持有窗口键盘焦点");
        require(UiWindowRuntime.practice().visibleEntries().size() == 1 && UiWindowRuntime.practice().visibleEntries().get(0).key().equals("technique:" + SKILL), "搜索没有选中绝脉");
        manager.minimize(window.key());
        render(screen);
        UiWindowRuntime.openPractice();
        render(screen);
        require(UiWindowRuntime.practice().visibleEntries().size() == 1 && UiWindowRuntime.practice().visibleEntries().get(0).key().equals("technique:" + SKILL), "最小化恢复丢失搜索选择");

        var results = component(window, "practice-results");
        screen.mouseClicked(results.x() + 12, results.y() + 30, 0);
        screen.mouseReleased(results.x() + 12, results.y() + 30, 0);
        screen.mouseClicked(results.x() + 12, results.y() + 30, 0);
        screen.mouseReleased(results.x() + 12, results.y() + 30, 0);
        render(screen);
        var detail = state(com.bong.client.practice.PracticeWindows.DETAIL.windowType());
        require(detail != null, "双击未打开独立详情");
        UiWindowRuntime.openPracticeBinding(SKILL);
        render(screen);
        var binding = state(com.bong.client.practice.PracticeWindows.BINDING.windowType());
        click(screen, component(binding, "bind-combat-0"));
        render(screen);
        require(requests.stream().anyMatch(request -> "technique_bind".equals(request.get("type").getAsString())
            && SKILL.equals(request.get("skill_id").getAsString())), "绑定未发送真实请求");
        require(SkillBarStore.snapshot().slot(0) == null, "本地发送后不得虚构权威绑定");
        manager.close(binding.key());
        UiWindowRuntime.openSkillConfig(TechniquesListPanel.snapshot().stream().filter(t -> t.id().equals(SKILL)).findFirst().orElseThrow());
        var config = state(SkillConfigWindows.DEFINITION.windowType());
        manager.settleAt(config.key(), new UiWindowManager.Rect(Math.max(8, shot.expectedLogicalWidth() - 272),
            12, 260, Math.min(300, shot.expectedLogicalHeight() - 48)));
        render(screen);
        var selector = component(config, "config-backfire_kind");
        click(screen, selector);
        render(screen);
        click(screen, component(config, "option-physical_carrier"));
        render(screen);
        manager.minimize(config.key());
        render(screen);
        manager.restore(config.key());
        render(screen);
        require(component(config, "config-backfire_kind") instanceof io.wispforest.owo.ui.component.ButtonComponent button
            && button.getMessage().getString().contains("载体"), "配置草稿在最小化后丢失");
        click(screen, component(config, "config-save"));
        require(config.closed(), "保存后配置窗口没有关闭");
        require(requests.stream().anyMatch(request -> "skill_config_intent".equals(request.get("type").getAsString())
            && request.toString().contains("physical_carrier")), "保存没有提交实际选择");
        UiWindowRuntime.openSkillConfig(TechniquesListPanel.snapshot().stream().filter(t -> t.id().equals(SKILL)).findFirst().orElseThrow());
        var newConfig = state(SkillConfigWindows.DEFINITION.windowType());
        manager.settleAt(newConfig.key(), config.bounds());
        render(screen);
        UiWindowRuntime.previewMotion(true);
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        for (var window : UiWindowRuntime.manager().snapshot()) {
            var bounds = window.bounds();
            require(bounds.x() >= 0 && bounds.y() >= 0 && bounds.x() + bounds.width() <= shot.expectedLogicalWidth()
                && bounds.y() + bounds.height() <= shot.expectedLogicalHeight() - 28, "窗口越出工作台");
        }
        if (shot.name().startsWith("practice-") && !shot.name().equals("practice-meridian")) {
            assertPracticeTextVisible();
        }
    }

    /** 几何与输入命中都可能正常，但正文仍被裁剪；必须检查真实 framebuffer。 */
    private static void assertPracticeTextVisible() {
        var windows = UiWindowRuntime.manager().snapshot();
        var front = windows.get(windows.size() - 1);
        var content = UiWindowRuntime.windowContentForPreview(front.key());
        var labels = new ArrayList<LabelComponent>();
        content.forEachDescendant(component -> {
            if (component instanceof LabelComponent label && !label.text().getString().isBlank()
                && label.x() >= content.x() && label.y() >= content.y()
                && label.x() + label.width() <= content.x() + content.width()
                && label.y() + label.height() <= content.y() + content.height()) {
                labels.add(label);
            }
        });
        require(!labels.isEmpty(), front.key().windowType() + "没有可见正文标签");
        var client = MinecraftClient.getInstance();
        double scale = client.getWindow().getScaleFactor();
        try (NativeImage image = ScreenshotRecorder.takeScreenshot(client.getFramebuffer())) {
            for (var label : labels) {
                assertLabelVisible(image, scale, label, front.key().windowType());
            }
        }
    }

    private static void assertLabelVisible(NativeImage image, double scale, LabelComponent label, String windowType) {
        int left = Math.max(0, (int) Math.floor(label.x() * scale));
        int top = Math.max(0, (int) Math.floor(label.y() * scale));
        int right = Math.min(image.getWidth(), (int) Math.ceil((label.x() + label.width()) * scale));
        int bottom = Math.min(image.getHeight(), (int) Math.ceil((label.y() + label.height() + 1) * scale));
        int foregroundPixels = 0;
        for (int y = top; y < bottom; y++) {
            for (int x = left; x < right; x++) {
                int color = image.getColor(x, y);
                // 正文亮于墨色窗底；不锁定字体栅格或逐像素快照。
                if (Math.max(color & 255, Math.max((color >>> 8) & 255, (color >>> 16) & 255)) > 110) {
                    foregroundPixels++;
                }
            }
        }
        require(foregroundPixels >= Math.max(4, (int) Math.ceil(scale * scale)),
            windowType + "正文未绘入截图，可能被覆盖或裁剪：" + label.text().getString());
    }

    private static UiWindowManager.WindowState state(String type) {
        return UiWindowRuntime.manager().snapshot().stream().filter(window -> window.key().windowType().equals(type))
            .findFirst().orElseThrow();
    }

    private static TechniquesListPanel.Technique technique(String id, String name, String description, String input,
            float proficiency, List<TechniquesListPanel.RequiredMeridian> meridians, String icon) {
        return new TechniquesListPanel.Technique(id, name, List.of(), TechniquesListPanel.Grade.MORTAL,
            proficiency, "", true, "", description, "", meridians, 0, 0, 20, 3, 12, "attack", input, icon);
    }

    /** 仅显式 native 预览启用；覆盖标签恢复、经脉链接与带占位的替换交互。 */
    private void preparePracticeShot(Screen screen, UiPreviewShot shot) {
        var manager = UiWindowRuntime.manager();
        for (var window : manager.snapshot()) manager.close(window.key());
        UiWindowRuntime.openPractice();
        UiWindowRuntime.practice().query("");
        var catalog = state(PracticeWindows.CATALOG.windowType());
        manager.settleAt(catalog.key(), new UiWindowManager.Rect(12, 12, 350, shot.expectedLogicalHeight() - 52));
        render(screen);
        if (shot.name().equals("practice-catalog")) {
            assertCatalogContent(catalog, "首次打开修习目录");
            manager.close(catalog.key());
            UiWindowRuntime.openPractice();
            catalog = state(PracticeWindows.CATALOG.windowType());
            manager.settleAt(catalog.key(), new UiWindowManager.Rect(12, 12, 350, shot.expectedLogicalHeight() - 52));
            render(screen);
            assertCatalogContent(catalog, "关闭后重开修习目录");
            return;
        }
        if (shot.name().equals("practice-experience")) {
            UiWindowRuntime.searchPractice("#技艺");
            UiWindowRuntime.openPracticeDetail("skill:mineral");
            var detail = state(PracticeWindows.DETAIL.windowType());
            manager.settleAt(detail.key(), new UiWindowManager.Rect(372, 12, 290, shot.expectedLogicalHeight() - 52));
            render(screen);
            return;
        }
        if (shot.name().equals("practice-detail") || shot.name().equals("practice-meridian")) {
            UiWindowRuntime.openPracticeDetail("technique:" + DASH);
            var detail = state(PracticeWindows.DETAIL.windowType());
            manager.settleAt(detail.key(), new UiWindowManager.Rect(372, 12, 290, shot.expectedLogicalHeight() - 52));
            render(screen);
            manager.close(detail.key());
            UiWindowRuntime.openPracticeDetail("technique:" + DASH);
            detail = state(PracticeWindows.DETAIL.windowType());
            manager.settleAt(detail.key(), new UiWindowManager.Rect(372, 12, 290, shot.expectedLogicalHeight() - 52));
            render(screen);
            manager.minimize(catalog.key());
            click(screen, component(detail, "practice-tag-闪避"));
            render(screen);
            require(!catalog.minimized() && UiWindowRuntime.practice().query().equals("#闪避"), "标签未恢复目录并筛选");
            manager.focus(detail.key());
            render(screen);
            if (shot.name().equals("practice-meridian")) {
                click(screen, component(detail, "practice-meridian-KI"));
                render(screen);
                require(UiWindowRuntime.body(BodyInspectComponent.Layer.MERIDIAN).selectedChannel() == MeridianChannel.KI,
                    "经脉链接没有选中足少阴肾经");
                manager.minimize(catalog.key());
                manager.minimize(detail.key());
            }
            return;
        }
        UiWindowRuntime.openPracticeBinding(PREVIEW_DASH);
        var binding = state(PracticeWindows.BINDING.windowType());
        manager.settleAt(binding.key(), new UiWindowManager.Rect(372, 12, 290, shot.expectedLogicalHeight() - 52));
        render(screen);
        click(screen, component(binding, "bind-dash"));
        render(screen);
        require(requests.isEmpty(), "占用槽在确认前就发送了替换");
        clickVisible(screen, binding, "bind-cancel");
        render(screen);
        require(requests.isEmpty(), "取消替换仍发送了请求");
        scrollTop(screen, binding);
        click(screen, component(binding, "bind-dash"));
        render(screen);
        if (shot.name().equals("practice-compare")) {
            clickVisible(screen, binding, "bind-compare");
            render(screen);
            var compare = state(PracticeWindows.COMPARE.windowType());
            manager.settleAt(compare.key(), new UiWindowManager.Rect(132, 12, 490, shot.expectedLogicalHeight() - 52));
            require(requests.isEmpty(), "对比操作不应改变绑定");
        } else {
            clickVisible(screen, binding, "bind-confirm");
            render(screen);
            require(requests.size() == 1 && requests.get(0).get("expected_binding").getAsString().equals("skill:" + DASH),
                "替换未携带旧绑定身份");
            require(SkillBarStore.snapshot().dashSkillId().equals(DASH), "本地确认不应虚构权威闪避绑定");
            // 模拟权威拒绝后的变化，再次打开确认供接触表展示；不伪称网络回执。
            manager.close(binding.key());
            UiWindowRuntime.openPracticeBinding(PREVIEW_DASH);
            binding = state(PracticeWindows.BINDING.windowType());
            manager.settleAt(binding.key(), new UiWindowManager.Rect(372, 12, 290, shot.expectedLogicalHeight() - 52));
            render(screen);
            click(screen, component(binding, "bind-dash"));
            render(screen);
            clickScroll(screen, binding, -5);
        }
    }

    private static void clickVisible(Screen screen, UiWindowManager.WindowState state, String id) {
        var target = component(state, id);
        for (int i = 0; i < 12 && target.y() + target.height() > state.bounds().y() + state.bounds().height() - 12; i++) {
            clickScroll(screen, state, -2);
            target = component(state, id);
        }
        click(screen, target);
    }

    private static void assertCatalogContent(UiWindowManager.WindowState catalog, String phase) {
        var body = component(catalog, "practice-body");
        var results = component(catalog, "practice-results");
        require(body != null && body.width() > 0 && body.height() > 0,
            phase + "内容插槽没有可见尺寸");
        require(results != null && results.width() > 0 && results.height() > 0,
            phase + "目录结果组件没有可见尺寸");
    }

    private static void scrollTop(Screen screen, UiWindowManager.WindowState state) { clickScroll(screen, state, 100); }
    private static void clickScroll(Screen screen, UiWindowManager.WindowState state, double amount) {
        var scroll = component(state, "practice-scroll");
        screen.mouseScrolled(scroll.x() + 10, scroll.y() + 10, amount);
        render(screen);
    }
    private static Component component(UiWindowManager.WindowState state, String id) {
        return UiWindowRuntime.windowContentForPreview(state.key()).childById(Component.class, id);
    }
    private static void click(Screen screen, Component component) {
        require(component != null, "预览控件不存在");
        screen.mouseClicked(component.x() + 8, component.y() + 8, 0);
        screen.mouseReleased(component.x() + 8, component.y() + 8, 0);
    }
    private static void render(Screen screen) {
        var client = MinecraftClient.getInstance();
        var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        screen.render(context, -1, -1, 0);
        context.draw();
    }
    private static void require(boolean condition, String detail) {
        if (!condition) throw new IllegalStateException(detail);
    }
    @Override public void cleanup() {
        ClientRequestSender.resetBackendForTests();
        UiWindowRuntime.manager().reset();
        UiWindowRuntime.endPreview();
        if (savedTechniques != null) TechniquesListPanel.replace(savedTechniques);
        if (savedBar != null) SkillBarStore.replace(savedBar);
        if (savedExperience != null) SkillSetStore.replace(savedExperience);
        if (savedConfigs != null) SkillConfigStore.replace(savedConfigs);
    }
}
