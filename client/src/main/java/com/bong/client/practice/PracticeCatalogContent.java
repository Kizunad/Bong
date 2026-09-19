package com.bong.client.practice;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.skill.SkillSetStore;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.TextBoxComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.*;
import io.wispforest.owo.ui.util.ScissorStack;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Util;
import net.minecraft.util.Identifier;
import java.util.List;
import java.util.function.Consumer;

/** 目录只管检索与浏览；选中不写槽位，双击才打开独立详情。 */
public final class PracticeCatalogContent {
    private final FlowLayout root = PracticeStyle.column();
    private final TextBoxComponent search = Components.textBox(Sizing.fill(100));
    private final Results results = new Results();
    private final Consumer<String> openDetail;
    private List<PracticeCatalog.Entry> entries = List.of(), filtered = List.of();
    private String query = "";

    public PracticeCatalogContent(Consumer<String> openDetail) {
        this.openDetail = openDetail;
        root.verticalSizing(Sizing.fill(100));
        root.padding(Insets.of(10));
        root.child(PracticeStyle.label("修习  /  所学皆有迹", PracticeStyle.GOLD));
        var frame = PracticeStyle.column();
        frame.padding(Insets.of(7));
        frame.surface(Surface.flat(0xFF0D191F).and(Surface.outline(0xFF51676B)));
        search.id("practice-search");
        search.setMaxLength(200);
        search.setDrawsBackground(false);
        search.setEditableColor(0xD8DDD1);
        search.verticalSizing(Sizing.fixed(16));
        search.setSuggestion("名称、描述，或 #技艺 #凡阶 #闪避");
        search.onChanged().subscribe(value -> {
            query = value;
            search.setSuggestion(value.isEmpty() ? "名称、描述，或 #技艺 #凡阶 #闪避" : null);
            filter(true);
        });
        frame.child(search);
        root.child(frame);
        var shortcuts = io.wispforest.owo.ui.container.Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(18));
        for (String tag : List.of("全部", "功法", "技艺", "闪避")) {
            var link = PracticeStyle.link(tag.equals("全部") ? tag : "#" + tag,
                () -> query(tag.equals("全部") ? "" : "#" + tag));
            link.horizontalSizing(Sizing.fill(25));
            shortcuts.child(link);
        }
        root.child(shortcuts);
        root.child(results);
        refresh();
    }

    public FlowLayout component() { return root; }
    public void query(String value) { search.text(value); }
    public String query() { return query; }
    public List<PracticeCatalog.Entry> visibleEntries() { return filtered; }
    public void layout(int width, int height) {
        int next = Math.max(36, height - 110);
        if (results.verticalSizing().get().value != next) results.verticalSizing(Sizing.fixed(next));
    }
    public void refresh() {
        var next = PracticeCatalog.entries(TechniquesListPanel.snapshot(), SkillSetStore.snapshot());
        if (!entries.equals(next)) { entries = next; filter(false); }
    }
    private void filter(boolean reset) {
        filtered = PracticeCatalog.filter(entries, query);
        if (reset) { results.target = 0; results.offset = 0; results.lastKey = ""; }
        results.clamp();
    }

    private final class Results extends BaseComponent {
        private static final int ROW = 58;
        private double offset, target;
        private String selected = "", lastKey = "";
        private long lastClick;
        Results() { id("practice-results"); sizing(Sizing.fill(100), Sizing.fixed(220)); cursorStyle(CursorStyle.HAND); }
        private void clamp() { target = Math.max(0, Math.min(target, Math.max(0, filtered.size() * ROW - height + 18))); }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partial, float delta) {
            clamp();
            offset += (target - offset) * Math.min(1, Math.max(0, delta) * .4);
            var font = MinecraftClient.getInstance().textRenderer;
            // owo 会拦截原版裁剪调用；嵌套 DrawContext 裁剪在恢复外层时会多压一层。
            ctx.draw();
            ScissorStack.push(x, y, width, height, ctx.getMatrices());
            try {
                ctx.drawText(font, filtered.size() + " 项  ·  双击展开", x + 2, y + 2, PracticeStyle.MUTED, false);
                ctx.draw();
                ScissorStack.push(x, y + 18, width, Math.max(0, height - 18), ctx.getMatrices());
                try {
                    for (int i = Math.max(0, (int) offset / ROW); i < filtered.size(); i++) {
                        int top = y + 18 + i * ROW - (int) offset;
                        if (top >= y + height) break;
                        var entry = filtered.get(i);
                        boolean active = entry.key().equals(selected);
                        boolean hover = mx >= x && mx < x + width - 5 && my >= Math.max(top, y + 18) && my < top + ROW - 3;
                        ctx.fillGradient(x, top, x + width - 5, top + ROW - 3,
                            active ? 0xFF30454B : hover ? 0xFF27393F : 0xFF18272D, 0xFF111D22);
                        ctx.fill(x, top + ROW - 4, x + width - 5, top + ROW - 3, 0xFF3F5251);
                        ctx.fill(x, top + 7, x + 2, top + ROW - 10, active ? PracticeStyle.GOLD : 0xFF5F817A);
                        int left = x + 10;
                        if (entry.isTechnique() && !entry.technique().iconTexture().isBlank()) {
                            Identifier icon = Identifier.tryParse(entry.technique().iconTexture());
                            if (icon != null) { ctx.drawTexture(icon, x + 7, top + 8, 0, 0, 28, 28, 28, 28); left += 32; }
                        }
                        ctx.drawText(font, font.trimToWidth(entry.name(), Math.max(1, width - (left - x) - 60)),
                            left, top + 8, PracticeStyle.TEXT, false);
                        ctx.drawText(font, entry.stage(), x + width - 12 - font.getWidth(entry.stage()), top + 8, PracticeStyle.GOLD, false);
                        String tags = String.join("  ", entry.tags().stream().limit(3).map(tag -> "#" + tag).toList());
                        ctx.drawText(font, font.trimToWidth(tags, Math.max(1, width - (left - x) - 12)), left, top + 25, PracticeStyle.MUTED, false);
                        ctx.fill(left, top + 43, x + width - 14, top + 46, 0xFF0B1418);
                        ctx.fill(left, top + 43, left + (int) ((width - (left - x) - 14) * entry.progress()), top + 46, 0xFF91AD99);
                    }
                    if (filtered.isEmpty()) ctx.drawText(font, "无匹配结果 · 调整名称或标签", x + 8, y + 38, PracticeStyle.MUTED, false);
                } finally {
                    try { ctx.draw(); }
                    finally { ScissorStack.pop(); }
                }
                int total = filtered.size() * ROW, available = height - 18;
                if (total > available && available > 0) {
                    int thumb = Math.max(8, available * available / total);
                    int top = y + 18 + (int) (offset / (total - available) * (available - thumb));
                    ctx.fill(x + width - 3, top, x + width, top + thumb, 0xFF91AD99);
                }
            } finally {
                try { ctx.draw(); }
                finally { ScissorStack.pop(); }
            }
        }
        @Override public boolean onMouseScroll(double mx, double my, double amount) { target -= amount * ROW * 1.5; clamp(); return true; }
        @Override public boolean onMouseDown(double mx, double my, int button) {
            if (button != 0 || my < 18 || mx >= width - 5) return false;
            int index = (int) (my - 18 + offset) / ROW;
            if (index < 0 || index >= filtered.size()) return true;
            var entry = filtered.get(index);
            selected = entry.key();
            long now = Util.getMeasuringTimeMs();
            if (lastKey.equals(selected) && now - lastClick <= 300) { lastKey = ""; openDetail.accept(selected); }
            else { lastKey = selected; lastClick = now; }
            return true;
        }
    }
}
