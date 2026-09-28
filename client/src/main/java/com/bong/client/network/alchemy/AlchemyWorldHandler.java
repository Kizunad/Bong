package com.bong.client.network.alchemy;

import com.bong.client.alchemy.AlchemyWorldEffects;
import com.bong.client.alchemy.AlchemyWorldPayload;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import net.minecraft.client.MinecraftClient;

/** 世界表现走统一连接生命周期；旁观数据不写炉主的 AlchemySessionStore。 */
public final class AlchemyWorldHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        try {
            var payload = AlchemyWorldPayload.parse(envelope.payload().toString());
            var client = MinecraftClient.getInstance();
            if (client != null) AlchemyWorldEffects.accept(client, payload);
            return ServerDataDispatch.handled(envelope.type(), "Applied alchemy world effect");
        } catch (RuntimeException error) {
            return ServerDataDispatch.noOp(envelope.type(), "Invalid alchemy world effect: " + error.getMessage());
        }
    }
}
