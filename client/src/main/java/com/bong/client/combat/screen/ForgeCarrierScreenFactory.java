package com.bong.client.combat.screen;

import com.bong.client.combat.ForgeCarrierIntent;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 暗器注入屏组合工厂；生产网络 sink 在应用层组装。 */
public final class ForgeCarrierScreenFactory {
    private final UiIntentSink<ForgeCarrierIntent> intentSink;

    public ForgeCarrierScreenFactory(UiIntentSink<ForgeCarrierIntent> intentSink) {
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    public ForgeCarrierScreen create() {
        return create("dagger", 0.5);
    }

    public ForgeCarrierScreen create(String selectedItem, double qiInvest) {
        return new ForgeCarrierScreen(selectedItem, qiInvest, intentSink);
    }
}
