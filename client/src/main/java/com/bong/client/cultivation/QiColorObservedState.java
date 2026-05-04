package com.bong.client.cultivation;

public record QiColorObservedState(
    String observer,
    String observed,
    ColorKind main,
    ColorKind secondary,
    boolean chaotic,
    boolean hunyuan,
    double realmDiff
) {
    public String displayText() {
        if (main == null) {
            return "";
        }
        StringBuilder sb = new StringBuilder("对方真元 ").append(main.label());
        if (secondary != null) {
            sb.append("/").append(secondary.label());
        }
        if (chaotic) {
            sb.append(" · 杂色");
        }
        if (hunyuan) {
            sb.append(" · 混元");
        }
        return sb.toString();
    }
}
