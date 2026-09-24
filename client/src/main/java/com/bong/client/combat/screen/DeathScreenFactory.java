package com.bong.client.combat.screen;

import com.bong.client.combat.DeathIntent;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 死亡屏工厂；Screen 只接收抽象 typed sink，不组装网络实现。 */
public final class DeathScreenFactory {
    private final UiIntentSink<DeathIntent> intentSink;

    public DeathScreenFactory(UiIntentSink<DeathIntent> intentSink) {
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    public DeathScreen create(DeathStateStore.State state) {
        return new DeathScreen(state, intentSink);
    }
}
