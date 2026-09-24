package com.bong.client.coffin;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * plan-coffin-tiers-v1 P3 — G 菜单 [回收] 生产链 Java 侧集成测试
 * （review finding [1] 修复：不得只靠 Python 场景的 server-endpoint 镜像冷注入）。
 *
 * <p>本测试驱动 <b>生产</b> 候选/派发/菜单/按钮/发送器链的<em>可 headless 测试</em>核心：
 * <ol>
 *   <li><b>派发派生</b>：{@link CoffinEnterIntentHandler#coffinBlockPos} 是<br>
 *       {@code dispatch} 对 marker 实体坐标取 floor（与 {@code Entity.getBlockPos} 同语义）
 *       的真实生产派生——marker 位于棺两格中心（server {@code coffin_marker_position}：
 *       lower+(1, 0, 0.5)），floor 后得到棺的 <b>upper</b> 格（lower.x+1, lower.y, lower.z）。</li>
 *   <li><b>菜单 → [回收] 按钮 → sender</b>：用上面派生的 upper 坐标构造真实
 *       {@link CoffinMenuScreen}，触发 XML [回收] 按钮绑定的 typed action，后者调用
 *       {@link ClientRequestSender#sendCoffinMenuReclaim}。通过
 *       {@code ClientRequestSender.setBackendForTests} 捕获 Java 侧产出的 wire payload。</li>
 * </ol>
 * 断言 payload 携带的正是 <b>upper</b> 坐标（而非 lower）——真实客户端 G→菜单→[回收]
 * 发出去的那份字节。candidate 门控（{@link CoffinEnterIntentHandler#isCoffinKind} 全表 +
 * 距离 6 格）由既有 {@link CoffinEnterIntentHandlerTest} 锁定；路由接线由
 * {@code DefaultInteractionHandlersTest} 锁定。本测试把这三级钉成一条
 * 「upper-coordinate payload」的确定性红线：任何把 [回收] 改发 lower/其它坐标或改走
 * 非生产 encoder 的实现，都在这里红。
 */
class CoffinGMenuProducerChainTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
    }

    private void installCapture() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void productionChainReclaimButtonEmitsUpperCoordinatePayload() {
        installCapture();
        // server coffin/mod.rs coffin_marker_position(lower)：lower+(1.0, 0.0, 0.5)。
        // lower=(3,65,7) → marker=(4.0, 65.0, 7.5)。
        double markerX = 4.0;
        double markerY = 65.0;
        double markerZ = 7.5;

        // 1) 生产派发派生：marker 实体坐标 floor → 棺 upper 格 (4,65,7)。
        BlockPos coffinPos = CoffinEnterIntentHandler.coffinBlockPos(markerX, markerY, markerZ);
        assertEquals(
            new BlockPos(4, 65, 7),
            coffinPos,
            "marker(4.0,65.0,7.5) 的 floor 派生必须得到棺 upper 格 (4,65,7)；" +
            "这是 dispatch 里对 marker 实体坐标的真实派生，不是 Python 镜像。期望 upper"
        );

        // 2) 触发 XML [回收] 按钮绑定的生产动作。
        CoffinMenuScreen screen = new CoffinMenuScreen(coffinPos);
        screen.onReclaim();

        // 3) 断言 Java 侧产出的 wire payload 使用 upper 坐标。
        assertEquals(1, sent.size(), "按下【回收】应恰好产生 1 条 client_request payload");
        assertEquals(
            new Identifier("bong", "client_request"),
            sent.get(0).channel(),
            "[回收] 必须走 bong:client_request 通道"
        );
        assertEquals(
            "{\"type\":\"coffin_menu_reclaim\",\"v\":1,\"x\":4,\"y\":65,\"z\":7}",
            sent.get(0).body(),
            "生产 [回收] 按钮必须发出 upper-coordinate payload（x=4,y=65,z=7）——" +
            "这正是真实 G→菜单→[回收] 链发出的字节；改用 lower (3,65,7) 或其他坐标的红点在 此"
        );
    }

    @Test
    void productionDerivationFloorsMarkerToUpperForOtherCoords() {
        // 另一组坐标，验证 floor 派生与 coffin_marker_position 的 center 语义一致：
        // lower=(0,64,0) → marker=(1.0, 64.0, 0.5) → upper=(1,64,0)。
        BlockPos upper = CoffinEnterIntentHandler.coffinBlockPos(1.0, 64.0, 0.5);
        assertEquals(new BlockPos(1, 64, 0), upper);
        // marker 恰落在整数边界（x=2.0, z=0.0 时 floor 稳定到 2/0，不因浮点滑到 1）。
        BlockPos boundary = CoffinEnterIntentHandler.coffinBlockPos(2.0, 64.0, 0.0);
        assertEquals(new BlockPos(2, 64, 0), boundary);
    }

    @Test
    void onReclaimSendsThroughProductionSender() {
        installCapture();
        CoffinMenuScreen screen = new CoffinMenuScreen(new BlockPos(-8, 65, 3));
        screen.onReclaim();
        assertEquals(1, sent.size(), "onReclaim()（[回收] 按钮 onClick 的调用目标）应发 1 条");
        assertEquals(
            "{\"type\":\"coffin_menu_reclaim\",\"v\":1,\"x\":-8,\"y\":65,\"z\":3}",
            sent.get(0).body()
        );
    }
}
