package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.window.UiWindowManager;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.Objects;
import java.util.Map;

/** 同一个无 Screen 的 owo adapter 可挂到工作台或 HUD；业务 scope 始终归 manager。 */
public final class OwoXmlWindowContentAdapter implements AutoCloseable {
    private static final int HEADER_HEIGHT = 22;
    private final OwoUIAdapter<FlowLayout> adapter;
    private final FlowLayout content;
    private final FlowLayout contentSlot;
    private UiWindowManager.Rect bounds;
    private String title = "";

    public OwoXmlWindowContentAdapter(UiWindowManager manager, UiWindowManager.WindowState state) {
        Objects.requireNonNull(manager);
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
            contentSlot.child(content);
            adapter.rootComponent.childById(ButtonComponent.class, "window-close")
                .onPress(button -> manager.close(state.key()));
            applyBounds();
        } catch (RuntimeException | Error failure) {
            adapter.dispose();
            throw failure;
        }
    }

    public FlowLayout content() { return content; }

    public void title(String value) {
        title = value;
        updateTitle();
    }

    private void updateTitle() {
        String visible = MinecraftClient.getInstance().textRenderer
            .trimToWidth(title, Math.max(0, bounds.width() - 38));
        adapter.rootComponent.childById(LabelComponent.class, "window-title")
            .text(Text.literal(visible.isEmpty() ? " " : visible));
    }

    public boolean headerAt(double x, double y) {
        return bounds.contains(x, y) && y < bounds.y() + HEADER_HEIGHT
            && x < bounds.x() + bounds.width() - 22;
    }

    public void layout(UiWindowManager.Rect next) {
        if (bounds.equals(next)) return;
        bounds = next;
        applyBounds();
    }

    private void applyBounds() {
        updateTitle();
        contentSlot.verticalSizing(Sizing.fixed(Math.max(1, bounds.height() - HEADER_HEIGHT)));
        adapter.moveAndResize(bounds.x(), bounds.y(), bounds.width(), bounds.height());
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
    public boolean keyPressed(int key, int scan, int mods) { return adapter.keyPressed(key, scan, mods); }
    public boolean charTyped(char chr, int mods) { return adapter.charTyped(chr, mods); }

    @Override
    public void close() { adapter.dispose(); }
}
