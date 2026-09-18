package com.bong.client.ui.preview;

import com.bong.client.craft.CraftCategory;
import com.bong.client.menu.MainMenuScreen;
import com.bong.client.menu.MainMenuReasonWidget;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftContext;
import com.bong.client.craft.CraftWindows;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.ui.window.UiWindowRuntime;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.craft.CraftStore;
import com.bong.client.combat.screen.ForgeCarrierScreen;
import com.bong.client.combat.screen.TerminateScreen;
import com.bong.client.combat.screen.RepairScreen;
import com.bong.client.combat.screen.DeathScreen;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.combat.store.TerminationSummary;
import com.bong.client.death.DeathCinematicState;
import com.bong.client.coffin.CoffinMenuScreen;
import com.bong.client.combat.screen.ZhenfaLayoutScreen;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelScreen;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.identity.IdentityPanelUiStateSource;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost.ComponentBounds;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.contract.UiViewport;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.DisconnectedScreen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.ClickableWidget;
import net.minecraft.text.Text;

import java.util.List;
import java.util.Map;

/** UI 截图场景白名单。新增场景必须显式登记并提供确定性 fixture。 */
final class UiPreviewScenes {
    private static final Map<String, UiPreviewScene> SCENES = Map.ofEntries(
        Map.entry("model-windows", new UiModelWindowPreviewScene()),
        Map.entry("body-model", new UiBodyModelPreviewScene()),
        Map.entry("hud-windows", new UiHudWindowPreviewScene()),
        Map.entry("technique-windows", new UiTechniqueWindowPreviewScene()),
        Map.entry("inventory-windows", new UiInventoryWindowPreviewScene()),
        Map.entry("item-windows", new UiWindowPreviewScene()),
        Map.entry("item-windows-armor", new UiWindowPreviewScene("armor_iron_leggings")),
        Map.entry("item-windows-pack", new UiWindowPreviewScene("grass_pouch")),
        Map.entry("item-windows-block", new UiWindowPreviewScene("weathered_stone")),
        Map.entry("item-windows-settings", new UiWindowPreviewScene("pickaxe_iron", "settings")),
        Map.entry("item-windows-terrain", new UiWindowPreviewScene("pickaxe_iron", "terrain")),
        Map.entry("item-windows-hud", new UiWindowPreviewScene("pickaxe_iron", "hud")),
        Map.entry("craft", new CraftScene()),
        Map.entry("forge-window", new UiForgeWindowPreviewScene()),
        Map.entry("terminate", new TerminateScene(0)),
        Map.entry("terminate-kind", new TerminateScene(1)),
        Map.entry("terminate-hungry", new TerminateScene(2)),
        Map.entry("terminate-mocking", new TerminateScene(3)),
        Map.entry("coffin-menu", new CoffinMenuScene()),
        Map.entry("repair", new RepairScene()),
        Map.entry("forge-carrier", new ForgeCarrierScene()),
        Map.entry("death", new DeathScene(-1, true)),
        Map.entry("death-dice-close", new DeathScene(0, true)),
        Map.entry("death-dice-roll", new DeathScene(23, true)),
        Map.entry("death-dice-land", new DeathScene(42, true)),
        Map.entry("death-dice-big", new DeathScene(60, true)),
        Map.entry("death-dice-small", new DeathScene(60, false)),
        Map.entry("death-dice-throw-big", new DeathScene(0, true, true)),
        Map.entry("death-dice-throw-small", new DeathScene(0, false, true)),
        Map.entry("identity-panel", new IdentityPanelScene(false)),
        Map.entry("identity-panel-empty", new IdentityPanelScene(true)),
        Map.entry("zhenfa-layout", new ZhenfaLayoutScene()),
        Map.entry("main-menu", new MainMenuScene())
    );

    private UiPreviewScenes() {
    }

    private static void verifyEmptyLabelInput(OwoXmlScreenHost<?> screen, String... ids) {
        var client = MinecraftClient.getInstance();
        var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        for (String id : ids) {
            var bounds = screen.componentBoundsForPreview(id);
            int mouseX = (int) bounds.centerX();
            int mouseY = (int) bounds.centerY();
            try {
                // 复现空反馈栏悬停崩溃：实际渲染 tooltip，并走同一个文本点击入口。
                screen.render(context, mouseX, mouseY, 0);
                context.draw();
                screen.mouseClicked(mouseX, mouseY, 0);
                screen.mouseReleased(mouseX, mouseY, 0);
            } catch (RuntimeException failure) {
                throw new IllegalStateException("空文本区域悬停或点击失败: " + id, failure);
            }
        }
    }

    private static final class MainMenuScene implements UiPreviewScene {
        @Override public void installFixture(UiPreviewConfig config) {}
        @Override public Screen createScreen() { return new MainMenuScreen(); }
        @Override public String selectedTemplateId(Screen screen) {
            return ((MainMenuScreen) screen).selectedTemplateIdForTests();
        }
        @Override public boolean isReady(Screen screen) {
            return ((MainMenuScreen) screen).hostReadyForTests();
        }
        @Override public boolean initializationFailed(Screen screen) {
            return ((MainMenuScreen) screen).hostInitializationFailedForTests();
        }
        @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
            MainMenuScreen menu = (MainMenuScreen) screen;
            for (String id : List.of("menu-brand", "menu-enter", "menu-settings", "menu-exit", "menu-status-slot")) {
                ComponentBounds bounds = menu.componentBoundsForPreview(id);
                if (!bounds.fitsInside(shot.expectedLogicalWidth(), shot.expectedLogicalHeight())) {
                    throw new IllegalStateException("主菜单元素超出窗口: " + id + " " + bounds);
                }
                if (id.equals("menu-enter") || id.equals("menu-settings") || id.equals("menu-exit")) {
                    if (!id.equals(menu.componentIdAtForPreview(bounds.centerX(), bounds.centerY()))) {
                        throw new IllegalStateException("菜单点击区没有命中对应命令: " + id);
                    }
                }
            }
            ComponentBounds status = menu.componentBoundsForPreview("menu-status");
            if (status.isPositive() && !menu.componentBoundsForPreview("menu-status-slot").contains(status)) {
                throw new IllegalStateException("主菜单提示文字超出预留区域: " + status);
            }
            if (!menu.focusOrderForPreview().equals(List.of("menu-enter", "menu-settings", "menu-exit"))) {
                throw new IllegalStateException("主菜单键盘顺序必须为入世、设置、退出");
            }
            validateDisconnectedResize(menu);
        }

        private static void validateDisconnectedResize(MainMenuScreen parent) {
            Screen disconnected = new DisconnectedScreen(parent, Text.empty(),
                Text.literal("连接已断开，请检查客户端资源版本。".repeat(80)));
            disconnected.init(MinecraftClient.getInstance(), 640, 360);
            // 原版断线页 resize 只调用 initTabNavigation，不能用重新 init 代替这一回归。
            for (int[] viewport : new int[][] {{320, 240}, {640, 360}}) {
                disconnected.resize(MinecraftClient.getInstance(), viewport[0], viewport[1]);
                MainMenuReasonWidget reason = disconnected.children().stream()
                    .filter(MainMenuReasonWidget.class::isInstance).map(MainMenuReasonWidget.class::cast)
                    .findFirst().orElseThrow(() -> new IllegalStateException("断线说明缺少滚动控件"));
                if (Math.abs(reason.getX() * 2 + reason.getWidth() - viewport[0]) > 1) {
                    throw new IllegalStateException("断线说明未随窗口缩放重新居中");
                }
                for (var child : disconnected.children()) {
                    if (!(child instanceof ClickableWidget widget) || !widget.visible) continue;
                    ComponentBounds bounds = new ComponentBounds(widget.getX(), widget.getY(), widget.getWidth(), widget.getHeight());
                    if (!bounds.fitsInside(viewport[0], viewport[1])) {
                        throw new IllegalStateException("断线控件在缩放后超出窗口: " + bounds);
                    }
                    if (widget instanceof ButtonWidget && widget.getY() < reason.getY() + reason.getHeight()) {
                        throw new IllegalStateException("断线按钮在缩放后覆盖了说明正文");
                    }
                }
            }
        }
        @Override public void cleanup() {}
    }

    static UiPreviewScene require(String sceneId) {
        UiPreviewScene scene = SCENES.get(sceneId);
        if (scene == null) {
            throw new IllegalArgumentException("未登记的 UI preview scene: " + sceneId);
        }
        return scene;
    }

    static boolean isRegistered(String sceneId) {
        return SCENES.containsKey(sceneId);
    }

    private static final class CraftScene implements UiPreviewScene {
        // 仅用于界面排版的玩家快照，不代表 qi_physics 全局灵气总量。
        private static final double PREVIEW_PLAYER_QI_CURRENT = 63.0;
        private static final double PREVIEW_PLAYER_QI_MAX = 96.0;

        @Override
        public void installFixture(UiPreviewConfig config) {
            UiWindowRuntime.beginPreview();
            UiWindowRuntime.manager().reset();
            CraftStore.clear();
            CraftStore.replaceRecipes(List.of(
                new CraftRecipe("rough_knife", CraftCategory.TOOL, "粗铁短刀", List.of(
                    new CraftRecipe.MaterialEntry("rust_iron", 3),
                    new CraftRecipe.MaterialEntry("withered_herb", 2),
                    new CraftRecipe.MaterialEntry("bitter_root", 1)
                ), 5.0, 80L, "rough_knife", 1, CraftRecipe.Requirements.NONE, true),
                recipe("herb_wrap", CraftCategory.MISC, "枯草裹伤布", "withered_herb", 2, true),
                recipe("sealed_powder", CraftCategory.POISON_POWDER, "未辨毒粉", "bitter_root", 4, false)
            ));
            InventoryStateStore.replace(InventoryModel.builder()
                .gridItem(item(1001L, "rust_iron", "锈铁片", 12), 0, 0)
                .gridItem(item(1002L, "withered_herb", "枯草", 8), 0, 1)
                .gridItem(item(1003L, "bitter_root", "苦根", 1), 0, 2)
                .cultivation("醒灵", PREVIEW_PLAYER_QI_CURRENT, PREVIEW_PLAYER_QI_MAX, 1.0)
                .build());
            SkillSetStore.replace(SkillSetSnapshot.empty());
        }

        @Override
        public Screen createScreen() {
            return new InspectScreen(InventoryStateStore.snapshot());
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            return "craft-window";
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof InspectScreen inspect && inspect.windowHostReadyForPreview();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof InspectScreen inspect && inspect.windowHostFailedForPreview();
        }

        @Override
        public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
            var manager = UiWindowRuntime.manager();
            for (var state : manager.snapshot()) manager.close(state.key());
            UiWindowRuntime.openCraft(CraftContext.HANDCRAFT);
            var state = manager.snapshot().stream().filter(window -> window.definition().equals(CraftWindows.DEFINITION))
                .findFirst().orElseThrow();
            manager.settleAt(state.key(), new UiWindowManager.Rect(4, 4,
                shot.expectedLogicalWidth() - 8, shot.expectedLogicalHeight() - 36));
            renderCraft(screen);
            var target = state.bounds();
            if (shot.name().equals("craft-wide")) {
                manager.resize(state.key(), "300", "200");
                renderCraft(screen);
                verifyDetailScroll(screen, state);
                manager.settleAt(state.key(), target);
                renderCraft(screen);
            } else {
                verifyDetailScroll(screen, state);
            }
            manager.minimize(state.key());
            renderCraft(screen);
            UiWindowRuntime.openCraft(CraftContext.HANDCRAFT);
            renderCraft(screen);
            if (state.minimized() || state.scope().isClosed() || !manager.snapshot().contains(state)) {
                throw new IllegalStateException("最小化恢复必须保留制作窗口及其订阅");
            }
        }

        private static void verifyDetailScroll(Screen screen, UiWindowManager.WindowState state) {
            var scroll = UiWindowRuntime.windowContentForPreview(state.key()).childById(
                io.wispforest.owo.ui.container.ScrollContainer.class, "craft-detail-scroll");
            int before = scroll.child().y();
            screen.mouseScrolled(scroll.x() + 10, scroll.y() + 10, -8);
            renderCraft(screen);
            if (scroll.child().height() > scroll.height() && scroll.child().y() >= before) {
                throw new IllegalStateException("制作详情内容溢出时，滚轮没有移动材料和产物");
            }
            screen.mouseScrolled(scroll.x() + 10, scroll.y() + 10, 100);
            renderCraft(screen);
        }

        private static void renderCraft(Screen screen) {
            var client = MinecraftClient.getInstance();
            var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
            screen.render(context, -1, -1, 0);
            context.draw();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            var state = UiWindowRuntime.manager().snapshot().stream()
                .filter(window -> window.definition().equals(CraftWindows.DEFINITION)).findFirst().orElseThrow();
            var content = UiWindowRuntime.windowContentForPreview(state.key());
            for (String id : List.of("craft-search", "craft-return", "craft-minus", "craft-plus", "craft-start")) {
                var component = content.childById(io.wispforest.owo.ui.core.Component.class, id);
                var bounds = new ComponentBounds(component.x(), component.y(), component.width(), component.height());
                requireInViewport(id, bounds, shot.expectedLogicalWidth(), shot.expectedLogicalHeight());
                var window = state.bounds();
                if (bounds.x() < window.x() || bounds.y() < window.y()
                    || bounds.x() + bounds.width() > window.x() + window.width()
                    || bounds.y() + bounds.height() > window.y() + window.height()) {
                    throw new IllegalStateException("制作操作超出窗口: " + id);
                }
            }
        }

        @Override
        public void cleanup() {
            UiWindowRuntime.manager().reset();
            UiWindowRuntime.endPreview();
            SessionScopedStoreRegistry.clearAllOnDisconnect();
        }

        private static CraftRecipe recipe(
            String id,
            CraftCategory category,
            String name,
            String material,
            int count,
            boolean unlocked
        ) {
            return new CraftRecipe(
                id, category, name,
                List.of(new CraftRecipe.MaterialEntry(material, count)),
                5.0, 80L, id, 1, CraftRecipe.Requirements.NONE, unlocked
            );
        }

        private static InventoryItem item(long instanceId, String id, String name, int count) {
            return InventoryItem.createFull(
                instanceId, id, name, 1, 1, 0.2, "common", "UI preview fixture",
                count, 1.0, 1.0
            );
        }

        private static void requireInViewport(
            String id,
            ComponentBounds bounds,
            int logicalWidth,
            int logicalHeight
        ) {
            if (!bounds.fitsInside(logicalWidth, logicalHeight)) {
                throw new IllegalStateException(
                    "关键组件越界: " + id + " -> " + bounds
                        + ", viewport=" + logicalWidth + "x" + logicalHeight);
            }
        }
    }

    private static final class TerminateScene implements UiPreviewScene {
        private final int voice;

        private TerminateScene(int voice) { this.voice = voice; }

        @Override
        public void installFixture(UiPreviewConfig config) {
            var summary = new TerminationSummary("行客", "Condense", 4, 47.5, 88.0, 72.0, 3, 7);
            // 改动无展示用途的旧建议字段，确定性覆盖四种终局口吻。
            TerminateStateStore.State fixture;
            int seed = 0;
            do {
                fixture = new TerminateStateStore.State(
                true,
                "此身已尽，遗言仍在。\n愿后来者少走一段旧路。",
                "尘土收拢，旧名不再回应。",
                "preview-" + seed++, summary
                );
            } while (Math.floorMod(fixture.hashCode(), 4) != voice);
            TerminateStateStore.replace(fixture);
        }

        @Override
        public Screen createScreen() {
            return new TerminateScreen(TerminateStateStore.snapshot());
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof TerminateScreen terminate)) {
                throw new IllegalStateException("terminate scene 打开的不是 TerminateScreen");
            }
            return terminate.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof TerminateScreen terminate && terminate.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof TerminateScreen terminate && terminate.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof TerminateScreen terminate)) {
                throw new IllegalStateException("terminate scene 打开的不是 TerminateScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = terminate.componentBoundsForPreview("terminate-panel");
            requireInViewport("terminate-panel", panel, width, height);
            for (String id : new String[] {
                "terminate-title", "terminate-epilogue", "terminate-content-scroll",
                "terminate-create-character"
            }) {
                ComponentBounds bounds = terminate.componentBoundsForPreview(id);
                requireInViewport(id, bounds, width, height);
                if (!panel.contains(bounds)) {
                    throw new IllegalStateException("终结屏组件越出 panel: " + id + " -> " + bounds);
                }
            }
            ComponentBounds button = terminate.componentBoundsForPreview("terminate-create-character");
            String hit = terminate.componentIdAtForPreview(button.centerX(), button.centerY());
            if (!"terminate-create-character".equals(hit)) {
                throw new IllegalStateException("终结屏按钮中心命中错误: expected=terminate-create-character, actual=" + hit);
            }
            if (!terminate.focusOrderForPreview().contains("terminate-create-character")) {
                throw new IllegalStateException("终结屏创建按钮没有进入 Tab 焦点顺序");
            }
            verifyEmptyLabelInput(terminate, "terminate-feedback", "terminate-title");
        }

        @Override
        public void cleanup() {
            SessionScopedStoreRegistry.clearAllOnDisconnect();
        }

        private static void requireInViewport(String id, ComponentBounds bounds, int width, int height) {
            if (!bounds.fitsInside(width, height)) {
                throw new IllegalStateException(
                    "终结屏关键组件越界: " + id + " -> " + bounds + ", viewport=" + width + "x" + height);
            }
        }
    }

    private static final class CoffinMenuScene implements UiPreviewScene {
        @Override
        public void installFixture(UiPreviewConfig config) {
        }

        @Override
        public Screen createScreen() {
            return new CoffinMenuScreen(new net.minecraft.util.math.BlockPos(4, 65, 7));
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof CoffinMenuScreen coffin)) {
                throw new IllegalStateException("coffin scene 打开的不是 CoffinMenuScreen");
            }
            return coffin.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof CoffinMenuScreen coffin && coffin.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof CoffinMenuScreen coffin && coffin.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof CoffinMenuScreen coffin)) {
                throw new IllegalStateException("coffin scene 打开的不是 CoffinMenuScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = coffin.componentBoundsForPreview("coffin-panel");
            if (!panel.fitsInside(width, height)) {
                throw new IllegalStateException("延寿棺面板越出 viewport: " + panel + ", viewport=" + width + "x" + height);
            }
            for (String id : new String[] {"coffin-title", "coffin-enter", "coffin-reclaim"}) {
                ComponentBounds bounds = coffin.componentBoundsForPreview(id);
                if (!panel.contains(bounds) || !bounds.fitsInside(width, height)) {
                    throw new IllegalStateException("延寿棺组件越界: " + id + " -> " + bounds);
                }
                String hit = coffin.componentIdAtForPreview(bounds.centerX(), bounds.centerY());
                if (!id.equals(hit)) {
                    throw new IllegalStateException("延寿棺组件中心命中错误: expected=" + id + ", actual=" + hit);
                }
            }
            if (!coffin.focusOrderForPreview().contains("coffin-enter")
                || !coffin.focusOrderForPreview().contains("coffin-reclaim")) {
                throw new IllegalStateException("延寿棺按钮没有进入 Tab 焦点顺序");
            }
        }

        @Override
        public void cleanup() {
        }
    }

    private static final class RepairScene implements UiPreviewScene {
        @Override
        public void installFixture(UiPreviewConfig config) {
        }

        @Override
        public Screen createScreen() {
            // 预览不应触碰真实网络；组合根注入一个确定性本地 sink。
            return new RepairScreen(
                "锈骨剑", 0.42f, 4242L, 1, 64, 2, intent ->
                    com.bong.client.ui.intent.UiIntentResult.accepted());
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof RepairScreen repair)) {
                throw new IllegalStateException("repair scene 打开的不是 RepairScreen");
            }
            return repair.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof RepairScreen repair && repair.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof RepairScreen repair && repair.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof RepairScreen repair)) {
                throw new IllegalStateException("repair scene 打开的不是 RepairScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = repair.componentBoundsForPreview("repair-panel");
            if (!panel.fitsInside(width, height)) {
                throw new IllegalStateException("养护面板越出 viewport: " + panel + ", viewport=" + width + "x" + height);
            }
            for (String id : new String[] {
                "repair-title", "repair-durability", "repair-durability-track", "repair-actions",
                "repair-steel", "repair-pill"
            }) {
                ComponentBounds bounds = repair.componentBoundsForPreview(id);
                if (!panel.contains(bounds) || !bounds.fitsInside(width, height)) {
                    throw new IllegalStateException("养护组件越界: " + id + " -> " + bounds);
                }
            }
            ComponentBounds fill = repair.componentBoundsForPreview("repair-durability-fill");
            if (fill.width() != Math.round(repair.durabilityNormForTests() * repair.durabilityBarWidthForTests())
                || fill.height() <= 0) {
                throw new IllegalStateException("养护耐久条没有反映快照: " + fill);
            }
            for (String id : new String[] {"repair-steel", "repair-pill"}) {
                ComponentBounds bounds = repair.componentBoundsForPreview(id);
                String hit = repair.componentIdAtForPreview(bounds.centerX(), bounds.centerY());
                if (!id.equals(hit)) {
                    throw new IllegalStateException("养护按钮中心命中错误: expected=" + id + ", actual=" + hit);
                }
            }
            if (!repair.focusOrderForPreview().containsAll(List.of("repair-steel", "repair-pill"))) {
                throw new IllegalStateException("养护按钮没有进入 Tab 焦点顺序");
            }
        }

        @Override
        public void cleanup() {
        }
    }

    private static final class ForgeCarrierScene implements UiPreviewScene {
        @Override
        public void installFixture(UiPreviewConfig config) {
        }

        @Override
        public Screen createScreen() {
            // 预览只验证 XML 布局与输入绑定，不触碰真实网络 transport。
            return new ForgeCarrierScreen("needle", 0.65,
                intent -> com.bong.client.ui.intent.UiIntentResult.accepted());
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof ForgeCarrierScreen forgeCarrier)) {
                throw new IllegalStateException("forge-carrier scene 打开的不是 ForgeCarrierScreen");
            }
            return forgeCarrier.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof ForgeCarrierScreen forgeCarrier && forgeCarrier.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof ForgeCarrierScreen forgeCarrier
                && forgeCarrier.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof ForgeCarrierScreen forgeCarrier)) {
                throw new IllegalStateException("forge-carrier scene 打开的不是 ForgeCarrierScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = forgeCarrier.componentBoundsForPreview("forge-carrier-panel");
            requireInViewport("forge-carrier-panel", panel, width, height);
            for (String id : new String[] {
                "forge-carrier-title", "forge-carrier-selection", "forge-carrier-item-actions",
                "forge-carrier-qi-label", "forge-carrier-qi-slider", "forge-carrier-submit"
            }) {
                ComponentBounds bounds = forgeCarrier.componentBoundsForPreview(id);
                if (!panel.contains(bounds) || !bounds.fitsInside(width, height)) {
                    throw new IllegalStateException("暗器注入组件越界: " + id + " -> " + bounds);
                }
            }
            for (String id : new String[] {
                "forge-carrier-dagger", "forge-carrier-needle", "forge-carrier-qi-slider",
                "forge-carrier-submit"
            }) {
                ComponentBounds bounds = forgeCarrier.componentBoundsForPreview(id);
                String hit = forgeCarrier.componentIdAtForPreview(bounds.centerX(), bounds.centerY());
                if (!id.equals(hit)) {
                    throw new IllegalStateException(
                        "暗器注入控件中心命中错误: expected=" + id + ", actual=" + hit);
                }
            }
            if (!forgeCarrier.focusOrderForPreview().containsAll(List.of(
                "forge-carrier-dagger", "forge-carrier-needle", "forge-carrier-qi-slider",
                "forge-carrier-submit"))) {
                throw new IllegalStateException("暗器注入控件没有进入 Tab 焦点顺序");
            }
        }

        private static void requireInViewport(String id, ComponentBounds bounds, int width, int height) {
            if (!bounds.fitsInside(width, height)) {
                throw new IllegalStateException(
                    "暗器注入关键组件越界: " + id + " -> " + bounds + ", viewport=" + width + "x" + height);
            }
        }

        @Override
        public void cleanup() {
        }
    }

    private static final class ZhenfaLayoutScene implements UiPreviewScene {
        @Override
        public void installFixture(UiPreviewConfig config) {
        }

        @Override
        public Screen createScreen() {
            // 预览只验证 XML 布局与输入绑定，不触碰真实网络 transport。
            return ZhenfaLayoutScreen.preview();
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof ZhenfaLayoutScreen zhenfa)) {
                throw new IllegalStateException("zhenfa scene 打开的不是 ZhenfaLayoutScreen");
            }
            return zhenfa.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof ZhenfaLayoutScreen zhenfa && zhenfa.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof ZhenfaLayoutScreen zhenfa && zhenfa.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof ZhenfaLayoutScreen zhenfa)) {
                throw new IllegalStateException("zhenfa scene 打开的不是 ZhenfaLayoutScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = zhenfa.componentBoundsForPreview("zhenfa-panel");
            if (!panel.fitsInside(width, height)) {
                throw new IllegalStateException("阵法面板越出 viewport: " + panel);
            }
            for (String id : new String[] {
                "zhenfa-title", "zhenfa-summary", "zhenfa-trigger-actions", "zhenfa-qi-slider",
                "zhenfa-feedback", "zhenfa-place"
            }) {
                ComponentBounds bounds = zhenfa.componentBoundsForPreview(id);
                if (!panel.contains(bounds) || !bounds.fitsInside(width, height)) {
                    throw new IllegalStateException("阵法组件越界: " + id + " -> " + bounds);
                }
            }
            for (String id : new String[] {
                "zhenfa-trigger-proximity", "zhenfa-trigger-contact", "zhenfa-trigger-timed",
                "zhenfa-qi-slider", "zhenfa-place"
            }) {
                ComponentBounds bounds = zhenfa.componentBoundsForPreview(id);
                String hit = zhenfa.componentIdAtForPreview(bounds.centerX(), bounds.centerY());
                if (!id.equals(hit)) {
                    throw new IllegalStateException("阵法控件中心命中错误: expected=" + id + ", actual=" + hit);
                }
            }
            if (!zhenfa.focusOrderForPreview().containsAll(List.of(
                "zhenfa-trigger-proximity", "zhenfa-trigger-contact", "zhenfa-trigger-timed",
                "zhenfa-qi-slider", "zhenfa-place"))) {
                throw new IllegalStateException("阵法控件没有进入 Tab 焦点顺序");
            }
        }

        @Override
        public void cleanup() {
        }
    }

    private static final class DeathScene implements UiPreviewScene {
        private final int rollTick;
        private final boolean big;
        private final boolean realtime;
        private com.bong.client.combat.DeathIntent dispatched;

        private DeathScene(int rollTick, boolean big) {
            this(rollTick, big, false);
        }

        private DeathScene(int rollTick, boolean big, boolean realtime) {
            this.rollTick = rollTick;
            this.big = big;
            this.realtime = realtime;
        }

        @Override
        public void installFixture(UiPreviewConfig config) {
            dispatched = null;
            DeathStateStore.replace(new DeathStateStore.State(
                true,
                "pk",
                0.42f,
                List.of("不甘心", "旧名未曾留下", "愿后来者少走一段旧路"),
                System.currentTimeMillis() + 30_000,
                rollTick < 0,
                rollTick < 0,
                "tribulation",
                2,
                "ordinary",
                0.0,
                0,
                0.0,
                0,
                0.0,
                false,
                rollTick < 0 ? DeathCinematicState.INACTIVE : new DeathCinematicState(
                    true, "preview", DeathCinematicState.Phase.ROLL, rollTick, 64, rollTick, 64,
                    new DeathCinematicState.Roll(0.42, 0.42, 0, big ? DeathCinematicState.RollResult.SURVIVE : DeathCinematicState.RollResult.FALL),
                    List.of(), false, 2, "ordinary", false, 0, realtime ? System.currentTimeMillis() + 4_000 : Long.MAX_VALUE)
            ));
        }

        @Override
        public Screen createScreen() {
            return new DeathScreen(
                DeathStateStore.snapshot(),
                intent -> {
                    dispatched = intent;
                    return com.bong.client.ui.intent.UiIntentResult.accepted();
                }
            );
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof DeathScreen death)) {
                throw new IllegalStateException("death scene 打开的不是 DeathScreen");
            }
            return death.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof DeathScreen death && death.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof DeathScreen death && death.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof DeathScreen death)) {
                throw new IllegalStateException("death scene 打开的不是 DeathScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = death.componentBoundsForPreview("death-panel");
            if (!panel.fitsInside(width, height)) {
                throw new IllegalStateException("死亡屏面板越出 viewport: " + panel + ", viewport=" + width + "x" + height);
            }
            for (String id : new String[] {
                "death-title", "death-dice", "death-content-scroll", "death-actions",
                "death-reincarnate", "death-terminate"
            }) {
                ComponentBounds bounds = death.componentBoundsForPreview(id);
                if (!panel.contains(bounds) || !bounds.fitsInside(width, height)) {
                    throw new IllegalStateException("死亡屏组件越界: " + id + " -> " + bounds);
                }
            }
            ComponentBounds fill = death.componentBoundsForPreview("death-luck-fill");
            if (fill.width() != Math.round(death.stateForTests().luckRemaining() * 220f)
                || fill.height() <= 0) {
                throw new IllegalStateException("死亡屏概率条没有反映快照: " + fill);
            }
            for (String id : List.of("death-reincarnate", "death-terminate")) {
                ComponentBounds bounds = death.componentBoundsForPreview(id);
                String hit = death.componentIdAtForPreview(bounds.centerX(), bounds.centerY());
                if (!id.equals(hit)) {
                    throw new IllegalStateException("死亡屏按钮中心命中错误: expected=" + id + ", actual=" + hit);
                }
            }
            if (rollTick < 0 && !death.focusOrderForPreview().containsAll(List.of("death-reincarnate", "death-terminate"))) {
                throw new IllegalStateException("死亡屏按钮没有进入 Tab 焦点顺序");
            }
            verifyEmptyLabelInput(death, "death-feedback", "death-title");
            if (rollTick < 0) {
                var button = death.componentBoundsForPreview("death-terminate");
                death.mouseClicked(button.centerX(), button.centerY(), 0);
                if (!(dispatched instanceof com.bong.client.combat.DeathIntent.Terminate)) {
                    throw new IllegalStateException("真实鼠标点击未发送终结意图");
                }
            }
        }

        @Override
        public void cleanup() {
            SessionScopedStoreRegistry.clearAllOnDisconnect();
        }
    }

    private static final class IdentityPanelScene implements UiPreviewScene {
        private final boolean empty;

        private IdentityPanelScene(boolean empty) {
            this.empty = empty;
        }

        @Override
        public void installFixture(UiPreviewConfig config) {
            if (empty) {
                IdentityPanelStateStore.replace(IdentityPanelState.empty());
                return;
            }
            IdentityPanelStateStore.replace(new IdentityPanelState(
                2,
                480L,
                0L,
                List.of(
                    entry(0, "旧名", -80, true),
                    entry(1, "行路人", 12, false),
                    entry(2, "当前身份", 75, false),
                    entry(3, "药铺掌柜", 4, false),
                    entry(4, "锈骨匠", -3, true),
                    entry(5, "渡口客", 18, false),
                    entry(6, "未登记名", 0, false)
                )
            ));
        }

        @Override
        public Screen createScreen() {
            // 预览只验证 XML 布局和状态绑定，输入走本地 sink，不发送网络命令。
            return new IdentityPanelScreen(
                IdentityPanelUiStateSource.production(),
                ignored -> com.bong.client.ui.intent.UiIntentResult.accepted()
            );
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof IdentityPanelScreen identity)) {
                throw new IllegalStateException("identity scene 打开的不是 IdentityPanelScreen");
            }
            return identity.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof IdentityPanelScreen identity && identity.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof IdentityPanelScreen identity
                && identity.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof IdentityPanelScreen identity)) {
                throw new IllegalStateException("identity scene 打开的不是 IdentityPanelScreen");
            }
            int width = shot.expectedLogicalWidth();
            int height = shot.expectedLogicalHeight();
            ComponentBounds panel = identity.componentBoundsForPreview("identity-panel");
            if (!panel.fitsInside(width, height)) {
                throw new IllegalStateException(
                    "身份面板越出 viewport: " + panel + ", viewport=" + width + "x" + height);
            }
            for (String id : new String[] {
                "identity-title", "identity-cooldown", "identity-name", "identity-name-actions",
                "identity-empty", "identity-rows", "identity-overflow"
            }) {
                ComponentBounds bounds = identity.componentBoundsForPreview(id);
                if (!panel.contains(bounds) || !bounds.fitsInside(width, height)) {
                    throw new IllegalStateException("身份面板组件越界: " + id + " -> " + bounds);
                }
            }
            List<String> hitTargets = new java.util.ArrayList<>(List.of("identity-new", "identity-rename"));
            if (!empty) {
                hitTargets.add("identity-switch-0");
                hitTargets.add("identity-switch-5");
            }
            for (String id : hitTargets) {
                ComponentBounds bounds = identity.componentBoundsForPreview(id);
                String hit = identity.componentIdAtForPreview(bounds.centerX(), bounds.centerY());
                if (!id.equals(hit)) {
                    throw new IllegalStateException(
                        "身份面板控件中心命中错误: expected=" + id + ", actual=" + hit);
                }
            }
            List<String> focus = identity.focusOrderForPreview();
            for (String id : List.of("identity-name", "identity-new", "identity-rename")) {
                if (!focus.contains(id)) {
                    throw new IllegalStateException("身份面板缺少 Tab 焦点组件: " + id + " -> " + focus);
                }
            }
            if (!empty && !focus.contains("identity-switch-0")) {
                throw new IllegalStateException("身份面板可用身份按钮没有进入 Tab 焦点顺序: " + focus);
            }
        }

        @Override
        public void cleanup() {
            SessionScopedStoreRegistry.clearAllOnDisconnect();
        }

        private static IdentityPanelEntry entry(int id, String name, int reputation, boolean frozen) {
            return new IdentityPanelEntry(id, name, reputation, frozen, List.of());
        }
    }
}
