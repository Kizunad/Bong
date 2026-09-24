package com.bong.client.combat;

import com.bong.client.combat.screen.ZhenfaLayoutScreen;
import com.bong.client.combat.screen.ZhenfaLayoutScreenFactory;
import com.bong.client.network.ClientRequestProtocol;
import net.minecraft.util.math.BlockPos;

/** 阵法布置屏组合根；在生产入口组装网络 sink。 */
public final class ZhenfaLayoutScreenBootstrap {
    private ZhenfaLayoutScreenBootstrap() {
    }

    public static ZhenfaLayoutScreen create(
        BlockPos targetPos, ClientRequestProtocol.ZhenfaKind kind, long itemInstanceId,
        ClientRequestProtocol.ZhenfaTargetFace targetFace
    ) {
        return new ZhenfaLayoutScreenFactory(ZhenfaLayoutClientIntentSink.production()).create(
            targetPos, kind == null ? null : kind.wireName(), itemInstanceId,
            targetFace == null ? null : targetFace.wireName());
    }
}
