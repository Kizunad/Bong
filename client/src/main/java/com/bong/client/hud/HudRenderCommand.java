package com.bong.client.hud;

import java.util.Objects;

public final class HudRenderCommand {
    private final HudRenderLayer layer;
    private final Kind kind;
    private final String text;
    private final int x;
    private final int y;
    private final int width;
    private final int height;
    private final int color;

    private HudRenderCommand(
        HudRenderLayer layer,
        Kind kind,
        String text,
        int x,
        int y,
        int width,
        int height,
        int color
    ) {
        this.layer = Objects.requireNonNull(layer, "layer");
        this.kind = Objects.requireNonNull(kind, "kind");
        this.text = text == null ? "" : text;
        this.x = x;
        this.y = y;
        this.width = width;
        this.height = height;
        this.color = color;
    }

    public static HudRenderCommand text(HudRenderLayer layer, String text, int x, int y, int color) {
        return new HudRenderCommand(layer, Kind.TEXT, text, x, y, 0, 0, color);
    }

    public static HudRenderCommand screenTint(HudRenderLayer layer, int color) {
        return new HudRenderCommand(layer, Kind.SCREEN_TINT, "", 0, 0, 0, 0, color);
    }

    public static HudRenderCommand edgeVignette(HudRenderLayer layer, int color) {
        return new HudRenderCommand(layer, Kind.EDGE_VIGNETTE, "", 0, 0, 0, 0, color);
    }

    public static HudRenderCommand edgeInkWash(HudRenderLayer layer, int color) {
        return new HudRenderCommand(layer, Kind.EDGE_INK_WASH, "", 0, 0, 0, 0, color);
    }

    public static HudRenderCommand toast(HudRenderLayer layer) {
        return new HudRenderCommand(layer, Kind.TOAST, "", 0, 0, 0, 0, 0);
    }

    public static HudRenderCommand toast(HudRenderLayer layer, String text, int x, int y, int color) {
        return new HudRenderCommand(layer, Kind.TOAST, text, x, y, 0, 0, color);
    }

    public static HudRenderCommand rect(HudRenderLayer layer, int x, int y, int width, int height, int color) {
        return new HudRenderCommand(layer, Kind.RECT, "", x, y, width, height, color);
    }

    /**
     * Draw an item PNG at {@code bong-client:textures/gui/items/{itemId}.png}
     * scaled into a {@code size×size} box with top-left at {@code (x, y)}.
     * Source PNG is assumed 128×128 (matches {@code GridSlotComponent}).
     */
    public static HudRenderCommand itemTexture(HudRenderLayer layer, String itemId, int x, int y, int size) {
        return new HudRenderCommand(layer, Kind.ITEM_TEXTURE, itemId == null ? "" : itemId, x, y, size, size, 0);
    }

    public HudRenderLayer layer() {
        return layer;
    }

    public Kind kind() {
        return kind;
    }

    public String text() {
        return text;
    }

    public int x() {
        return x;
    }

    public int y() {
        return y;
    }

    public int width() {
        return width;
    }

    public int height() {
        return height;
    }

    public int color() {
        return color;
    }

    public boolean isText() {
        return kind == Kind.TEXT;
    }

    public boolean isScreenTint() {
        return kind == Kind.SCREEN_TINT;
    }

    public boolean isEdgeVignette() {
        return kind == Kind.EDGE_VIGNETTE;
    }

    public boolean isEdgeInkWash() {
        return kind == Kind.EDGE_INK_WASH;
    }

    public boolean isToast() {
        return kind == Kind.TOAST;
    }

    public boolean isRect() {
        return kind == Kind.RECT;
    }

    public boolean isItemTexture() {
        return kind == Kind.ITEM_TEXTURE;
    }

    public enum Kind {
        TEXT,
        SCREEN_TINT,
        EDGE_VIGNETTE,
        EDGE_INK_WASH,
        TOAST,
        RECT,
        ITEM_TEXTURE
    }
}
