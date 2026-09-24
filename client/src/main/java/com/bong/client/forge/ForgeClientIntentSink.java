package com.bong.client.forge;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

/** 在应用组合根安装，内容组件不直接操作网络。 */
public final class ForgeClientIntentSink implements UiIntentSink<ForgeIntent> {
    @Override public UiIntentResult dispatch(ForgeIntent intent) {
        try {
            if (intent instanceof ForgeIntent.Start value) {
                ClientRequestSender.sendForgeStartSession(value.station(), value.blueprint(), value.materials());
            } else if (intent instanceof ForgeIntent.TurnPage value) {
                ClientRequestSender.sendForgeBlueprintTurnPage(value.delta());
            } else if (intent instanceof ForgeIntent.Material value) {
                ClientRequestSender.sendMaterialMove(value.blueprint(), value.station(), value.instanceId(), value.returning(), value.revision());
            } else if (intent instanceof ForgeIntent.Advance value) {
                ClientRequestSender.sendForgeStepAdvance(value.session());
            } else if (intent instanceof ForgeIntent.Hit value) {
                ClientRequestSender.sendForgeTemperingHit(value.session(), value.beat(), 1);
            } else if (intent instanceof ForgeIntent.Inscribe value) {
                ClientRequestSender.sendForgeInscriptionScroll(value.session(), value.inscription());
            } else if (intent instanceof ForgeIntent.Inject value) {
                ClientRequestSender.sendForgeConsecrationInject(value.session(), value.qi());
            } else return UiIntentResult.rejected("未知锻造操作");
            return UiIntentResult.accepted();
        } catch (RuntimeException failure) {
            return UiIntentResult.error("锻造请求发送失败：" + failure.getMessage());
        }
    }
}
