package com.bong.client.combat;

import com.bong.client.ui.contract.UiIntent;

import java.util.Objects;

/** 暗器注入界面允许发送的类型化动作。 */
public sealed interface ForgeCarrierIntent extends UiIntent
    permits ForgeCarrierIntent.Begin {

    /** 提交选定的暗器类型与真元注入比例。 */
    record Begin(String item, double qiInvest) implements ForgeCarrierIntent {
        public Begin {
            Objects.requireNonNull(item, "carrier item must not be null");
            if (item.isBlank()) {
                throw new IllegalArgumentException("carrier item must not be blank");
            }
            if (!Double.isFinite(qiInvest) || qiInvest < 0.0 || qiInvest > 1.0) {
                throw new IllegalArgumentException("carrier qi invest must be between 0 and 1");
            }
        }
    }
}
