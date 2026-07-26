package com.bong.client.combat.juice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.network.CastSyncHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;

import java.nio.charset.StandardCharsets;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * plan-fpv-cast-av-v1 §P3「可及性」—— juice 倍率<b>配置持有者</b>饱和测试：默认值、钳位
 * （NaN / 负 / 0 / 上限）、档位循环全状态转换、以及「写配置即传播到控制器」的接线证明。
 */
class JuiceConfigTest {
    /** 注入时钟：脉冲在 t=0 的偏移天然是 0，必须推进到半程才能证明「取消前确实有值」。 */
    private final long[] now = {10_000_000L};

    @BeforeEach
    void setUp() {
        CastFovController.resetForTests();
        CameraShakeController.resetForTests();
        JuiceConfig.resetForTests();
        CastStateStore.resetForTests();
        SkillBarStore.resetForTests();
        CastFovController.setClockForTest(() -> now[0]);
        CastFovController.setLocalPlayerPredicateForTest(id -> true);
    }

    @AfterEach
    void tearDown() {
        CastFovController.resetForTests();
        CameraShakeController.resetForTests();
        JuiceConfig.resetForTests();
        CastStateStore.resetForTests();
        SkillBarStore.resetForTests();
    }

    // ---- 默认值 ----

    @Test
    void defaultsToOne() {
        assertEquals(1.0f, JuiceConfig.DEFAULT_JUICE_MULTIPLIER, 1e-9f, "plan §P3 明写「默认 1.0」");
        assertEquals(1.0f, JuiceConfig.juiceMultiplier(), 1e-9f, "启动即默认 1.0（无持久化层，见类注释）");
    }

    // ---- 钳位（边界 + 错误分支） ----

    @Test
    void clampsNanAndNegativeToZero() {
        assertEquals(0f, JuiceConfig.setJuiceMultiplier(Float.NaN), 1e-9f, "NaN → 0（关闭），不得放行成脏值");
        assertEquals(0f, JuiceConfig.juiceMultiplier(), 1e-9f);

        assertEquals(0f, JuiceConfig.setJuiceMultiplier(-1.0f), 1e-9f, "负倍率 → 0");
        assertEquals(0f, JuiceConfig.setJuiceMultiplier(-0.0001f), 1e-9f, "负的极小值同样 → 0");
        assertEquals(0f, JuiceConfig.setJuiceMultiplier(Float.NEGATIVE_INFINITY), 1e-9f, "负无穷 → 0");
    }

    @Test
    void clampsAboveMaxToMax() {
        assertEquals(JuiceConfig.MAX_JUICE_MULTIPLIER, JuiceConfig.setJuiceMultiplier(999f), 1e-9f,
            "超上限钳到 MAX，防手滑把画面震飞");
        assertEquals(JuiceConfig.MAX_JUICE_MULTIPLIER,
            JuiceConfig.setJuiceMultiplier(Float.POSITIVE_INFINITY), 1e-9f, "正无穷 → MAX");
        assertEquals(JuiceConfig.MAX_JUICE_MULTIPLIER,
            JuiceConfig.setJuiceMultiplier(JuiceConfig.MAX_JUICE_MULTIPLIER), 1e-9f, "恰好等于上限：原样保留");
    }

    @Test
    void acceptsZeroAndInRangeValues() {
        assertEquals(0f, JuiceConfig.setJuiceMultiplier(0f), 1e-9f, "0 = 关闭，是合法档位不是错误值");
        assertEquals(0.5f, JuiceConfig.setJuiceMultiplier(0.5f), 1e-9f);
        assertEquals(1.0f, JuiceConfig.setJuiceMultiplier(1.0f), 1e-9f);
    }

    @Test
    void resetToDefaultsRestoresOne() {
        JuiceConfig.setJuiceMultiplier(0f);
        JuiceConfig.resetToDefaults();
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f);
    }

    // ---- 档位循环（全状态转换） ----

    @Test
    void cycleWalksEveryLevelThenWrapsToZero() {
        float[] levels = JuiceConfig.cycleLevels();
        assertEquals(0f, levels[0], 1e-9f, "0 = 关闭必须是玩家可达的一档（plan §P3）");
        assertTrue(levels.length >= 2, "至少要有关闭与默认两档");

        // 从默认 1.0 起循环一整圈，应恰好走遍所有档位并回到起点。
        JuiceConfig.resetForTests();
        for (int i = 0; i < levels.length; i++) {
            JuiceConfig.cycleJuiceMultiplier();
        }
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f,
            "循环 levels.length 次回到起点（不重不漏）");

        // 逐档核对顺序：从最低档起依次严格递增，最后一档回绕到第一档。
        JuiceConfig.setJuiceMultiplier(levels[0]);
        for (int i = 1; i < levels.length; i++) {
            assertEquals(levels[i], JuiceConfig.cycleJuiceMultiplier(), 1e-9f,
                "第 " + i + " 次循环应到达档位 " + levels[i]);
        }
        assertEquals(levels[0], JuiceConfig.cycleJuiceMultiplier(), 1e-9f, "到顶回绕第一档");
    }

    @Test
    void cycleFromOffTableValueSnapsToNextHigherLevel() {
        JuiceConfig.setJuiceMultiplier(0.75f);   // 不在档位表上（API 直接设的任意值）
        assertEquals(1.0f, JuiceConfig.cycleJuiceMultiplier(), 1e-9f, "取严格大于当前值的第一档");

        JuiceConfig.setJuiceMultiplier(JuiceConfig.MAX_JUICE_MULTIPLIER);  // 高于所有档位
        assertEquals(JuiceConfig.cycleLevels()[0], JuiceConfig.cycleJuiceMultiplier(), 1e-9f,
            "高于所有档位时回绕到第一档（不卡死）");
    }

    @Test
    void cycleLevelsReturnsDefensiveCopy() {
        float[] first = JuiceConfig.cycleLevels();
        float[] second = JuiceConfig.cycleLevels();
        assertNotSame(first, second, "返回副本，外部改不动内部档位表");
        first[0] = 99f;
        assertEquals(0f, JuiceConfig.cycleLevels()[0], 1e-9f, "外部改副本不污染内部");
    }

    // ---- 接线证明：写配置 → 控制器真的收到（不是两套互不相干的静态） ----

    /**
     * 走完整真实链路打出一发在播 juice（FOV 脉冲 7t + 持续震动 20t）：技能栏预测 →
     * 服务端权威 {@code cast_sync{casting}} 武装 → {@code cast_sync{complete}} 触发。
     * 不直接调 {@code onAnimPlayed}/{@code fire}——两条路径现在都要求服务端已 accepted
     * （plan §P3 门控硬约束，review finding A/B）。
     */
    private void fireOneCast() {
        int slot = 3;
        long startedAt = 1_700_000_000_000L;
        SkillBarStore.updateSlot(slot,
            SkillBarEntry.skill("baomai.full_power_release", "全力", 2000, 0, ""));
        CastStateStore.addTransitionListener(CastFovController::onCastState);
        CastStateStore.beginSkillBarCast(slot, 2000, startedAt);
        castSync("casting", slot, startedAt, "none");
        castSync("complete", slot, startedAt, "completed");
    }

    /** server cast_sync 消费（真实 {@link CastSyncHandler} 入口）。 */
    private static void castSync(String phase, int slot, long startedAt, String outcome) {
        String json = "{\"v\":1,\"type\":\"cast_sync\",\"phase\":\"" + phase + "\",\"slot\":" + slot
            + ",\"duration_ms\":2000,\"started_at_ms\":" + startedAt
            + ",\"outcome\":\"" + outcome + "\"}";
        ServerPayloadParseResult parsed =
            ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), parsed.errorMessage());
        new CastSyncHandler().handle(parsed.envelope());
    }

    @Test
    void writingZeroPropagatesCancelToController() {
        fireOneCast();
        long fire = now[0];
        now[0] += 3 * 50;   // FOV punch 7t 的半程 → 达峰
        assertTrue(CastFovController.fovDelta() > 0.0, "取消前 FOV 脉冲确实有非零偏移");
        assertTrue(!CameraShakeController.activeOffsets(fire).isZero(), "取消前震动确实在播");

        JuiceConfig.setJuiceMultiplier(0f);   // 走配置入口，不直接调 controller

        assertEquals(0.0, CastFovController.fovDelta(), 1e-9,
            "配置写 0 必须传播到控制器并复位 FOV（否则配置就是个孤岛静态字段）");
        assertTrue(CameraShakeController.activeOffsets(fire).isZero(), "震动通道同样被传播取消");
    }

    @Test
    void writingNonZeroDoesNotCancelInFlightJuice() {
        fireOneCast();
        long fire = now[0];
        now[0] += 3 * 50;
        double peak = CastFovController.fovDelta();
        assertTrue(peak > 0.0, "脉冲进行中");

        JuiceConfig.setJuiceMultiplier(0.5f);  // 非 0 变更：只影响后续 release

        assertEquals(peak, CastFovController.fovDelta(), 1e-9,
            "调小（非 0）不取消也不追溯缩放在播 juice——取消只保留给「关闭」");
        assertTrue(!CameraShakeController.activeOffsets(fire).isZero(), "在播震动照常");
    }
}
