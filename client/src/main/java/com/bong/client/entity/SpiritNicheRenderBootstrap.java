package com.bong.client.entity;

public final class SpiritNicheRenderBootstrap {
    private SpiritNicheRenderBootstrap() {}

    public static void register() {
        BongEntityRenderBootstrap.register();
    }

    static BongEntityModelKind kindForTests() {
        return BongEntityModelKind.SPIRIT_NICHE;
    }
}
