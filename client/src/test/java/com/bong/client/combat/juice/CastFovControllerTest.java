package com.bong.client.combat.juice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.network.CastSyncHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import java.nio.charset.StandardCharsets;
import java.util.List;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * plan-fpv-cast-av-v1 P3 施法 juice 状态机饱和测试。
 *
 * <p>**从真实入口驱动**（§P3 硬约束「不直接调 controller 方法」）：arming 走本地预测
 * {@link CastStateStore#beginSkillBarCast}（{@code SkillBarKeyRouter} 的真实入口，保住 SKILL_BAR
 * source），release/interrupt/reject 走真实 {@link CastSyncHandler#handle} 消费 server cast_sync。
 * 断言只读外部可观察量 {@link CastFovController#fovDelta()}（加法 FOV 偏移），每条路径终点断言归基准 0。
 */
class CastFovControllerTest {
    private static final int HEAVY_SLOT = 3;
    private static final int LIGHT_SLOT = 5;    // 非重型招（无 profile）
    private static final String HEAVY_SKILL = "sword_path.heaven_gate";  // 强/8t, FOV +6°/4t
    private static final String LIGHT_SKILL = "sword.cleave";            // 未登记 → 无 juice
    private static final int DURATION_MS = 2000;
    private static final long START = 1_700_000_000_000L;
    /** heaven_gate FOV 脉冲：peak +6°、时长 4 tick = 200ms。 */
    private static final double HEAVY_FOV_PEAK = 6.0;
    private static final int FOV_DURATION_MS = 4 * 50;

    private final long[] now = {10_000_000L};

    @BeforeEach
    void setUp() {
        CastStateStore.resetForTests();
        CastFovController.resetForTests();
        SkillBarStore.resetForTests();
        DeathStateStore.resetForTests();
        CameraShakeController.resetForTests();
        SkillBarStore.updateSlot(HEAVY_SLOT, SkillBarEntry.skill(HEAVY_SKILL, "开天", DURATION_MS, 0, ""));
        SkillBarStore.updateSlot(LIGHT_SLOT, SkillBarEntry.skill(LIGHT_SKILL, "竖劈", 1000, 0, ""));
        // 注册真实 cast 转换监听（生产由 bootstrap 挂；单测无 Fabric 事件环境，仅挂 listener）。
        CastStateStore.addListener(CastFovController::onCastState);
        CastFovController.setClockForTest(() -> now[0]);
    }

    @AfterEach
    void tearDown() {
        CastStateStore.resetForTests();
        CastFovController.resetForTests();
        SkillBarStore.resetForTests();
        DeathStateStore.resetForTests();
        CameraShakeController.resetForTests();
    }

    // ---- 驱动辅助（真实入口） ----

    /** 本地预测开始技能栏施法（SkillBarKeyRouter 的真实入口）——武装 pending。 */
    private void predict(int slot, long startedAt) {
        CastStateStore.beginSkillBarCast(slot, DURATION_MS, startedAt);
    }

    /** server cast_sync 消费（真实 CastSyncHandler 入口）。 */
    private void serverSync(String phase, int slot, long startedAt, String outcome) {
        String json = "{\"v\":1,\"type\":\"cast_sync\",\"phase\":\"" + phase + "\",\"slot\":" + slot
            + ",\"duration_ms\":" + DURATION_MS + ",\"started_at_ms\":" + startedAt
            + ",\"outcome\":\"" + outcome + "\"}";
        ServerPayloadParseResult parsed =
            ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), parsed.errorMessage());
        new CastSyncHandler().handle(parsed.envelope());
    }

    private void advanceMs(long ms) {
        now[0] += ms;
    }

    private double fov() {
        return CastFovController.fovDelta();
    }

    private void assertBaseline(String why) {
        assertEquals(0.0, fov(), 1e-9, why + "：FOV 加法偏移必须回到基准 0");
    }

    // ---- profile 注册表 pin ----

    @Test
    void registryPinsHeavySkillProfilesAndOmitsOthers() {
        CastJuiceProfile gate = CastJuiceProfiles.get("sword_path.heaven_gate");
        assertNotNull(gate, "heaven_gate 必须登记");
        assertEquals(CastJuiceProfiles.STRONG, gate.shakeIntensity(), "heaven_gate = 强抖动");
        assertEquals(8, gate.shakeDurationTicks());
        assertEquals(6.0f, gate.fovPeakDegrees(), 1e-6f);
        assertEquals(4, gate.fovDurationTicks());

        assertNotNull(CastJuiceProfiles.get("baomai.full_power_release"));
        assertNotNull(CastJuiceProfiles.get("woliu.turbulence_burst"));
        // sever_chain / echo_fractal 有 shake 无 FOV
        CastJuiceProfile sever = CastJuiceProfiles.get("zhenmai.sever_chain");
        assertNotNull(sever);
        assertTrue(sever.hasShake(), "sever_chain 有 shake");
        assertFalse(sever.hasFovPulse(), "sever_chain 无 FOV 脉冲（表中 —）");
        assertFalse(CastJuiceProfiles.get("anqi.echo_fractal").hasFovPulse(), "echo_fractal 无 FOV");

        // 沉浸式极简：普通招零 juice
        assertNull(CastJuiceProfiles.get("sword.cleave"), "普通招不登记 → 无 juice");
        assertNull(CastJuiceProfiles.get(null));
        assertEquals(5, CastJuiceProfiles.skillIds().size(), "当前仅 5 个重型招登记");
    }

    // ---- 状态机全路径（终点断言基准） ----

    @Test
    void normalReleaseFiresPulseThenDecaysToBaseline() {
        assertBaseline("施法前");
        predict(HEAVY_SLOT, START);            // 武装
        assertBaseline("CASTING 中不触发（release 才触发）");
        serverSync("complete", HEAVY_SLOT, START, "completed");  // release

        // 脉冲中点（fire + 100ms = 半程）→ 接近峰值
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "脉冲半程 = sin(π/2)·peak = +6°");

        // 脉冲结束后自然回基准
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲 decay 完毕");
    }

    @Test
    void serverRejectNeverFires() {
        // 施放前拒绝：从未收到 CASTING → 无 pending 可触发
        serverSync("idle", HEAVY_SLOT, START, "reject_qi_insufficient");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("施放前拒绝不触发 juice");

        // 预测后被拒（打断回执作废 pending）
        predict(HEAVY_SLOT, START + 1);
        serverSync("interrupt", HEAVY_SLOT, START + 1, "reject_qi_insufficient");
        serverSync("complete", HEAVY_SLOT, START + 1, "completed");  // 迟到的 complete 不应复活
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("被拒后即便迟到 complete 也不触发");
    }

    @Test
    void interruptVoidsPendingBeforeRelease() {
        predict(HEAVY_SLOT, START);
        serverSync("interrupt", HEAVY_SLOT, START, "interrupt_contam");  // 施法中打断
        serverSync("complete", HEAVY_SLOT, START, "completed");         // 打断后的迟到 complete
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("打断作废 pending，后续 complete 不触发");
    }

    @Test
    void outOfOrderInterruptBeforeStartedStaysBaseline() {
        // 回执乱序：打断先于 casting 到达（纯 server sync，无本地预测）
        serverSync("interrupt", HEAVY_SLOT, START, "interrupt_control");
        serverSync("casting", HEAVY_SLOT, START, "none");
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("乱序打断早到 → 该 cast 全程不触发");
    }

    @Test
    void duplicateReleaseIsIdempotent() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        double afterFirst = fov();
        assertTrue(afterFirst > 0.0, "首次 release 触发脉冲");

        // 同 identity 重复 complete（重复/乱序回执）→ 不重启脉冲
        long peakBefore = now[0];
        serverSync("complete", HEAVY_SLOT, START, "completed");
        assertEquals(afterFirst, fov(), 1e-9, "重复 release 幂等：脉冲不重启（同一进度值）");
        assertEquals(peakBefore, now[0], "时钟未变，进度一致");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("幂等脉冲照常 decay 到基准");
    }

    @Test
    void consecutiveCastsSecondPulseSupersedesFirst() {
        // cast1 release
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);   // cast1 脉冲半程
        assertTrue(fov() > 0.0, "cast1 脉冲进行中");

        // 前一脉冲 decay 中开新 cast（不同 startedAtMs = 新身份）
        predict(HEAVY_SLOT, START + 5000);
        serverSync("complete", HEAVY_SLOT, START + 5000, "completed");
        // cast2 刚 fire（进度 0）→ 偏移应为 cast2 的 0，不是 cast1+cast2 叠加
        assertEquals(0.0, fov(), 1e-9, "重叠触发 = last-wins 单脉冲，非叠加（cast2 刚起进度 0）");

        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "cast2 脉冲半程达峰");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("cast2 decay 完毕");
    }

    @Test
    void nonHeavySkillProducesNoJuice() {
        predict(LIGHT_SLOT, START);
        serverSync("complete", LIGHT_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("普通招（无 profile）release 不产生 FOV 脉冲");
    }

    @Test
    void multiplierZeroMidPulseResetsImmediately() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertTrue(fov() > 0.0, "脉冲进行中");

        CastFovController.setJuiceMultiplier(0.0f);  // 进行中调 0
        assertBaseline("倍率 0 → 立即复位（不只影响后续脉冲）");

        // 恢复倍率后同一脉冲仍在其时间窗内 → 偏移随倍率回来（乘法即时）
        CastFovController.setJuiceMultiplier(1.0f);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "倍率恢复 → 偏移即时恢复");
    }

    @Test
    void multiplierScalesPulseAmplitude() {
        CastFovController.setJuiceMultiplier(0.5f);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK * 0.5, fov(), 1e-6, "倍率 0.5 → 峰值减半");
    }

    @Test
    void shakeBakesMultiplierAtFireTimeNotLiveRecompute() {
        // shake 分量：fire 时刻把倍率并入 CameraShakeController 强度（与 FOV 每帧乘算不对称，
        // 见 CastFovController.multiplier javadoc 的刻意取舍）。
        CastFovController.setJuiceMultiplier(0.5f);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        long fireNow = now[0];
        CameraShakeController.Offsets scaled = CameraShakeController.activeOffsets(fireNow);
        assertFalse(scaled.isZero(), "倍率 0.5 → shake 以缩放后强度触发（非零抖动）");

        // 触发后改倍率：在播 shake 不追溯缩放（FOV 才每帧乘算）。
        CastFovController.setJuiceMultiplier(0.0f);
        CameraShakeController.Offsets afterChange = CameraShakeController.activeOffsets(fireNow);
        assertEquals(scaled.yawDegrees(), afterChange.yawDegrees(), 1e-9f,
            "shake 用触发时刻的倍率，事后改倍率不追溯（与 FOV 每帧乘算的取舍差异）");
        assertEquals(scaled.pitchDegrees(), afterChange.pitchDegrees(), 1e-9f, "俯仰分量同理");
    }

    @Test
    void shakeSuppressedWhenMultiplierZeroAtFire() {
        CastFovController.setJuiceMultiplier(0.0f);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "fire 时倍率 0 → 强度 0 → 不触发 shake（juice 全局关闭）");
        assertBaseline("倍率 0 时 FOV 也无脉冲");
    }

    @Test
    void deathTeardownResetsMidPulse() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertTrue(fov() > 0.0, "脉冲进行中");

        // 施法中非受击死亡：server 静默不发 cast_sync（§8.1 #3 缺口）→ client 观测死亡即 teardown
        DeathStateStore.replace(new DeathStateStore.State(
            true, "tribulation", 0.5f, List.of(), 0L, false, false));
        CastFovController.tick();
        assertBaseline("本地玩家死亡 → 立即复位基准 + 清 pending");
    }

    @Test
    void disconnectTeardownResetsMidPulse() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertTrue(fov() > 0.0, "脉冲进行中");

        CastFovController.teardown();  // 断线/切世界的真实 teardown 入口（DISCONNECT 事件调此）
        assertBaseline("断线/切世界 → 立即复位");
        // 幂等
        CastFovController.teardown();
        assertBaseline("teardown 幂等");
    }

    @Test
    void teardownClearsPendingSoLaterReleaseDoesNotFire() {
        predict(HEAVY_SLOT, START);          // 武装 pending
        CastFovController.teardown();         // 施法中断线：清 pending
        serverSync("complete", HEAVY_SLOT, START, "completed");  // 迟到 complete
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("teardown 清 pending 后，迟到 release 不触发");
    }

    @Test
    void idlePhaseAfterCompleteReturnsBaseline() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        // 300ms 后 CastStateStore 自回 idle（真实 tick 路径）
        CastState terminal = CastStateStore.snapshot();
        assertEquals(CastState.Phase.COMPLETE, terminal.phase(), "complete 终态");
        assertBaseline("脉冲结束后稳定在基准");
    }
}
