package com.bong.client.ui.model;

import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.List;
import java.util.Locale;

/** 目录是带搜索的分类列表；右侧始终保留取景区和操作栏，不随条目数量挤出窗口。 */
public final class ModelPreviewContent implements AutoCloseable {
    private final ModelPreviewComponent preview = new ModelPreviewComponent();
    private final List<ModelPreviewCatalog.Entry> catalog = ModelPreviewCatalog.entries();
    private final ModelList list = new ModelList();
    private final FlowLayout root = Containers.horizontalFlow(Sizing.fill(100), Sizing.fill(100));
    private final FlowLayout stage = Containers.verticalFlow(Sizing.fixed(1), Sizing.fill(100));
    private int layoutWidth, layoutHeight;
    private ModelPreviewCatalog.Category category = ModelPreviewCatalog.Category.PLAYER;
    private String query = "";

    public ModelPreviewContent() {
        root.padding(Insets.of(8));
        root.gap(8);
        var sidebar = Containers.verticalFlow(Sizing.fixed(122), Sizing.fill(100));
        sidebar.gap(6);
        sidebar.child(new Categories());
        var search = Components.textBox(Sizing.fill(100));
        search.id("model-search");
        search.verticalSizing(Sizing.fixed(20));
        search.setSuggestion("搜索名称 / ID");
        search.setMaxLength(100);
        search.setDrawsBackground(false);
        search.setEditableColor(0xCCDCE1);
        search.onChanged().subscribe(text -> {
            query = text.strip().toLowerCase(Locale.ROOT);
            search.setSuggestion(query.isEmpty() ? "搜索名称 / ID" : null);
            filter();
        });
        var searchFrame = Containers.verticalFlow(Sizing.fill(100), Sizing.fixed(24));
        searchFrame.padding(Insets.of(3));
        searchFrame.surface((ctx, component) -> {
            ctx.fill(component.x(), component.y(), component.x() + component.width(), component.y() + component.height(), 0xFF10191E);
            ctx.fill(component.x(), component.y() + component.height() - 1,
                component.x() + component.width(), component.y() + component.height(), 0xFF506774);
        });
        searchFrame.child(search);
        sidebar.child(searchFrame);
        sidebar.child(list);
        root.child(sidebar);
        stage.gap(6);
        stage.child(new Caption());
        preview.verticalSizing(Sizing.fixed(1));
        stage.child(preview);
        stage.child(new Controls());
        root.child(stage);
        filter();
    }

    public FlowLayout component() { return root; }
    public ModelPreviewComponent preview() { return preview; }
    public void layout(int width, int height) {
        if (layoutWidth == width && layoutHeight == height) return;
        layoutWidth = width; layoutHeight = height;
        stage.horizontalSizing(Sizing.fixed(Math.max(1, width - 146)));
        preview.verticalSizing(Sizing.fixed(Math.max(1, height - 104)));
        list.verticalSizing(Sizing.fixed(Math.max(1, height - 96)));
    }
    public void invalidate() { preview.invalidate(); }
    @Override public void close() { preview.close(); }

    private void filter() {
        list.entries = catalog.stream().filter(entry -> entry.category() == category)
            .filter(entry -> query.isEmpty() || entry.label().toLowerCase(Locale.ROOT).contains(query)
                || entry.id().toLowerCase(Locale.ROOT).contains(query)).toList();
        list.first = 0;
    }

    private static void text(OwoUIDrawContext ctx, String value, int x, int y, int maxWidth, int color) {
        var font = MinecraftClient.getInstance().textRenderer;
        ctx.drawText(font, font.trimToWidth(value, Math.max(1, maxWidth)), x, y, color, false);
    }

    private final class Categories extends BaseComponent {
        Categories() { id("model-categories"); sizing(Sizing.fill(100), Sizing.fixed(44)); cursorStyle(CursorStyle.HAND); }
        @Override public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY, float partialTicks, float delta) {
            var categories = ModelPreviewCatalog.Category.values();
            for (int i = 0; i < categories.length; i++) {
                int left = x + (i % 2) * width / 2, top = y + i / 2 * 22;
                boolean active = categories[i] == category;
                if (active) ctx.fill(left, top, left + width / 2 - 2, top + 21, 0xFF2E424C);
                text(ctx, categories[i].label, left + 6, top + 6, width / 2 - 10, active ? 0xFFD2B780 : 0xFF99ACB6);
                if (active) ctx.fill(left + 5, top + 20, left + width / 2 - 7, top + 21, 0xFFC7AB73);
            }
        }
        @Override public boolean onMouseDown(double mx, double my, int button) {
            if (button != 0) return false;
            int index = (int) (my / 22) * 2 + (mx < width / 2.0 ? 0 : 1);
            if (index >= 0 && index < ModelPreviewCatalog.Category.values().length) {
                category = ModelPreviewCatalog.Category.values()[index];
                filter();
            }
            return true;
        }
    }

    private final class ModelList extends BaseComponent {
        private static final int ROW_HEIGHT = 30;
        private List<ModelPreviewCatalog.Entry> entries = List.of();
        private int first;
        ModelList() { id("model-list"); sizing(Sizing.fill(100), Sizing.fixed(1)); cursorStyle(CursorStyle.HAND); }
        @Override public boolean canFocus(FocusSource source) { return true; }
        @Override public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY, float partialTicks, float delta) {
            ctx.fill(x, y, x + width, y + height, 0xFF131D23);
            ctx.enableScissor(x, y, x + width, y + height);
            try {
                if (entries.isEmpty()) text(ctx, "没有匹配模型", x + 6, y + 10, width - 12, 0xFF8F9FA8);
                for (int row = 0; row * ROW_HEIGHT < height && first + row < entries.size(); row++) {
                    var entry = entries.get(first + row);
                    int top = y + row * ROW_HEIGHT;
                    boolean selected = entry.id().equals(preview.option().id());
                    boolean hovered = mouseX >= x && mouseX < x + width && mouseY >= top && mouseY < top + ROW_HEIGHT;
                    if (hovered || selected) ctx.fill(x + 1, top, x + width - 3, top + ROW_HEIGHT - 1,
                        selected ? 0xFF30424A : 0xFF233039);
                    if (selected) ctx.fill(x + 1, top + 4, x + 3, top + ROW_HEIGHT - 5, 0xFFD2B780);
                    text(ctx, entry.label(), x + 8, top + 4, width - 16, selected ? 0xFFE1CBA0 : 0xFFCEDADF);
                    text(ctx, entry.id(), x + 8, top + 17, width - 16, 0xFF8297A4);
                }
                int total = entries.size() * ROW_HEIGHT;
                if (total > height) {
                    int thumb = Math.max(8, height * height / total);
                    int top = y + (height - thumb) * first / Math.max(1, entries.size() - rows());
                    ctx.fill(x + width - 2, top, x + width, top + thumb, 0xFF718894);
                }
            } finally { ctx.disableScissor(); }
        }
        private int rows() { return Math.max(1, height / ROW_HEIGHT); }
        @Override public boolean onMouseScroll(double mx, double my, double amount) {
            first = Math.max(0, Math.min(Math.max(0, entries.size() - rows()), first - (int) Math.signum(amount) * 3));
            return true;
        }
        @Override public boolean onMouseDown(double mx, double my, int button) {
            if (button != 0) return false;
            int index = first + (int) my / ROW_HEIGHT;
            if (index >= 0 && index < entries.size()) preview.option(entries.get(index));
            return true;
        }
        @Override public boolean onKeyPress(int key, int scan, int mods) {
            if (key != GLFW.GLFW_KEY_DOWN && key != GLFW.GLFW_KEY_UP) return false;
            if (entries.isEmpty()) return true;
            int selected = -1;
            for (int i = 0; i < entries.size(); i++) if (entries.get(i).id().equals(preview.option().id())) selected = i;
            int next = Math.max(0, Math.min(entries.size() - 1, selected + (key == GLFW.GLFW_KEY_DOWN ? 1 : -1)));
            preview.option(entries.get(next));
            if (next < first) first = next;
            if (next >= first + rows()) first = next - rows() + 1;
            return true;
        }
    }

    private final class Caption extends BaseComponent {
        Caption() { sizing(Sizing.fill(100), Sizing.fixed(28)); }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partialTicks, float delta) {
            text(ctx, preview.option().label(), x, y + 2, width, 0xFFE1CBA0);
            text(ctx, preview.option().id(), x, y + 16, width, 0xFF8FA5B0);
        }
    }

    private final class Controls extends BaseComponent {
        Controls() { id("model-controls"); sizing(Sizing.fill(100), Sizing.fixed(48)); cursorStyle(CursorStyle.HAND); }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partialTicks, float delta) {
            String[] labels = {preview.camera().autoRotate() ? "停止旋转" : "自动旋转",
                preview.material() ? "去除材质" : "恢复材质", "重新取景"};
            for (int i = 0; i < 3; i++) {
                int left = x + width * i / 3, right = x + width * (i + 1) / 3;
                if (mx >= left && mx < right && my >= y && my < y + 25) ctx.fill(left, y, right - 2, y + 25, 0xFF30424A);
                text(ctx, labels[i], left + 3, y + 8, right - left - 5, 0xFFCCDCE1);
                ctx.fill(left + 3, y + 25, right - 5, y + 26, i == 1 && !preview.material() ? 0xFFD2B780 : 0xFF566E7A);
            }
            text(ctx, preview.camera().autoRotate() ? "自动环绕 · 滚轮缩放" : "左右键拖拽旋转 · 滚轮缩放", x + 3, y + 35, width - 6, 0xFF8FA5B0);
        }
        @Override public boolean onMouseDown(double mx, double my, int button) {
            if (button != 0 || my >= 26) return false;
            int control = Math.min(2, (int) (mx * 3 / Math.max(1, width)));
            if (control == 0) preview.camera().autoRotate(!preview.camera().autoRotate());
            else if (control == 1) preview.material(!preview.material());
            else preview.camera().reset();
            return true;
        }
    }
}
