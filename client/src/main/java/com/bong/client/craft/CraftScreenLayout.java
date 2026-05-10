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
    public static final int MATERIAL_SLOT_SIZE = 44;
    public static final int MATERIAL_COLUMNS = 3;
    public static final int MATERIAL_ROWS = 2;

    private CraftScreenLayout() {}

    public static boolean matchesAlchemyTabHeight() {
        return PANEL_H == ALCHEMY_PANEL_H;
    }
}
