package com.bong.client.combat;

/** Client → server parry reaction during an active {@link DefenseWindowState} (§11.3). */
public record JiemaiIntent() {
    public static final JiemaiIntent INSTANCE = new JiemaiIntent();
}
