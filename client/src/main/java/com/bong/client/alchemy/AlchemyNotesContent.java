package com.bong.client.alchemy;

import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.ui.window.UiWindowDefinition;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import org.lwjgl.opengl.GL11;

import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 可独立阅读的丹方与炉记；翻阅不发请求，选作炉方才交给工位所有者。 */
public final class AlchemyNotesContent {
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "alchemy-notes", "alchemy-notes", 100, 100, Set.of(UiWindowDefinition.Capability.WINDOW));
    private static final Insets PAGE_PADDING = Insets.of(32, 28, 24, 24);
    private static final Insets COMPACT_PADDING = Insets.of(10, 10, 6, 6);
    private static final int ROW_HEIGHT = 20;
    private static final int GAP = 8;
    private final AlchemyWindows windows;
    private final FlowLayout root;
    private final FlowLayout navigation;
    private final ScrollContainer<FlowLayout> scroll;
    private final LabelComponent pageNumber;
    private final ButtonComponent previous;
    private final ButtonComponent next;
    private final ButtonComponent select;
    private AlchemyScreenViewModel rendered;
    private boolean ready;
    private InventoryItem item;
    private int page;
    private boolean history;
    private boolean compact;
    private String selectLabel = "选作炉方";
    private Object renderedBody;

    public AlchemyNotesContent(FlowLayout root, AlchemyWindows windows) {
        this.windows = windows;
        this.root = root;
        root.padding(PAGE_PADDING);
        root.gap(GAP);
        root.surface((context, component) -> {
            context.draw();
            boolean blending = GL11.glIsEnabled(GL11.GL_BLEND);
            RenderSystem.enableBlend();
            try {
                AlchemyPaperSurface.draw(context, paper(), component.x(), component.y(), component.width(), component.height(), compact ? 10 : 28);
            } finally {
                if (!blending) RenderSystem.disableBlend();
            }
        });
        navigation = row();
        previous = button("‹", "alchemy-notes-prev", () -> turn(-1));
        previous.sizing(Sizing.fixed(24), Sizing.fixed(ROW_HEIGHT));
        previous.tooltip(Text.literal("上一方"));
        next = button("›", "alchemy-notes-next", () -> turn(1));
        next.sizing(Sizing.fixed(24), Sizing.fixed(ROW_HEIGHT));
        next.tooltip(Text.literal("下一方"));
        select = button("选作炉方", "alchemy-notes-select", this::select);
        select.horizontalSizing(Sizing.fixed(88));
        pageNumber = Components.label(Text.empty());
        pageNumber.color(Color.ofRgb(0x726249));
        pageNumber.sizing(Sizing.fixed(36), Sizing.fixed(ROW_HEIGHT));
        navigation.children(List.of(previous, next, pageNumber, select));
        scroll = Containers.verticalScroll(Sizing.fill(100), Sizing.fixed(100), body());
        scroll.id("alchemy-notes-scroll");
        scroll.scrollbarThiccness(3);
        root.children(List.of(scroll, navigation));
        page = windows.model().recipes().currentIndex();
        refresh();
    }

    public static boolean isRecipeItem(InventoryItem item) {
        return item != null && (item.itemId().startsWith("recipe_scroll_")
            || item.itemId().startsWith("fragment_alchemy_"));
    }

    public void read(InventoryItem value) {
        item = value;
        history = false;
        refresh();
        scroll.scrollTo(0);
    }

    public void showHistory() {
        history = true;
        refresh();
        scroll.scrollTo(0);
    }

    public void layout(int width, int height) {
        compact = width < 240 || height < 150;
        var padding = compact ? COMPACT_PADDING : PAGE_PADDING;
        int gap = compact ? 4 : GAP;
        root.padding(padding);
        root.gap(gap);
        scroll.child().padding(compact ? Insets.of(0, 4, 0, 2) : Insets.of(4, 10, 2, 6));
        scroll.child().gap(compact ? 6 : 12);
        navigation.gap(compact ? 3 : 5);
        previous.horizontalSizing(Sizing.fixed(compact ? 16 : 24));
        next.horizontalSizing(Sizing.fixed(compact ? 16 : 24));
        pageNumber.horizontalSizing(Sizing.fixed(compact ? 24 : 36));
        select.horizontalSizing(Sizing.fixed(compact ? Math.max(20, width - padding.horizontal() - 65) : 88));
        updateSelectLabel();
        int controls = history ? 0 : ROW_HEIGHT;
        int spacing = Math.max(0, root.children().size() - 1) * gap;
        scroll.verticalSizing(Sizing.fixed(Math.max(1, height - padding.vertical() - controls - spacing)));
    }

    public Identifier paper() {
        return history ? AlchemyPaperSurface.JOURNAL : AlchemyPaperSurface.RECIPE;
    }

    public void tick() {
        if (!Objects.equals(rendered, windows.model()) || ready != windows.ready()) refresh();
    }

    private void turn(int delta) {
        item = null;
        int size = windows.model().recipes().learned().size();
        if (size > 0) page = Math.floorMod(page + delta, size);
        refresh();
        scroll.scrollTo(0);
    }

    private RecipeScrollStore.RecipeEntry selected() {
        var recipes = windows.model().recipes().learned();
        if (item != null) {
            // 只有旧协议明确定义了此前缀与 recipe_id 的映射，真实 fragment 只读物品正文。
            if (!item.itemId().startsWith("recipe_scroll_")) return null;
            String id = item.itemId().substring("recipe_scroll_".length());
            return recipes.stream().filter(recipe -> recipe.id().equals(id)).findFirst().orElse(null);
        }
        return recipes.isEmpty() ? null : recipes.get(Math.floorMod(page, recipes.size()));
    }

    private void select() {
        var recipe = selected();
        if (recipe == null) {
            if (item != null && item.itemId().startsWith("recipe_scroll_")) windows.learn(item);
        } else {
            var recipes = windows.model().recipes();
            int index = recipes.learned().indexOf(recipe);
            if (index >= 0 && index != recipes.currentIndex()) windows.turnPage(index - recipes.currentIndex());
        }
        refresh();
    }

    private void refresh() {
        List<io.wispforest.owo.ui.core.Component> originalRootChildren = List.copyOf(root.children());
        FlowLayout originalBody = scroll.child();
        AlchemyScreenViewModel originalRendered = rendered;
        boolean originalReady = ready;
        Object originalRenderedBody = renderedBody;
        String originalSelectLabel = selectLabel;
        try {
            refreshUnchecked();
        } catch (RuntimeException | Error failure) {
            Throwable rollbackFailure = null;
            rollbackFailure = restoreStep(rollbackFailure, root::clearChildren);
            rollbackFailure = restoreStep(rollbackFailure, () -> root.children(originalRootChildren));
            rollbackFailure = restoreStep(rollbackFailure, () -> scroll.child(originalBody));
            rollbackFailure = restoreStep(rollbackFailure, () -> {
                rendered = originalRendered;
                ready = originalReady;
                renderedBody = originalRenderedBody;
                selectLabel = originalSelectLabel;
                restoreControls();
            });
            if (rollbackFailure != null && rollbackFailure != failure) failure.addSuppressed(rollbackFailure);
            throw failure;
        }
    }

    private void refreshUnchecked() {
        rendered = windows.model();
        ready = windows.ready();
        var recipe = selected();
        if (history) root.removeChild(navigation);
        else if (!root.children().contains(navigation)) root.child(navigation);
        previous.active(!history && rendered.recipes().learned().size() > 1);
        next.active(previous.active());
        boolean canSelect = ready && !rendered.session().isActive() && !rendered.furnace().hasSession();
        boolean current = recipe != null && recipe.equals(rendered.recipes().current());
        selectLabel = current ? "本炉所用" : recipe != null ? "照此方炼" : "研读残卷";
        int count = rendered.recipes().learned().size();
        pageNumber.text(Text.literal(count == 0 ? "" : (Math.floorMod(page, count) + 1) + "/" + count));
        select.active(!history && canSelect && !current
            && (recipe != null || item != null && item.itemId().startsWith("recipe_scroll_")));
        // 进度、香料倒计时和请求反馈不改变正文，不能重建滚动容器。
        Object bodyKey = history ? List.of(true, rendered.history(), rendered.recipes())
            : List.of(false, recipe != null ? recipe : item != null ? item : "empty");
        if (!Objects.equals(renderedBody, bodyKey)) {
            var body = body();
            if (history) history(body);
            else if (recipe != null) {
                title(body, recipe.displayName());
                String attribution = java.util.stream.Stream.of(recipe.author(), recipe.era())
                    .filter(value -> !value.isBlank()).collect(java.util.stream.Collectors.joining(" · "));
                if (!attribution.isBlank()) line(body, attribution, 0x726249);
                line(body, recipe.bodyText().isBlank() ? "此卷尚无正文。" : recipe.bodyText(), 0x30291F);
            } else if (item != null) {
                title(body, item.displayName());
                line(body, item.description().isBlank() ? "此残卷尚无可读正文。" : item.description(), 0x30291F);
                item.alchemyLines().forEach(text -> line(body, text, 0x514634));
            } else {
                line(body, "卷中尚无丹方。", 0x592C21);
            }
            scroll.child(body);
            renderedBody = bodyKey;
        }
        layout(root.width(), root.height());
    }

    private void restoreControls() {
        if (rendered == null) return;
        previous.active(!history && rendered.recipes().learned().size() > 1);
        next.active(previous.active());
        int count = rendered.recipes().learned().size();
        pageNumber.text(Text.literal(count == 0 ? "" : (Math.floorMod(page, count) + 1) + "/" + count));
        updateSelectLabel();
        boolean canSelect = ready && !rendered.session().isActive() && !rendered.furnace().hasSession();
        var recipe = selected();
        boolean current = recipe != null && recipe.equals(rendered.recipes().current());
        select.active(!history && canSelect && !current
            && (recipe != null || item != null && item.itemId().startsWith("recipe_scroll_")));
    }

    private static Throwable restoreStep(Throwable primary, Runnable step) {
        try {
            step.run();
        } catch (RuntimeException | Error failure) {
            if (primary == null) return failure;
            if (primary != failure) primary.addSuppressed(failure);
        }
        return primary;
    }

    private void updateSelectLabel() {
        String label = compact ? switch (selectLabel) {
            case "本炉所用" -> "此方";
            case "照此方炼" -> "用方";
            default -> "研读";
        } : selectLabel;
        select.setMessage(ink(label));
    }

    private void history(FlowLayout body) {
        if (rendered.history().isEmpty()) line(body, "炉灰未落，纸上尚无一笔。", 0x514634);
        for (int index = rendered.history().size() - 1; index >= 0; index--) {
            var entry = rendered.history().get(index);
            String name = rendered.recipes().learned().stream().filter(recipe -> recipe.id().equals(entry.recipeId()))
                .map(RecipeScrollStore.RecipeEntry::displayName).findFirst().orElse("无名方");
            title(body, "第 " + (index + 1) + " 炉 · " + name);
            String account = switch (entry.bucket()) {
                case "perfect" -> "揭盖时药香清正，得上成之丸。";
                case "good" -> "炉火守得平稳，这一炉成了。";
                case "flawed" -> "药已成形，成色却有欠缺。下回还须细守火候。";
                case "waste" -> "翻检炉底，只余焦渣与药糊。";
                case "explode" -> "炉内猛然作响，顶盖弹起，这一炉毁了。";
                default -> "收起此炉所得，留待细察。";
            };
            line(body, account, 0x30291F);
            if (entry.quality() != null) line(body, String.format("记：成色约 %.0f 成。", entry.quality() * 10), 0x514634);
            if (entry.toxinAmount() != null && entry.toxinAmount() > 0) line(body, "药中仍留余毒，不可贪服。", 0x843A2D);
            if (entry.damage() != null && entry.damage() > 0) line(body, String.format("反噬伤身，损气血 %.1f。", entry.damage()), 0x843A2D);
            if (entry.flawedPath()) line(body, "此炉循残方而炼，药性须慎辨。", 0x514634);
        }
    }

    private static FlowLayout body() {
        var body = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        body.padding(Insets.of(4, 10, 2, 6));
        body.gap(12);
        return body;
    }

    private static FlowLayout row() {
        var row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(ROW_HEIGHT));
        row.gap(5);
        return row;
    }

    private static ButtonComponent button(String title, String id, Runnable action) {
        var button = Components.button(ink(title), ignored -> action.run());
        button.id(id);
        button.sizing(Sizing.fixed(70), Sizing.fixed(ROW_HEIGHT));
        button.textShadow(false);
        button.renderer((context, component, delta) -> {
            int ink = component.active() ? 0xFF60392B : 0xFF8C7B61;
            if (component.isHovered() && component.active()) {
                context.fill(component.getX() + 5, component.getY() + component.getHeight() - 2,
                    component.getX() + component.getWidth() - 5, component.getY() + component.getHeight() - 1, ink);
            }
            // owo renderer 只画背景，文本仍由 ButtonComponent 绘制，不能在这里重复画。
        });
        return button;
    }

    private static void line(FlowLayout body, String text, int color) {
        var label = Components.label(Text.literal(text));
        label.color(Color.ofRgb(color));
        label.horizontalSizing(Sizing.fill(100));
        label.lineHeight(11);
        body.child(label);
    }

    private static Text ink(String text) {
        return Text.literal(text).styled(style -> style.withColor(0x60392B));
    }

    private static void title(FlowLayout body, String text) {
        var label = Components.label(Text.literal(text).styled(style -> style.withBold(true)));
        label.color(Color.ofRgb(0x592C21));
        label.horizontalSizing(Sizing.fill(100));
        label.lineHeight(13);
        body.child(label);
    }
}
