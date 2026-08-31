package com.bong.client.combat.screen;

import com.bong.client.combat.RepairClientIntentSink;
import com.bong.client.combat.RepairIntent;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/**
 * 养护界面的组合工厂；生产网络 sink 只在应用层组装，Screen 本身只接收抽象意图接口。
 */
public final class RepairScreenFactory {
    private final UiIntentSink<RepairIntent> intentSink;

    public RepairScreenFactory(UiIntentSink<RepairIntent> intentSink) {
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    public static RepairScreenFactory production() {
        return new RepairScreenFactory(RepairClientIntentSink.production());
    }

    public RepairScreen create(
        String weaponLabel,
        float durabilityNorm,
        long weaponInstanceId,
        int stationX,
        int stationY,
        int stationZ
    ) {
        return new RepairScreen(
            weaponLabel,
            durabilityNorm,
            weaponInstanceId,
            stationX,
            stationY,
            stationZ,
            intentSink
        );
    }
}
