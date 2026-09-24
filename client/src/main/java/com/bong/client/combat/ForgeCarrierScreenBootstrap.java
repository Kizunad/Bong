package com.bong.client.combat;

import com.bong.client.combat.screen.ForgeCarrierScreen;
import com.bong.client.combat.screen.ForgeCarrierScreenFactory;

/** 应用组合根：在真正打开暗器注入屏时组装生产网络 sink。 */
public final class ForgeCarrierScreenBootstrap {
    private ForgeCarrierScreenBootstrap() {
    }

    public static ForgeCarrierScreen create() {
        return new ForgeCarrierScreenFactory(ForgeCarrierClientIntentSink.production()).create();
    }
}
