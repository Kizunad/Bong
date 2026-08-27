package com.bong.client.ui.intent;

import com.bong.client.ui.contract.UiIntent;

/** 窄的类型化输入边界，不宣称服务端已接受。 */
public interface UiIntentSink<I extends UiIntent> {
    UiIntentResult dispatch(I intent);
}
