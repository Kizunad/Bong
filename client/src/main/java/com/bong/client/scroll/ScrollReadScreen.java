package com.bong.client.scroll;

import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * 通用可阅读残卷阅读屏（plan-scroll-reading-v1 P1，范式 A，仿 {@code InsightOfferScreen}）。
 *
 * <p><b>可复用壳</b>：本屏只依赖 {@link ScrollOpenViewModel} 的 title / bodyPages 字段渲染，
 * 不 hardcode 任何具体残卷内容——第二卷任意 {@code readable_scroll_spec} 挂的物品，server 只
 * 需 emit 同一 {@code ScrollOpen} payload，client 零改动即可读。
 *
 * <p>两个已知坑的规避：
 * <ul>
 *   <li>viewport 高度用 {@code Sizing.fixed(VIEWPORT_HEIGHT_PX)}——{@code Sizing.fill(100)}
 *       会撑满父容器整高，viewport 解算到 >= 内容高度导致滚动条判定"无需滚动"而拖不动
 *       （手搓台配方列表 PR#816 同坑）。</li>
 *   <li>翻页刷新用 {@link LabelComponent#text(Text)} 原地更新正文/页码 label，不
 *       {@code clearChildren()} 重建 —— owo {@code ScrollContainer.layout()} 会在内容清空
 *       瞬间把 {@code maxScroll} 算成 0 并 clamp {@code scrollOffset} 回 0，导致翻页后滚动位置
 *       回弹到顶部（仿 {@code CraftRecipeListWidget} 的 diff 原地更新模式）。</li>
 * </ul>
 *
 * <p>关闭方式：ESC 或点击关闭按钮 → {@link ScrollReadStore#close()}（回传 C2S
 * {@code ScrollReadClosed} + 清空 store）→ {@code setScreen(null)}。{@link #shouldPause()}
 * 恒为 {@code false}（阅读不暂停游戏，对齐全仓其余 owo screen 惯例）。
 */
public final class ScrollReadScreen extends BaseOwoScreen<FlowLayout> {
    private static final int PANEL_WIDTH = 260;
    private static final int OUTER_PADDING = 14;
    private static final int VIEWPORT_HEIGHT_PX = 160;
    private static final int BODY_MAX_WIDTH_PX = PANEL_WIDTH - OUTER_PADDING * 2 - 8;

    private static final int COLOR_TITLE = 0xFFE9D9A6;
    private static final int COLOR_BODY = 0xFFDCD0B4;
    private static final int COLOR_META = 0xFFAAAAAA;
    private static final int COLOR_NAV = 0xFFB0A37C;
    private static final int COLOR_CLOSE = 0xFFAAAAAA;

    private final ScrollOpenViewModel viewModel;

    private int currentPage;
    private LabelComponent bodyLabel;
    private LabelComponent pageIndicatorLabel;
    private boolean closed;

    public ScrollReadScreen(ScrollOpenViewModel viewModel) {
        super(Text.literal(Objects.requireNonNull(viewModel, "viewModel").title()));
        this.viewModel = viewModel;
        this.currentPage = 0;
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.fixed(PANEL_WIDTH), Sizing.content());
        panel.surface(Surface.DARK_PANEL);
        panel.padding(Insets.of(OUTER_PADDING));
        panel.gap(6);
        panel.horizontalAlignment(HorizontalAlignment.CENTER);

        panel.child(Components.label(Text.literal(viewModel.title()).formatted(Formatting.BOLD))
            .color(Color.ofArgb(COLOR_TITLE)));

        FlowLayout bodyContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        bodyContent.padding(Insets.right(6));
        bodyLabel = Components.label(Text.literal(viewModel.page(currentPage)))
            .color(Color.ofArgb(COLOR_BODY))
            .maxWidth(BODY_MAX_WIDTH_PX);
        bodyContent.child(bodyLabel);

        // viewport 高度必须 Sizing.fixed —— 见类注释「已知坑」#1。
        var scroll = Containers.verticalScroll(Sizing.fill(100), Sizing.fixed(VIEWPORT_HEIGHT_PX), bodyContent);
        scroll.scrollbarThiccness(3);
        panel.child(scroll);

        if (viewModel.pageCount() > 1) {
            FlowLayout nav = Containers.horizontalFlow(Sizing.content(), Sizing.content());
            nav.gap(10);
            nav.verticalAlignment(VerticalAlignment.CENTER);
            nav.child(navButton("◀ 上一页", this::goPrevPage));
            pageIndicatorLabel = Components.label(Text.literal(pageIndicatorText()))
                .color(Color.ofArgb(COLOR_META));
            nav.child(pageIndicatorLabel);
            nav.child(navButton("下一页 ▶", this::goNextPage));
            panel.child(nav);
        }

        FlowLayout closeRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        closeRow.padding(Insets.top(6));
        closeRow.child(buildCloseButton());
        panel.child(closeRow);

        root.child(panel);
    }

    private FlowLayout navButton(String label, Runnable onClick) {
        FlowLayout box = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        box.surface(Surface.PANEL_INSET);
        box.padding(Insets.of(4, 4, 10, 10));
        box.child(Components.label(Text.literal(label)).color(Color.ofArgb(COLOR_NAV)));
        box.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                onClick.run();
                return true;
            }
            return false;
        });
        return box;
    }

    private FlowLayout buildCloseButton() {
        FlowLayout box = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        box.surface(Surface.PANEL_INSET);
        box.padding(Insets.of(6, 6, 14, 14));
        box.child(Components.label(Text.literal("[ 合上卷轴 ]").formatted(Formatting.ITALIC))
            .color(Color.ofArgb(COLOR_CLOSE)));
        box.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                close();
                return true;
            }
            return false;
        });
        return box;
    }

    // ─── 翻页：diff 原地更新，不 clearChildren（防滚动回弹，见类注释「已知坑」#2）───

    private void goPrevPage() {
        setPage(currentPage - 1);
    }

    private void goNextPage() {
        setPage(currentPage + 1);
    }

    /** 切页：index 越界（含负数）时钳到最近边界；已在边界上再点同方向按钮是 no-op。 */
    private void setPage(int index) {
        int clamped = viewModel.clampPageIndex(index);
        if (clamped == currentPage) {
            return;
        }
        currentPage = clamped;
        bodyLabel.text(Text.literal(viewModel.page(currentPage)));
        if (pageIndicatorLabel != null) {
            pageIndicatorLabel.text(Text.literal(pageIndicatorText()));
        }
    }

    private String pageIndicatorText() {
        return (currentPage + 1) + " / " + viewModel.pageCount();
    }

    // ─── 关闭 ──────────────────────────────────────────────────────────────

    @Override
    public void close() {
        if (!closed) {
            closed = true;
            ScrollReadStore.close();
        }
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    ScrollOpenViewModel viewModel() {
        return viewModel;
    }

    int currentPageForTests() {
        return currentPage;
    }

    boolean closedForTests() {
        return closed;
    }

    /** 测试用：纯函数式渲染描述，便于 assert 关键文本（不依赖 owo/MC 运行时）。 */
    public static RenderContent describe(ScrollOpenViewModel viewModel, int page) {
        Objects.requireNonNull(viewModel, "viewModel");
        int clamped = viewModel.clampPageIndex(page);
        List<String> lines = new ArrayList<>();
        lines.add(viewModel.title());
        lines.add(viewModel.page(clamped));
        if (viewModel.pageCount() > 1) {
            lines.add((clamped + 1) + " / " + viewModel.pageCount());
        }
        lines.add("[ 合上卷轴 ]");
        return new RenderContent(lines);
    }

    public record RenderContent(List<String> lines) {
        public RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
