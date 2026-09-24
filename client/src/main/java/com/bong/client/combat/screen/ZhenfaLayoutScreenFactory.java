package com.bong.client.combat.screen;

import com.bong.client.combat.ZhenfaLayoutIntent;
import com.bong.client.ui.intent.UiIntentSink;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/** 阵法布置屏工厂；网络 sink 在应用层注入。 */
public final class ZhenfaLayoutScreenFactory {
    private final UiIntentSink<ZhenfaLayoutIntent> intentSink;

    public ZhenfaLayoutScreenFactory(UiIntentSink<ZhenfaLayoutIntent> intentSink) {
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    public ZhenfaLayoutScreen create(BlockPos targetPos, String kind, long itemInstanceId, String targetFace) {
        return new ZhenfaLayoutScreen(targetPos, kind, itemInstanceId, targetFace, intentSink);
    }
}
