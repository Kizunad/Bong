package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.window.UiWindowManager;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.TextBoxComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.opengl.GL11;

import java.util.Objects;
import java.util.Map;

/** 同一个无 Screen 的 owo adapter 可挂到工作台或 HUD；业务 scope 始终归 manager。 */
public final class OwoXmlWindowContentAdapter implements AutoCloseable {
    private static final int HEADER_HEIGHT = 22;
    private static final int CONTROL_ICON_SIZE = 14;
    private static final Identifier LOCKED_ICON = new Identifier("bong-client", "textures/gui/window/lock-keyhole.png");
    private static final Identifier UNLOCKED_ICON = new Identifier("bong-client", "textures/gui/window/lock-keyhole-open.png");
    private static final Identifier MINIMIZE_ICON = new Identifier("bong-client", "textures/gui/window/minus.png");
    private static final Identifier CLOSE_ICON = new Identifier("bong-client", "textures/gui/window/x.png");
    private static final Identifier RESIZE_ICON = new Identifier("bong-client", "textures/gui/window/maximize-2.png");
    private final OwoUIAdapter<FlowLayout> adapter;
    private final FlowLayout content;
    private final FlowLayout contentSlot;
    private final FlowLayout sizeRow;
    private final FlowLayout sizeSlot;
    private final FlowLayout actions;
    private final UiWindowManager manager;
    private final UiWindowManager.WindowState state;
    private final Runnable preferencesChanged;
    private final TextBoxComponent widthInput;
    private final TextBoxComponent heightInput;
    private boolean workspace = true;
    private boolean sizeExpanded;
    private UiWindowManager.Rect bounds;
    private String title = "";

    public OwoXmlWindowContentAdapter(UiWindowManager manager, UiWindowManager.WindowState state,
                                      Runnable preferencesChanged) {
        Objects.requireNonNull(manager);
        this.manager = manager;
        this.state = state;
        this.preferencesChanged = preferencesChanged;
        bounds = state.bounds();
        var templates = OwoXmlTemplateRegistry.production();
        adapter = templates.require("window-frame").createAdapterWithoutScreen(
            bounds.x(), bounds.y(), bounds.width(), bounds.height(), FlowLayout.class);
        try {
            adapter.rootComponent.surface((context, component) -> {
                int x = component.x(), y = component.y(), w = component.width(), h = component.height();
                context.fillGradient(x, y, x + w, y + h, 0xFF292C2D, 0xFF17191B);
                context.drawRectOutline(x, y, w, h, 0xFF626762);
                context.fill(x + 1, y + 1, x + w - 1, y + 2, 0xFF9C9D8C);
                context.fill(x + 1, y + 2, x + 2, y + h - 1, 0xFF494E49);
                context.fill(x + 1, y + h - 2, x + w - 1, y + h - 1, 0xFF090B0C);
            });
            adapter.rootComponent.childById(FlowLayout.class, "window-header").surface((context, component) -> {
                int x = component.x(), y = component.y(), w = component.width(), h = component.height();
                context.fillGradient(x + 2, y + 2, x + w - 2, y + h, 0xFF484D48, 0xFF292E2C);
                context.fill(x + 2, y + h - 1, x + w - 2, y + h, 0xFF0D1110);
                context.fill(x + 7, y + h - 2, x + 32, y + h - 1, 0xFFB4A77C);
            });
            content = templates.require(state.definition().templateId())
                .expandTemplate(FlowLayout.class, "content", Map.of());
            contentSlot = adapter.rootComponent.childById(FlowLayout.class, "window-content");
            sizeRow = adapter.rootComponent.childById(FlowLayout.class, "window-size");
            sizeSlot = adapter.rootComponent.childById(FlowLayout.class, "window-size-slot");
            actions = adapter.rootComponent.childById(FlowLayout.class, "window-actions");
            widthInput = adapter.rootComponent.childById(TextBoxComponent.class, "window-width");
            heightInput = adapter.rootComponent.childById(TextBoxComponent.class, "window-height");
            resetSizeDraft();
            contentSlot.child(content);
            adapter.rootComponent.childById(ButtonComponent.class, "window-close")
                .renderer((context, button, delta) -> renderControlIcon(context, button, CLOSE_ICON, 0xFF824548))
                .onPress(button -> manager.close(state.key()));
            adapter.rootComponent.childById(ButtonComponent.class, "window-minimize")
                .renderer((context, button, delta) -> renderControlIcon(context, button, MINIMIZE_ICON, 0xFF454D46))
                .onPress(button -> {
                    cancelInput();
                    setSizeExpanded(false);
                    manager.minimize(state.key());
                    preferencesChanged.run();
                });
            var pin = adapter.rootComponent.childById(ButtonComponent.class, "window-pin");
            pin.renderer((context, button, delta) -> renderControlIcon(context, button,
                state.pinned() ? LOCKED_ICON : UNLOCKED_ICON, 0xFF454D46));
            pin.tooltip(Text.literal(state.pinned() ? "取消 HUD 固定" : "固定到 HUD"));
            pin.onPress(button -> {
                manager.pin(state.key(), !state.pinned());
                pin.tooltip(Text.literal(state.pinned() ? "取消 HUD 固定" : "固定到 HUD"));
                preferencesChanged.run();
            });
            if (state.definition().supports(com.bong.client.ui.window.UiWindowDefinition.Capability.STATION)) {
                actions.removeChild(pin);
            }
            adapter.rootComponent.childById(ButtonComponent.class, "window-size-toggle")
                .renderer((context, button, delta) -> renderControlIcon(context, button, RESIZE_ICON, 0xFF454D46))
                .onPress(button -> setSizeExpanded(!sizeExpanded));
            adapter.rootComponent.childById(ButtonComponent.class, "window-resize")
                .onPress(button -> submitSize());
            sizeSlot.removeChild(sizeRow);
            applyBounds();
        } catch (RuntimeException | Error failure) {
            adapter.dispose();
            throw failure;
        }
    }

    static void renderControlIcon(OwoUIDrawContext context, ButtonComponent button,
                                          Identifier texture, int hoverColor) {
        if (button.isHovered()) {
            context.fill(button.getX() + 1, button.getY() + 1,
                button.getX() + button.getWidth() - 1, button.getY() + button.getHeight() - 1, hoverColor);
        }
        context.draw();
        boolean blending = GL11.glIsEnabled(GL11.GL_BLEND);
        RenderSystem.enableBlend();
        try {
            context.drawTexture(texture,
                button.getX() + (button.getWidth() - CONTROL_ICON_SIZE) / 2,
                button.getY() + (button.getHeight() - CONTROL_ICON_SIZE) / 2,
                CONTROL_ICON_SIZE, CONTROL_ICON_SIZE, 0, 0, 48, 48, 48, 48);
        } finally {
            if (!blending) RenderSystem.disableBlend();
        }
    }

    public FlowLayout content() { return content; }

    public void closeAction(Runnable action) {
        adapter.rootComponent.childById(ButtonComponent.class, "window-close").onPress(button -> action.run());
    }

    public void title(String value) {
        if (title.equals(value)) return;
        title = value;
        updateTitle();
    }

    private void updateTitle() {
        String visible = MinecraftClient.getInstance().textRenderer
            .trimToWidth(title, Math.max(0, bounds.width() - (workspace ? 102 : 18)));
        adapter.rootComponent.childById(LabelComponent.class, "window-title")
            .text(Text.literal(visible.isEmpty() ? " " : visible));
    }

    public boolean headerAt(double x, double y) {
        return bounds.contains(x, y) && y < bounds.y() + HEADER_HEIGHT
            && x < bounds.x() + bounds.width() - 88;
    }

    public void layout(UiWindowManager.Rect next) {
        if (bounds.equals(next)) return;
        bounds = next;
        applyBounds();
    }

    private void applyBounds() {
        updateTitle();
        contentSlot.verticalSizing(Sizing.fixed(Math.max(1, bounds.height() - HEADER_HEIGHT - (sizeExpanded ? 24 : 0))));
        adapter.moveAndResize(bounds.x(), bounds.y(), bounds.width(), bounds.height());
    }

    public void workspace(boolean enabled) {
        if (workspace == enabled) return;
        cancelInput();
        setSizeExpanded(false);
        if (enabled) {
            adapter.rootComponent.child(actions);
        } else {
            adapter.rootComponent.removeChild(actions);
        }
        workspace = enabled;
        applyBounds();
    }

    public boolean textFocused() {
        var handler = adapter.rootComponent.focusHandler();
        return handler != null && handler.focused() instanceof TextBoxComponent;
    }

    private void setSizeExpanded(boolean expanded) {
        if (sizeExpanded == expanded) return;
        cancelInput();
        resetSizeDraft();
        sizeExpanded = expanded;
        if (expanded) sizeSlot.child(sizeRow);
        else sizeSlot.removeChild(sizeRow);
        applyBounds();
    }

    private void submitSize() {
        if (!manager.resize(state.key(), widthInput.getText(), heightInput.getText())) {
            widthInput.setEditableColor(0xEE8F8F);
            heightInput.setEditableColor(0xEE8F8F);
            return;
        }
        setSizeExpanded(false);
        preferencesChanged.run();
    }

    private void resetSizeDraft() {
        widthInput.setText(Integer.toString(state.bounds().width()));
        heightInput.setText(Integer.toString(state.bounds().height()));
        widthInput.setEditableColor(0xDEE5DE);
        heightInput.setEditableColor(0xDEE5DE);
    }

    public void cancelInput() {
        adapter.mouseReleased(-1, -1, 0);
        adapter.mouseReleased(-1, -1, 1);
        if (adapter.rootComponent.focusHandler() != null) adapter.rootComponent.focusHandler().focus(null, null);
    }

    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        for (int spread = 7; spread >= 1; spread--) {
            context.fill(bounds.x() - spread, bounds.y() - spread + 3,
                bounds.x() + bounds.width() + spread, bounds.y() + bounds.height() + spread + 3,
                0x08000000);
        }
        adapter.render(context, mouseX, mouseY, delta);
    }

    public void mouseDown(double x, double y, int button) { adapter.mouseClicked(x - bounds.x(), y - bounds.y(), button); }
    public void mouseUp(double x, double y, int button) { adapter.mouseReleased(x - bounds.x(), y - bounds.y(), button); }
    public void mouseDrag(double x, double y, int button, double dx, double dy) {
        adapter.mouseDragged(x - bounds.x(), y - bounds.y(), button, dx, dy);
    }
    public void scroll(double x, double y, double amount) { adapter.mouseScrolled(x - bounds.x(), y - bounds.y(), amount); }
    public boolean keyPressed(int key, int scan, int mods) {
        if (sizeExpanded && key == GLFW.GLFW_KEY_ESCAPE) {
            setSizeExpanded(false);
            return true;
        }
        if (textFocused()) {
            if (sizeExpanded && (widthInput.isFocused() || heightInput.isFocused())
                && (key == GLFW.GLFW_KEY_ENTER || key == GLFW.GLFW_KEY_KP_ENTER)) {
                submitSize();
                return true;
            }
            adapter.keyPressed(key, scan, mods);
            return true;
        }
        return adapter.keyPressed(key, scan, mods);
    }
    public boolean charTyped(char chr, int mods) { return adapter.charTyped(chr, mods); }

    @Override
    public void close() { adapter.dispose(); }
}
