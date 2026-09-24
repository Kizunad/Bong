package com.bong.client.combat;

import com.bong.client.combat.screen.DeathScreen;
import com.bong.client.combat.screen.DeathScreenFactory;
import com.bong.client.combat.store.DeathStateStore;

/** 应用组合根：在真正打开死亡屏时组装生产网络 sink。 */
public final class DeathScreenBootstrap {
    private DeathScreenBootstrap() {
    }

    public static DeathScreen create(DeathStateStore.State state) {
        DeathScreenFactory factory = new DeathScreenFactory(DeathClientIntentSink.production());
        return factory.create(state);
    }
}
