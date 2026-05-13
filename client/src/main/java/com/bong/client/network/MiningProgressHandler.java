package com.bong.client.network;

public final class MiningProgressHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return GatheringProgressPayloadReader.apply(
            envelope,
            "ore",
            "矿脉",
            "display_name",
            "mineral_id"
        );
    }
}
