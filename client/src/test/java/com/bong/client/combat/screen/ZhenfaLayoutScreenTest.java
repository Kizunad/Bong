package com.bong.client.combat.screen;

import com.bong.client.network.ClientRequestProtocol;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

final class ZhenfaLayoutScreenTest {

    @Test
    void defaultsClassicArrayToOriginTrapAndKeepsTrigger() {
        ZhenfaLayoutScreen screen = new ZhenfaLayoutScreen(null, null, 0L, null);

        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":0,\"y\":64,\"z\":0,\"kind\":\"trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.1,\"trigger\":\"proximity\"}",
            screen.encodePlacementRequestForTests()
        );
    }

    @Test
    void fixedTrapOmitsTriggerAndForwardsItemAndTargetFace() {
        ZhenfaLayoutScreen screen = new ZhenfaLayoutScreen(
            new BlockPos(11, 64, -3),
            ClientRequestProtocol.ZhenfaKind.BLAST_TRAP,
            9001L,
            ClientRequestProtocol.ZhenfaTargetFace.NORTH
        );

        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"blast_trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.1,\"item_instance_id\":9001,\"target_face\":\"north\"}",
            screen.encodePlacementRequestForTests()
        );
    }

    @Test
    void nonPositiveItemIdsAreOmittedForFixedTraps() {
        ZhenfaLayoutScreen zeroItem = new ZhenfaLayoutScreen(
            new BlockPos(1, 65, 2),
            ClientRequestProtocol.ZhenfaKind.SLOW_TRAP,
            0L,
            ClientRequestProtocol.ZhenfaTargetFace.TOP
        );
        ZhenfaLayoutScreen negativeItem = new ZhenfaLayoutScreen(
            new BlockPos(1, 65, 2),
            ClientRequestProtocol.ZhenfaKind.WARNING_TRAP,
            -1L,
            null
        );

        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":1,\"y\":65,\"z\":2,\"kind\":\"slow_trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.1,\"target_face\":\"top\"}",
            zeroItem.encodePlacementRequestForTests()
        );
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":1,\"y\":65,\"z\":2,\"kind\":\"warning_trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.1}",
            negativeItem.encodePlacementRequestForTests()
        );
    }
}
