package com.bong.client.botany;

/**
 * plan-botany-v1 §1.3 可拖拽浮窗的全局状态。
 *
 * <p>策略：
 * <ul>
 * <li>planner 每帧基于 projection anchor + 累计 delta 计算最终 panel 位置，并把结果写回
 * {@link #recordRenderedBounds(int, int, int, int)}。</li>
 * <li>鼠标左键按下时，mixin 调 {@link #onLeftButton(int, double, double)}；若命中已渲染的 panel 范围
 * 则开始拖拽并消耗点击；否则不干预。</li>
 * <li>每 client tick 调 {@link #tickDrag(double, double)} 跟踪鼠标移动，更新 delta。</li>
 * <li>session 终结（打断/完成/换会话）时 {@link #resetForNewSession()} 清零偏移。</li>
 * </ul>
 */
public final class BotanyDragState {
    private static volatile int deltaX = 0;
    private static volatile int deltaY = 0;
    private static volatile int lastPanelX = Integer.MIN_VALUE;
    private static volatile int lastPanelY = Integer.MIN_VALUE;
    private static volatile int lastPanelWidth = 0;
    private static volatile int lastPanelHeight = 0;
    private static volatile boolean dragging = false;
    private static volatile double dragStartMouseX = 0;
    private static volatile double dragStartMouseY = 0;
    private static volatile int dragStartDeltaX = 0;
    private static volatile int dragStartDeltaY = 0;
    private static volatile String lastSessionId = "";

    private BotanyDragState() {
    }

    public static int deltaX() {
        return deltaX;
    }

    public static int deltaY() {
        return deltaY;
    }

    public static boolean isDragging() {
        return dragging;
    }

    public static void recordRenderedBounds(int x, int y, int width, int height) {
        lastPanelX = x;
        lastPanelY = y;
        lastPanelWidth = width;
        lastPanelHeight = height;
    }

    /**
     * 1: 按下 / 0: 松开。返回 true 表示事件已被浮窗消化（mixin 应取消传播）。
     */
    public static boolean onLeftButton(int action, double scaledMouseX, double scaledMouseY) {
        if (action == 1) {
            if (!isMouseOverPanel(scaledMouseX, scaledMouseY)) {
                return false;
            }
            dragging = true;
            dragStartMouseX = scaledMouseX;
            dragStartMouseY = scaledMouseY;
            dragStartDeltaX = deltaX;
            dragStartDeltaY = deltaY;
            return true;
        }
        if (action == 0 && dragging) {
            dragging = false;
            return true;
        }
        return false;
    }

    public static void tickDrag(double scaledMouseX, double scaledMouseY) {
        if (!dragging) {
            return;
        }
        int newDeltaX = dragStartDeltaX + (int) Math.round(scaledMouseX - dragStartMouseX);
        int newDeltaY = dragStartDeltaY + (int) Math.round(scaledMouseY - dragStartMouseY);
        deltaX = newDeltaX;
        deltaY = newDeltaY;
    }

    public static void maybeResetForSession(String sessionId) {
        String prev = lastSessionId;
        if (sessionId == null) {
            sessionId = "";
        }
        if (!sessionId.equals(prev)) {
            resetForNewSession();
            lastSessionId = sessionId;
        }
    }

    public static void resetForNewSession() {
        deltaX = 0;
        deltaY = 0;
        dragging = false;
        lastPanelX = Integer.MIN_VALUE;
        lastPanelY = Integer.MIN_VALUE;
    }

    public static void resetForTests() {
        resetForNewSession();
        lastSessionId = "";
    }

    private static boolean isMouseOverPanel(double mx, double my) {
        if (lastPanelX == Integer.MIN_VALUE) {
            return false;
        }
        return mx >= lastPanelX
            && mx < (double) lastPanelX + lastPanelWidth
            && my >= lastPanelY
            && my < (double) lastPanelY + lastPanelHeight;
    }
}
