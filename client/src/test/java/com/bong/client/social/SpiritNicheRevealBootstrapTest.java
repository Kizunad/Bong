package com.bong.client.social;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-interaction-intent-cleanup-v1 P2 — 感灵龛「主动 M 键」行为锁定。
 *
 * <p>锁住的契约（{@link SpiritNicheRevealBootstrap#dispatchSense}）：
 * <ul>
 *   <li>看向方块按 M 键 → 同时发 gaze + mark_coordinate 两包，坐标都透传准星方块。</li>
 *   <li>空准星按 M 键 → 不发任何包（无被动凝视、无意外触发）。</li>
 *   <li>多次按键逐次成对单发，不丢不聚合。</li>
 * </ul>
 * 被动凝视轮询路径已删除，本测试不再覆盖（observeBlock 等已不存在）。
 */
class SpiritNicheRevealBootstrapTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
    }

    private void install() {
        sent.clear();
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void pressingMWhileLookingAtBlockSendsBothGazeAndMarkWithCoords() {
        install();
        BlockPos target = new BlockPos(11, 64, 10);

        SpiritNicheRevealBootstrap.dispatchSense(target);

        assertEquals(2, sent.size(),
            "一次 M 键应同时发 gaze + mark 两包，期望 2 实际 " + sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"spirit_niche_gaze\",\"v\":1,\"x\":11,\"y\":64,\"z\":10}",
            sent.get(0).body(),
            "第一包应为 gaze，坐标透传 (11,64,10)"
        );
        assertEquals(
            "{\"type\":\"spirit_niche_mark_coordinate\",\"v\":1,\"x\":11,\"y\":64,\"z\":10}",
            sent.get(1).body(),
            "第二包应为 mark_coordinate，坐标透传 (11,64,10)"
        );
    }

    @Test
    void pressingMWithEmptyCrosshairSendsNothing() {
        install();

        SpiritNicheRevealBootstrap.dispatchSense(null);

        assertTrue(sent.isEmpty(),
            "空准星按 M 键不应发任何包（无被动凝视/无意外触发），实际 " + sent.size() + " 包");
    }

    @Test
    void repeatedPressesEachSendGazeAndMarkPairsNoLoss() {
        install();
        BlockPos a = new BlockPos(1, 64, 1);
        BlockPos b = new BlockPos(2, 64, 2);

        SpiritNicheRevealBootstrap.dispatchSense(a);
        SpiritNicheRevealBootstrap.dispatchSense(b);

        assertEquals(4, sent.size(),
            "两次按键应发 4 包（每次 gaze+mark），实际 " + sent.size());
        assertEquals(
            "{\"type\":\"spirit_niche_gaze\",\"v\":1,\"x\":1,\"y\":64,\"z\":1}",
            sent.get(0).body());
        assertEquals(
            "{\"type\":\"spirit_niche_mark_coordinate\",\"v\":1,\"x\":1,\"y\":64,\"z\":1}",
            sent.get(1).body());
        assertEquals(
            "{\"type\":\"spirit_niche_gaze\",\"v\":1,\"x\":2,\"y\":64,\"z\":2}",
            sent.get(2).body());
        assertEquals(
            "{\"type\":\"spirit_niche_mark_coordinate\",\"v\":1,\"x\":2,\"y\":64,\"z\":2}",
            sent.get(3).body());
    }
}
