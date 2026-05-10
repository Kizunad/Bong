package com.bong.client.entity;

public final class LingtianPlotBlock {
    private LingtianPlotBlock() {}

    public enum VisualState {
        WILD(0),
        TILLED(1),
        PLANTED(2),
        MATURE(3);

        private final int textureState;

        VisualState(int textureState) {
            this.textureState = textureState;
        }

        public int textureState() {
            return textureState;
        }
    }

    public static BongEntityModelKind modelKind() {
        return BongEntityModelKind.LINGTIAN_PLOT;
    }
}
