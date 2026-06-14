package com.bong.client.mineral;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-interaction-intent-cleanup-v1 P0 — 感矿脉「主动键位触发」行为锁定。
 *
 * <p>锁住的契约：看向方块按键 → 恰好发一次探针（坐标透传准星方块）；空准星按键 → 不发包；
 * 多次按键 → 多次单发（不丢、不聚合、不刷屏）。
 */
class MineralSenseBootstrapTest {

    private final List<BlockPos> captured = new ArrayList<>();

    @AfterEach
    void tearDown() {
        MineralSenseBootstrap.resetForTests();
    }

    private void installCapture() {
        captured.clear();
        MineralSenseBootstrap.setProbeSenderForTests(captured::add);
    }

    @Test
    void pressingWhileLookingAtBlockSendsExactlyOneProbeWithBlockCoords() {
        installCapture();
        BlockPos target = new BlockPos(8, 32, 8);

        boolean sent = MineralSenseBootstrap.dispatchSenseForTests(target);

        assertTrue(sent, "看向方块按键应发出探针，期望 true 实际 false");
        assertEquals(1, captured.size(),
            "一次按键应恰好发一包，期望 1 实际 " + captured.size());
        assertEquals(target, captured.get(0),
            "探针坐标应等于准星方块坐标 " + target + "，实际 " + captured.get(0));
    }

    @Test
    void pressingWithEmptyCrosshairSendsNoProbe() {
        installCapture();

        boolean sent = MineralSenseBootstrap.dispatchSenseForTests(null);

        assertFalse(sent, "空准星按键不应发探针，期望 false 实际 true");
        assertTrue(captured.isEmpty(),
            "空准星不应发任何包，实际发了 " + captured.size() + " 包");
    }

    @Test
    void repeatedPressesEachSendOneProbeNoSpamNoLoss() {
        installCapture();
        BlockPos a = new BlockPos(1, 64, 1);
        BlockPos b = new BlockPos(2, 64, 2);
        BlockPos c = new BlockPos(3, 64, 3);

        MineralSenseBootstrap.dispatchSenseForTests(a);
        MineralSenseBootstrap.dispatchSenseForTests(b);
        MineralSenseBootstrap.dispatchSenseForTests(c);

        assertEquals(List.of(a, b, c), captured,
            "三次按键应逐次单发，按序 [a,b,c]，实际 " + captured);
    }

    @Test
    void interleavedEmptyCrosshairDoesNotEmitButRealTargetsDo() {
        installCapture();
        BlockPos a = new BlockPos(5, 5, 5);
        BlockPos b = new BlockPos(6, 6, 6);

        MineralSenseBootstrap.dispatchSenseForTests(a);
        MineralSenseBootstrap.dispatchSenseForTests(null);
        MineralSenseBootstrap.dispatchSenseForTests(b);

        assertEquals(List.of(a, b), captured,
            "中间一次空准星不应发包，只应留下 [a,b]，实际 " + captured);
    }
}
