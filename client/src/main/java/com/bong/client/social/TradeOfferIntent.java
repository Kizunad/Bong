package com.bong.client.social;

import com.bong.client.ui.contract.UiIntent;

/** 交易屏唯一的网络动作；选择本身是本地 UI 状态，不伪装为 server action。 */
public sealed interface TradeOfferIntent extends UiIntent permits TradeOfferIntent.Respond, TradeOfferIntent.Request {
    record Respond(String offerId, boolean accepted, Long requestedInstanceId) implements TradeOfferIntent {}

    record Request(String target, long offeredInstanceId) implements TradeOfferIntent {}
}
