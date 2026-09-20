package com.bong.client.cultivation;

import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

/** 复用既有修炼请求；显示回路不参与服务器的真元计算与目标准入。 */
public final class CultivationClientIntentSink implements UiIntentSink<CultivationIntent> {
    @Override public UiIntentResult dispatch(CultivationIntent intent) {
        var body = MeridianStateStore.snapshot();
        String reason = blockedReason(body, intent);
        if (reason != null) return UiIntentResult.rejected(reason);
        switch (intent.action()) {
            case TARGET -> ClientRequestSender.sendSetMeridianTarget(ClientRequestProtocol.toMeridianId(intent.channel()));
            case BREAKTHROUGH -> ClientRequestSender.sendBreakthroughRequest();
            case DU_XU -> ClientRequestSender.sendStartDuXuRequest();
            case FORGE_RATE, FORGE_CAPACITY -> ClientRequestSender.sendForgeRequest(
                ClientRequestProtocol.toMeridianId(intent.channel()), intent.action() == CultivationIntent.Action.FORGE_RATE
                    ? ClientRequestProtocol.ForgeAxis.Rate : ClientRequestProtocol.ForgeAxis.Capacity);
        }
        return UiIntentResult.accepted();
    }
    public static String targetBlockReason(MeridianBody body, MeridianChannel channel) {
        if (body == null) return "经脉数据加载中";
        if (channel == null) return "先点击一条经脉";
        var state = body.channel(channel);
        if (state == null) return "当前构型没有这条经脉";
        if (!state.blocked()) return channel.displayName() + "已通，不需要再设为经脉目标";
        if (opened(body) == 0 && channel.family() == MeridianChannel.Family.EXTRAORDINARY) return "首脉需先走十二正经";
        return null;
    }
    public static boolean duXuEligible(MeridianBody body) { return body != null && "Spirit".equals(body.realm()) && opened(body) == 20; }
    public static long opened(MeridianBody body) { return body == null ? 0 : body.allChannels().values().stream().filter(ch -> !ch.blocked()).count(); }
    public static String blockedReason(MeridianBody body, CultivationIntent intent) {
        if (intent.action() == CultivationIntent.Action.TARGET) return targetBlockReason(body, intent.channel());
        if (body == null) return "经脉数据加载中";
        if (intent.action() == CultivationIntent.Action.DU_XU && !duXuEligible(body))
            return "Spirit".equals(body.realm()) ? "渡虚劫需打通全部经脉" : "渡虚劫需通灵境";
        if (intent.action() == CultivationIntent.Action.FORGE_RATE || intent.action() == CultivationIntent.Action.FORGE_CAPACITY) {
            var state = body.channel(intent.channel());
            if (state == null) return "先选中一条经脉";
            if (state.blocked()) return "经脉尚未贯通";
        }
        return null;
    }
}
