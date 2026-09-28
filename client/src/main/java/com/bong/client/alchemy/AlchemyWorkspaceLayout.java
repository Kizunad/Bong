package com.bong.client.alchemy;

import com.bong.client.ui.window.UiWindowManager.Rect;

/** 炉景与纸页的初始摆位。宽屏并排阅读，窄屏纸页让出工位标题，合卷后回到原工位。 */
public final class AlchemyWorkspaceLayout {
    private static final int PAGE_WIDTH = 260;
    private static final int MARGIN = 12;
    private static final int GAP = 12;
    private static final int NARROW_PAGE_TOP = 44;

    private AlchemyWorkspaceLayout() {}

    public static boolean canReadBesideFurnace(int width) {
        return width >= PAGE_WIDTH + 300 + GAP + MARGIN * 2;
    }

    public static Rect furnace(int width, int height, boolean reading) {
        int margin = margin(width, height);
        int left = reading && canReadBesideFurnace(width) ? margin + PAGE_WIDTH + GAP : margin;
        int available = Math.max(1, width - left - margin);
        int panelHeight = Math.max(1, Math.min(410, height - margin * 2));
        int panelWidth = Math.min(available, Math.max(300, (panelHeight - 62) * 3 / 2));
        if (!reading) left = (width - panelWidth) / 2;
        return new Rect(left, margin, panelWidth, panelHeight);
    }

    public static Rect notes(int width, int height) {
        boolean beside = canReadBesideFurnace(width);
        int margin = margin(width, height);
        // 窄屏先让出炼丹标题栏；缩短正文滚动区，不能靠顶出视口保留纸张高度。
        // 极小视口优先保证阅读面积，合卷后即可返回炉景。
        int top = beside || height < 224 ? margin : NARROW_PAGE_TOP;
        int pageHeight = Math.max(1, Math.min(380, height - top - margin));
        int pageWidth = Math.max(1, Math.min(PAGE_WIDTH, width - margin * 2));
        return new Rect(beside ? margin : (width - pageWidth) / 2, top, pageWidth, pageHeight);
    }

    private static int margin(int width, int height) {
        return Math.min(MARGIN, Math.max(0, (Math.min(width, height) - 100) / 2));
    }
}
