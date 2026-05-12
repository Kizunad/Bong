package com.bong.client.network;

public final class LumberProgressHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return GatheringProgressPayloadReader.apply(
            envelope,
            "wood",
            "木材",
            "display_name",
            "tree_id",
            "detail"
        );
    }
}
