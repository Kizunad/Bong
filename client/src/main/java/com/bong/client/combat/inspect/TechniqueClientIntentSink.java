package com.bong.client.combat.inspect;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

/** 仅适配传输；配置结果仍由 SkillConfigStore 的服务端快照确认。 */
public final class TechniqueClientIntentSink implements UiIntentSink<TechniqueIntent> {
    @Override public UiIntentResult dispatch(TechniqueIntent intent) {
        if (intent == null) return UiIntentResult.rejected("功法操作为空");
        try {
            if (intent instanceof TechniqueIntent.BindChecked bind) {
                ClientRequestSender.sendTechniqueBind(bind.dash(), bind.slot(), bind.skillId(), bind.expectedBinding());
            } else if (intent instanceof TechniqueIntent.Clear clear) {
                ClientRequestSender.sendSkillBarBindClear(clear.slot());
            } else if (intent instanceof TechniqueIntent.Configure configure) {
                ClientRequestSender.sendSkillConfigIntent(configure.skillId(), configure.config());
            }
            return UiIntentResult.accepted();
        } catch (RuntimeException failure) {
            return UiIntentResult.error("功法请求发送失败：" + failure.getClass().getSimpleName());
        }
    }
}
