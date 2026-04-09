package com.bong.client.network;

@FunctionalInterface
public interface ServerDataHandler {
    ServerDataDispatch handle(ServerDataEnvelope envelope);
}
