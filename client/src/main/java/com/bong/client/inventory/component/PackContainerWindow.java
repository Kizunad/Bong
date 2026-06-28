package com.bong.client.inventory.component;

import com.bong.client.inventory.WornContainerPanel;
import io.wispforest.owo.ui.container.DraggableContainer;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Positioning;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

/**
 * plan-tarkov-floating-windows —— 单个套包内含物的可拖动悬浮窗口。
 *
 * <p><b>机制选型（真活非死代码）</b>：继承 owo {@link DraggableContainer}。其
 * {@code onMouseDrag} 真实累加 {@code xOffset/yOffset} 并 {@code updateX/Y} 移位（非注释占位）；
 * {@code childAt} 在顶部 {@code foreheadSize} 标题栏区返回 {@code this}（owo focusHandler 聚焦窗口
 * → 后续 drag 路由到 onMouseDrag），grid 区返回格子组件 —— 窗口拖动 / 物品拖动天然互斥。
 * {@code alwaysOnTop} 在 draw 前后把渲染 z 抬 +500 盖过主面板。仓库内 {@code DraggableContainer}
 * 此前零命中，是干净的新机制（区别于 SkillConfigFloatingWindow.dragBy 那种零调用点的死代码）。</p>
 *
 * <p><b>内容复用</b>：包裹一个已 {@code build()} 的 {@link WornContainerPanel}（grid + 包重 label +
 * InventoryStateStore snapshot listener 全部沿用已验证逻辑），本窗口只额外贡献「可拖动标题栏 + ✕ 关闭
 * 按钮 + z-order」。{@link #grid()} / {@link #containerId()} / {@link #isClosed()} / {@link #dispose()}
 * 透传内含面板。</p>
 *
 * <p><b>z-order（多窗前后）</b>：{@code alwaysOnTop} 只保证整体盖过主面板；多个悬浮窗之间的前后由
 * 子节点渲染顺序决定。点击置顶用 {@link #bringToFront(FlowLayout)}：把当前 offset bake 进
 * {@link Positioning#absolute}、offset 归零、从 root removeChild 后再 child（末尾=最后渲染=视觉最上），
 * 坐标守恒不跳位。</p>
 */
public final class PackContainerWindow extends DraggableContainer<FlowLayout> {

    private static final int FOREHEAD = 18;
    private static final int TITLE_BG = 0xFF2A1F18;
    private static final int TITLE_FG = 0xFFE0D5C5;
    private static final int CLOSE_FG = 0xFFAAAAAA;
    private static final int CLOSE_FG_HOVER = 0xFFFF6666;
    private static final int CLOSE_BTN_WIDTH = 14;

    private final WornContainerPanel inner;

    /**
     * 包裹一个<b>已 {@code build()}</b> 的 {@link WornContainerPanel}。owo {@code super(...)} 要求
     * child 为首个表达式，故内含面板必须在外部先 build；{@link #builtContent} 兜底校验。
     */
    public PackContainerWindow(WornContainerPanel inner) {
        super(Sizing.content(), Sizing.content(), builtContent(inner));
        this.inner = inner;
        this.foreheadSize(FOREHEAD);
        this.alwaysOnTop(true);
    }

    private static FlowLayout builtContent(WornContainerPanel inner) {
        if (inner == null) {
            throw new IllegalArgumentException("inner WornContainerPanel must not be null");
        }
        FlowLayout content = inner.container();
        if (content == null) {
            throw new IllegalStateException(
                "WornContainerPanel.build() must be called before wrapping in a PackContainerWindow");
        }
        return content;
    }

    /** 该容器 id（{@code pack_<instance_id>}），透传内含面板。 */
    public String containerId() {
        return inner.containerId();
    }

    /** 内含物 grid，供 InspectScreen 拖拽落位 / 拾取的 hit-test。 */
    public BackpackGridPanel grid() {
        return inner.grid();
    }

    /** 是否已 dispose（listener 解绑）。 */
    public boolean isClosed() {
        return inner.isClosed();
    }

    /** 解绑内含面板的 InventoryStateStore 订阅，防止泄漏。 */
    public void dispose() {
        inner.dispose();
    }

    /**
     * 把窗口提到 z 最上：bake 当前拖动 offset 进绝对坐标、offset 归零、从 root removeChild 后再
     * child（末尾=最后渲染=最上）。坐标守恒（拖完置顶不跳位）。root 为 null 时仅做 offset bake。
     */
    public void bringToFront(FlowLayout root) {
        int cx = this.baseX + (int) Math.round(this.xOffset);
        int cy = this.baseY + (int) Math.round(this.yOffset);
        if (root != null) {
            root.removeChild(this);
        }
        this.xOffset = 0;
        this.yOffset = 0;
        this.positioning(Positioning.absolute(cx, cy));
        if (root != null) {
            root.child(this);
        }
    }

    /** ✕ 关闭按钮命中检测（标题栏右上角）。布局未就绪（width==0）时恒 false。 */
    public boolean isOverCloseButton(double mouseX, double mouseY) {
        int w = width();
        if (w <= 0) {
            return false;
        }
        int btnLeft = x() + w - CLOSE_BTN_WIDTH;
        return mouseX >= btnLeft && mouseX <= x() + w
            && mouseY >= y() && mouseY <= y() + FOREHEAD;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        // 标题栏与 child 同抬 +500（alwaysOnTop），保证盖过主面板；与 super.draw 的 push 平衡（各自闭合）。
        boolean top = alwaysOnTop();
        if (top) {
            context.getMatrices().translate(0, 0, 500);
        }
        int w = width();
        context.fill(x(), y(), x() + w, y() + FOREHEAD, TITLE_BG);
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.textRenderer != null) {
            context.drawTextWithShadow(
                mc.textRenderer, Text.literal(inner.displayName()), x() + 4, y() + 5, TITLE_FG);
            boolean hoverClose = isOverCloseButton(mouseX, mouseY);
            context.drawTextWithShadow(
                mc.textRenderer, Text.literal("✕"),
                x() + w - CLOSE_BTN_WIDTH + 3, y() + 5,
                hoverClose ? CLOSE_FG_HOVER : CLOSE_FG);
        }
        if (top) {
            context.getMatrices().translate(0, 0, -500);
        }
        super.draw(context, mouseX, mouseY, partialTicks, delta);
    }

    // ── package-private 测试可见的 offset / 坐标访问器（headless 拖动移位 / 坐标守恒断言用）──

    double xOffsetForTest() {
        return this.xOffset;
    }

    double yOffsetForTest() {
        return this.yOffset;
    }

    int baseXForTest() {
        return this.baseX;
    }

    int baseYForTest() {
        return this.baseY;
    }
}
