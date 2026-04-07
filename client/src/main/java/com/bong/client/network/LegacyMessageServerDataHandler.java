package com.bong.client.network;

public final class LegacyMessageServerDataHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return envelope.message()
            .map(message -> ServerDataDispatch.handledWithLegacyMessage(
                envelope.type(),
                message,
                "Routed legacy payload type '" + envelope.type() + "' with message dispatch"
            ))
            .orElseGet(() -> ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring legacy payload type '" + envelope.type() + "' because field 'message' is missing or not a string"
            ));
    }
}
