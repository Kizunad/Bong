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
    private final String texturePath;

    private HudRenderCommand(
        HudRenderLayer layer,
        Kind kind,
        String text,
        int x,
        int y,
        int width,
        int height,
        int color,
        String texturePath
    ) {
        this.layer = Objects.requireNonNull(layer, "layer");
        this.kind = Objects.requireNonNull(kind, "kind");
        this.text = text == null ? "" : text;
        this.x = x;
        this.y = y;
        this.width = width;
        this.height = height;
        this.color = color;
        this.texturePath = texturePath == null ? "" : texturePath;
    }

    public static HudRenderCommand text(HudRenderLayer layer, String text, int x, int y, int color) {
        return new HudRenderCommand(layer, Kind.TEXT, text, x, y, 0, 0, color, null);
    }

    public static HudRenderCommand screenTint(HudRenderLayer layer, int color) {
        return new HudRenderCommand(layer, Kind.SCREEN_TINT, "", 0, 0, 0, 0, color, null);
    }

    public static HudRenderCommand edgeVignette(HudRenderLayer layer, int color) {
        return new HudRenderCommand(layer, Kind.EDGE_VIGNETTE, "", 0, 0, 0, 0, color, null);
    }

    public static HudRenderCommand toast(HudRenderLayer layer) {
        return new HudRenderCommand(layer, Kind.TOAST, "", 0, 0, 0, 0, 0, null);
    }

    public static HudRenderCommand toast(HudRenderLayer layer, String text, int x, int y, int color) {
        return new HudRenderCommand(layer, Kind.TOAST, text, x, y, 0, 0, color, null);
    }

    public static HudRenderCommand rect(HudRenderLayer layer, int x, int y, int width, int height, int color) {
        return new HudRenderCommand(layer, Kind.RECT, "", x, y, width, height, color, null);
    }

    /**
     * plan §1.3 缩略图：drawTexture 支持。{@code texturePath} 例：{@code bong-client:textures/gui/botany/ci_she_hao.png}。
     * {@code color} 作为 tint（0xFFFFFFFF = 无 tint）。
     */
    public static HudRenderCommand texture(
        HudRenderLayer layer,
        String texturePath,
        int x,
        int y,
        int width,
        int height,
        int color
    ) {
        return new HudRenderCommand(layer, Kind.TEXTURED_RECT, "", x, y, width, height, color, texturePath);
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

    public boolean isToast() {
        return kind == Kind.TOAST;
    }

    public boolean isRect() {
        return kind == Kind.RECT;
    }

    public boolean isTexturedRect() {
        return kind == Kind.TEXTURED_RECT;
    }

    public String texturePath() {
        return texturePath;
    }

    public enum Kind {
        TEXT,
        SCREEN_TINT,
        EDGE_VIGNETTE,
        TOAST,
        RECT,
        TEXTURED_RECT
    }
}
