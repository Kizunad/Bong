package com.bong.client.craft;

/** plan-craft-ux-v1 — 手搓屏幕固定布局契约。 */
public final class CraftScreenLayout {
    public static final int PANEL_W = 640;
    public static final int PANEL_H = 340;
    public static final int ALCHEMY_PANEL_H = 340;
    public static final int HEADER_H = 20;
    public static final int BODY_H = 276;
    public static final int ACTION_BAR_H = 32;
    public static final int LEFT_W = 160;
    public static final int RIGHT_W = 200;
    // 中栏（材料网格）固定宽：PANEL_W(640) − 面板内边距 2×6 − LEFT_W − RIGHT_W − 列间距 2×4 = 260。
    // 必须固定而非 Sizing.fill(100)：owo fill(100) = 占父整宽（非剩余空间），放在 horizontalFlow
    // 中间且后有 fixed 兄弟会把 outputPreview(产物预览) 整个顶出右边界（见 CraftActionBar 同坑注释）。
    public static final int MID_W = PANEL_W - 2 * 6 - LEFT_W - RIGHT_W - 2 * 4;
    public static final int MATERIAL_SLOT_SIZE = 44;
    public static final int MATERIAL_COLUMNS = 3;
    public static final int MATERIAL_ROWS = 3;

    private CraftScreenLayout() {}

    public static boolean matchesAlchemyTabHeight() {
        return PANEL_H == ALCHEMY_PANEL_H;
    }
}
