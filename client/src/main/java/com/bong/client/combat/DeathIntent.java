package com.bong.client.combat;

import com.bong.client.ui.contract.UiIntent;

/** 死亡屏允许发送的类型化动作。 */
public sealed interface DeathIntent extends UiIntent
    permits DeathIntent.Reincarnate, DeathIntent.Terminate {

    record Reincarnate() implements DeathIntent {
    }

    record Terminate() implements DeathIntent {
    }
}
