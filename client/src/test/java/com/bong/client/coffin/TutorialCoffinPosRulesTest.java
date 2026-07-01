package com.bong.client.coffin;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F9 跨层修复 — {@link TutorialCoffinPosRules#isSpawnTutorialCoffinPos} 饱和测试。
 *
 * <p>覆盖既定的三分支契约：收到坐标+匹配 / 收到坐标+不匹配 / 尚未收到坐标（fail-closed）。
 * 这是取代旧硬编码 {@code |x|<=8, y∈[60,90], |z|<=8} 判定盒的核心逻辑，必须锁死——
 * 任何回归（比如误用近似比较、误把"未收到"当"匹配"）都要立刻撞红。</p>
 */
class TutorialCoffinPosRulesTest {

    @Test
    void matchesWhenBroadcastPosEqualsCandidateExactly() {
        BlockPos broadcast = new BlockPos(12, 71, -33);
        BlockPos candidate = new BlockPos(12, 71, -33);

        assertTrue(TutorialCoffinPosRules.isSpawnTutorialCoffinPos(Optional.of(broadcast), candidate));
    }

    @Test
    void doesNotMatchWhenCandidateDiffersOnAnySingleAxis() {
        BlockPos broadcast = new BlockPos(0, 69, 0);

        assertFalse(TutorialCoffinPosRules.isSpawnTutorialCoffinPos(Optional.of(broadcast), new BlockPos(1, 69, 0)),
            "off-by-one on x must not match — this is exact-pos comparison, not the old +-8 box");
        assertFalse(TutorialCoffinPosRules.isSpawnTutorialCoffinPos(Optional.of(broadcast), new BlockPos(0, 70, 0)),
            "off-by-one on y must not match");
        assertFalse(TutorialCoffinPosRules.isSpawnTutorialCoffinPos(Optional.of(broadcast), new BlockPos(0, 69, -1)),
            "off-by-one on z must not match");
    }

    @Test
    void doesNotMatchWhenCandidateIsFarOutsideTheOldHardcodedBox() {
        // Regression case this fix targets: spawn relocated far away from the origin box
        // (|x|<=8, y in [60,90], |z|<=8) that used to be hardcoded on the client.
        BlockPos broadcast = new BlockPos(4200, 96, -1800);
        BlockPos candidate = new BlockPos(4200, 96, -1800);

        assertTrue(TutorialCoffinPosRules.isSpawnTutorialCoffinPos(Optional.of(broadcast), candidate),
            "coordinates far outside the old hardcoded box must still match when they are the "
                + "server-broadcast authoritative pos — this is exactly the regression F9 fixes");
    }

    @Test
    void failsClosedWhenNoBroadcastHasBeenReceivedYet() {
        // Even a coordinate that would have satisfied the *old* hardcoded box must not match
        // when the store is empty: fail-closed, never fall back to guessing.
        BlockPos candidateInsideOldBox = new BlockPos(0, 69, 0);

        assertFalse(TutorialCoffinPosRules.isSpawnTutorialCoffinPos(Optional.empty(), candidateInsideOldBox),
            "no broadcast received yet must fail-closed (false), never silently accept a guess");
    }
}
