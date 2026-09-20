package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.WindowLayoutPreferenceStore;
import com.bong.client.hud.HudWidgetWindows;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.CheckboxComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Positioning;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Util;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.function.Consumer;
import java.util.function.BiConsumer;
import java.util.EnumMap;

/** 工作台恢复条和本地背景设置；不持有任何业务 scope。 */
public final class WorkspaceControls {
    private final UiWindowManager manager;
    private final WorkspaceBackgrounds backgrounds;
    private final WindowLayoutPreferenceStore preferences;
    private final Runnable save;
    private final HudWidgetWindows hud;
    private final Consumer<HudWidgetWindows.Widget> openHud;
    private final BiConsumer<HudWidgetWindows.Widget, Boolean> showHud;
    private final Runnable resetLayout;
    private final Runnable openModelPreview;
    private final java.util.function.BooleanSupplier canPreviewModels;
    private final Consumer<UiWindowManager.WindowState> remember;
    private OwoUIAdapter<FlowLayout> adapter;
    private FlowLayout bar;
    private FlowLayout settings;
    private FlowLayout hudList;
    private ButtonComponent modelButton;
    private final Map<HudWidgetWindows.Widget, CheckboxComponent> hudChecks = new EnumMap<>(HudWidgetWindows.Widget.class);
    private final Map<UiWindowManager.WindowKey, ButtonComponent> restoreButtons = new LinkedHashMap<>();
    private int width;
    private int height;
    private boolean settingsOpen;
    private boolean hudOpen;
    private boolean updatingChecks;

    public WorkspaceControls(UiWindowManager manager, WorkspaceBackgrounds backgrounds,
                             WindowLayoutPreferenceStore preferences, Runnable save, HudWidgetWindows hud,
                             Consumer<HudWidgetWindows.Widget> openHud,
                             BiConsumer<HudWidgetWindows.Widget, Boolean> showHud, Runnable resetLayout,
                             Consumer<UiWindowManager.WindowState> remember, Runnable openModelPreview,
                             java.util.function.BooleanSupplier canPreviewModels) {
        this.manager = manager;
        this.backgrounds = backgrounds;
        this.preferences = preferences;
        this.save = save;
        this.hud = hud;
        this.openHud = openHud;
        this.showHud = showHud;
        this.resetLayout = resetLayout;
        this.remember = remember;
        this.openModelPreview = openModelPreview;
        this.canPreviewModels = canPreviewModels;
    }

    public void layout(int w, int h, Function<UiWindowManager.WindowKey, String> titles) {
        if (adapter == null) {
            var template = OwoXmlTemplateRegistry.production().require("workspace-controls");
            adapter = template.createAdapterWithoutScreen(0, 0, w, h, FlowLayout.class);
            bar = template.expandTemplate(FlowLayout.class, "bar", Map.of());
            settings = template.expandTemplate(FlowLayout.class, "settings", Map.of());
            hudList = template.expandTemplate(FlowLayout.class, "hud-list", Map.of());
            adapter.rootComponent.child(bar);
            bar.childById(ButtonComponent.class, "workspace-settings").onPress(button -> toggleSettings());
            bar.childById(ButtonComponent.class, "workspace-hud")
                .renderer((context, button, delta) -> OwoXmlWindowContentAdapter.renderControlIcon(context, button,
                    new net.minecraft.util.Identifier("bong-client", "textures/gui/window/panels-top-left.png"), 0xFF485A51))
                .onPress(button -> toggleHud());
            bar.childById(ButtonComponent.class, "workspace-reset")
                .renderer((context, button, delta) -> OwoXmlWindowContentAdapter.renderControlIcon(context, button,
                    new net.minecraft.util.Identifier("bong-client", "textures/gui/window/rotate-ccw.png"), 0xFF485A51))
                .onPress(button -> resetLayout.run());
            modelButton = bar.childById(ButtonComponent.class, "workspace-model");
            modelButton.onPress(button -> { if (canPreviewModels.getAsBoolean()) openModelPreview.run(); });
            var hudRows = hudList.childById(FlowLayout.class, "workspace-hud-rows");
            for (var widget : HudWidgetWindows.Widget.values()) {
                var row = template.expandTemplate(FlowLayout.class, "hud-entry", Map.of());
                row.id(widget.id());
                var check = row.childById(CheckboxComponent.class, "hud-visible");
                row.childById(LabelComponent.class, "hud-label").text(Text.literal(widget.title()));
                check.checked(hud.visible(widget));
                check.onChanged(enabled -> { if (!updatingChecks) showHud.accept(widget, enabled); });
                var edit = row.childById(ButtonComponent.class, "hud-edit");
                edit
                    .renderer((context, button, delta) -> OwoXmlWindowContentAdapter.renderControlIcon(context, button,
                        new net.minecraft.util.Identifier("bong-client", "textures/gui/window/maximize-2.png"), 0xFF485A51))
                    .tooltip(Text.literal("编辑" + widget.title()));
                edit.onPress(button -> { toggleHud(); openHud.accept(widget); });
                hudRows.child(row);
                hudChecks.put(widget, check);
            }
            var motion = settings.childById(CheckboxComponent.class, "workspace-motion");
            motion.checked(preferences.motion());
            motion.onChanged(value -> { preferences.motion(value); save.run(); });
            settings.childById(ButtonComponent.class, "workspace-import").onPress(button ->
                Util.getOperatingSystem().open(backgrounds.directory().toFile()));
            settings.childById(ButtonComponent.class, "workspace-refresh").onPress(button -> {
                boolean loaded = backgrounds.select(preferences.background());
                status(loaded ? "" : "背景无法加载，保留当前图像");
                populateBackgrounds();
            });
            populateBackgrounds();
        }
        if (width != w || height != h) {
            width = w;
            height = h;
            settings.positioning(Positioning.absolute(Math.max(0, w - 264), Math.max(0, h - 230)));
            int listHeight = Math.max(60, Math.min(300, h - 40));
            hudList.verticalSizing(io.wispforest.owo.ui.core.Sizing.fixed(listHeight));
            hudList.childById(io.wispforest.owo.ui.container.ScrollContainer.class, "workspace-hud-scroll")
                .verticalSizing(io.wispforest.owo.ui.core.Sizing.fixed(listHeight - 30));
            hudList.positioning(Positioning.absolute(4, Math.max(0, h - listHeight - 32)));
            adapter.moveAndResize(0, 0, w, h);
        }
        boolean canPreview = canPreviewModels.getAsBoolean();
        if (canPreview != bar.children().contains(modelButton)) {
            if (canPreview) bar.child(3, modelButton);
            else bar.removeChild(modelButton);
        }
        var restore = bar.childById(io.wispforest.owo.ui.container.ScrollContainer.class, "workspace-restore-scroll");
        int restoreWidth = Math.max(1, w - (canPreview ? 130 : 90));
        if (restore.horizontalSizing().get().value != restoreWidth)
            restore.horizontalSizing(io.wispforest.owo.ui.core.Sizing.fixed(restoreWidth));
        List<UiWindowManager.WindowKey> minimized = manager.snapshot().stream()
            .filter(UiWindowManager.WindowState::minimized).map(UiWindowManager.WindowState::key).toList();
        if (!List.copyOf(restoreButtons.keySet()).equals(minimized)) {
            var row = bar.childById(FlowLayout.class, "workspace-minimized");
            row.clearChildren();
            restoreButtons.clear();
            for (var key : minimized) {
                var button = OwoXmlTemplateRegistry.production().require("workspace-controls")
                    .expandTemplate(ButtonComponent.class, "restore", Map.of());
                button.onPress(ignored -> {
                    manager.restore(key);
                    manager.snapshot().stream().filter(state -> state.key().equals(key)).findFirst().ifPresent(remember);
                });
                row.child(button);
                restoreButtons.put(key, button);
            }
        }
        restoreButtons.forEach((key, button) -> {
            String title = titles.apply(key);
            button.setMessage(Text.literal(MinecraftClient.getInstance().textRenderer.trimToWidth(title, 100)));
            button.tooltip(Text.literal(title));
        });
        updatingChecks = true;
        try { hudChecks.forEach((widget, check) -> check.checked(hud.visible(widget))); }
        finally { updatingChecks = false; }
    }

    private void populateBackgrounds() {
        var list = settings.childById(FlowLayout.class, "workspace-backgrounds");
        list.clearChildren();
        for (var entry : backgrounds.entries()) {
            var button = OwoXmlTemplateRegistry.production().require("workspace-controls")
                .expandTemplate(ButtonComponent.class, "background", Map.of());
            button.tooltip(Text.literal(entry.title()));
            button.renderer((context, value, delta) -> {
                backgrounds.thumbnail(context, entry.id(), value.getX(), value.getY(), value.getWidth(), value.getHeight());
                context.fill(value.getX(), value.getY() + 38, value.getX() + value.getWidth(),
                    value.getY() + value.getHeight(), 0xCC111819);
                context.drawText(MinecraftClient.getInstance().textRenderer,
                    MinecraftClient.getInstance().textRenderer.trimToWidth(entry.title(), value.getWidth() - 12),
                    value.getX() + 6, value.getY() + 43, 0xFFE1E8DC, false);
                context.drawRectOutline(value.getX(), value.getY(), value.getWidth(), value.getHeight(),
                    backgrounds.selected().equals(entry.id()) ? 0xFFDBBF7B : 0xFF65736B);
            });
            button.onPress(value -> {
                if (backgrounds.select(entry.id())) {
                    preferences.background(entry.id());
                    save.run();
                    status("");
                } else status("背景无法加载，保留当前图像");
            });
            list.child(button);
        }
    }

    private void status(String message) {
        settings.childById(LabelComponent.class, "workspace-background-status")
            .text(Text.literal(message.isEmpty() ? " " : message));
    }

    private void toggleSettings() {
        if (hudOpen) { adapter.rootComponent.removeChild(hudList); hudOpen = false; }
        settingsOpen = !settingsOpen;
        if (settingsOpen) adapter.rootComponent.child(settings);
        else adapter.rootComponent.removeChild(settings);
    }

    private void toggleHud() {
        if (settingsOpen) { adapter.rootComponent.removeChild(settings); settingsOpen = false; }
        hudOpen = !hudOpen;
        if (hudOpen) adapter.rootComponent.child(hudList);
        else adapter.rootComponent.removeChild(hudList);
    }

    public boolean hit(double x, double y) {
        return adapter != null && (bar.isInBoundingBox(x, y) || settingsOpen && settings.isInBoundingBox(x, y)
            || hudOpen && hudList.isInBoundingBox(x, y));
    }
    public boolean mouseDown(double x, double y, int button) {
        if (!hit(x, y)) return false;
        adapter.mouseClicked(x, y, button);
        return true;
    }
    public void mouseUp(double x, double y, int button) { adapter.mouseReleased(x, y, button); }
    public void scroll(double x, double y, double amount) { adapter.mouseScrolled(x, y, amount); }
    public boolean escape() {
        if (hudOpen) { toggleHud(); return true; }
        if (!settingsOpen) return false;
        toggleSettings();
        return true;
    }
    public UiWindowManager.Rect anchor(UiWindowManager.WindowKey key) {
        var button = restoreButtons.get(key);
        int x = button == null ? width / 2 - 56 : Math.max(28, Math.min(button.getX(), width - 112));
        return new UiWindowManager.Rect(x, height - 25, 112, 20);
    }
    public void render(DrawContext context, int x, int y, float delta) { adapter.render(context, x, y, delta); }
    public UiWindowManager.Rect boundsForPreview(String... path) {
        io.wispforest.owo.ui.core.Component component = adapter.rootComponent;
        for (String id : path) component = ((io.wispforest.owo.ui.core.ParentComponent) component)
            .childById(io.wispforest.owo.ui.core.Component.class, id);
        return new UiWindowManager.Rect(component.x(), component.y(), component.width(), component.height());
    }
    public void close() {
        if (adapter != null) adapter.dispose();
        adapter = null;
    }
}
