package com.bong.client.network;

public final class AcknowledgingServerDataHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return ServerDataDispatch.handled(
            envelope.type(),
            "Routed payload type '" + envelope.type() + "' without client-side side effects"
        );
    }
}
