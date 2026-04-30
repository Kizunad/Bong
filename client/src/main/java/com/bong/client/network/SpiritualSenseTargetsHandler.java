package com.bong.client.network;

import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.bong.client.visual.realm_vision.SpiritualSenseStateReducer;

public final class SpiritualSenseTargetsHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        PerceptionEdgeState next = SpiritualSenseStateReducer.apply(envelope.payload());
        PerceptionEdgeStateStore.replace(next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied spiritual_sense_targets (entries=" + next.entries().size()
                + ", generation=" + next.generation() + ")"
        );
    }
}
