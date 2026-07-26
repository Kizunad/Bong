package com.bong.client.combat.juice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.bong.client.animation.BongAnimations;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.network.CastSyncHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import net.minecraft.util.Identifier;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.UUID;
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
    // CastState 驱动的重型招（baomai 全力释放）。heaven_gate 已移到动画事件驱动，
    // 走 onAnimPlayed（cast 条 4s 与引导窗 7s 错开），见 animDriven* 测试。
    private static final String HEAVY_SKILL = "baomai.full_power_release";  // 强/20t 持续, FOV +9°/7t
    private static final String LIGHT_SKILL = "sword.cleave";            // 未登记 → 无 juice
    private static final int DURATION_MS = 2000;
    private static final long START = 1_700_000_000_000L;
    /** baomai FOV punch：peak +9°、时长 7 tick = 350ms。 */
    private static final double HEAVY_FOV_PEAK = 9.0;
    private static final int FOV_DURATION_MS = 7 * 50;
    /** baomai 持续抖动时长：20 tick = 1000ms（SUSTAIN 包络）。 */
    private static final int SHAKE_DURATION_MS = 20 * 50;
    /** 同相位采样点：tick 8 = 400ms（elapsedTick%4==0 = 满相位，落在 SUSTAIN 平台段内）。 */
    private static final int SUSTAIN_SAMPLE_MS = 8 * 50;

    /** 动画事件驱动 juice 的本地玩家 / 非本地玩家 UUID（固定，免 Math.random）。 */
    private static final UUID LOCAL_PLAYER = new UUID(0x1111L, 0x2222L);
    private static final UUID OTHER_PLAYER = new UUID(0x3333L, 0x4444L);

    private final long[] now = {10_000_000L};

    @BeforeEach
    void setUp() {
        CastStateStore.resetForTests();
        CastFovController.resetForTests();
        SkillBarStore.resetForTests();
        DeathStateStore.resetForTests();
        CameraShakeController.resetForTests();
        JuiceConfig.resetForTests();
        SkillBarStore.updateSlot(HEAVY_SLOT, SkillBarEntry.skill(HEAVY_SKILL, "全力", DURATION_MS, 0, ""));
        SkillBarStore.updateSlot(LIGHT_SLOT, SkillBarEntry.skill(LIGHT_SKILL, "竖劈", 1000, 0, ""));
        // 注册真实 cast 转换监听（生产由 bootstrap 挂；单测无 Fabric 事件环境，仅挂 listener）。
        CastStateStore.addListener(CastFovController::onCastState);
        CastFovController.setClockForTest(() -> now[0]);
        // 动画事件驱动 juice 的本地玩家判定 seam（免 MinecraftClient）。
        CastFovController.setLocalPlayerPredicateForTest(LOCAL_PLAYER::equals);
    }

    @AfterEach
    void tearDown() {
        CastStateStore.resetForTests();
        CastFovController.resetForTests();
        SkillBarStore.resetForTests();
        DeathStateStore.resetForTests();
        CameraShakeController.resetForTests();
        JuiceConfig.resetForTests();
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
    void registryPinsCastStateSkillsAndOmitsHeavenGateAndOthers() {
        CastJuiceProfile baomai = CastJuiceProfiles.get("baomai.full_power_release");
        assertNotNull(baomai, "baomai 全力释放必须登记（CastState 驱动）");
        assertEquals(CastJuiceProfiles.STRONG, baomai.shakeIntensity(), "baomai = 强抖动");
        assertEquals(20, baomai.shakeDurationTicks(), "持续震动 ≈1.0s，不是抖一下");
        assertEquals(9.0f, baomai.fovPeakDegrees(), 1e-6f);
        assertEquals(7, baomai.fovDurationTicks());

        assertNotNull(CastJuiceProfiles.get("woliu.turbulence_burst"));
        // sever_chain / echo_fractal 有 shake 无 FOV
        CastJuiceProfile sever = CastJuiceProfiles.get("zhenmai.sever_chain");
        assertNotNull(sever);
        assertTrue(sever.hasShake(), "sever_chain 有 shake");
        assertFalse(sever.hasFovPulse(), "sever_chain 无 FOV 脉冲（表中 —）");
        assertFalse(CastJuiceProfiles.get("anqi.echo_fractal").hasFovPulse(), "echo_fractal 无 FOV");

        // heaven_gate 已从 CastState 驱动移除（cast 条 4s 与引导窗 7s 错开）→ 改动画事件驱动。
        assertNull(CastJuiceProfiles.get("sword_path.heaven_gate"),
            "heaven_gate 不再走 CastState 驱动（改 onAnimPlayed 动画事件，见 animDriven* 测试）");
        // 沉浸式极简：普通招零 juice
        assertNull(CastJuiceProfiles.get("sword.cleave"), "普通招不登记 → 无 juice");
        assertNull(CastJuiceProfiles.get(null));
        assertEquals(4, CastJuiceProfiles.skillIds().size(), "CastState 驱动仅剩 4 个重型招");
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
    void lateInterruptOfSupersededCastDoesNotKillTheLiveOne() {
        // 回归（PR #1249 review finding 1）：A 起 → B 取代 A → 迟到的 INTERRUPT(A) → COMPLETE(B)
        // **仍然**触发脉冲。旧实现 voidPending 无条件清 pending，迟到的旧打断会误杀新施法；
        // 且旧 identity 含推断得来的 source，INTERRUPT 落地后 COMPLETE 会退化成 QUICK_SLOT 认不出自己。
        predict(HEAVY_SLOT, START);                                   // 武装 A
        serverSync("casting", HEAVY_SLOT, START + 5000, "none");      // B 取代 A（supersession）
        serverSync("interrupt", HEAVY_SLOT, START, "interrupt_movement");  // 迟到/重传的 INTERRUPT(A)

        serverSync("complete", HEAVY_SLOT, START + 5000, "completed");     // B 正常 release
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6,
            "迟到的 INTERRUPT(A) 只作废 A，不得牵连在飞的 B —— B 的 release 必须照常触发脉冲");
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "B 的抖动同样照常触发");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("B 的脉冲 decay 完毕");
    }

    @Test
    void lateInterruptStillVoidsItsOwnPending() {
        // 身份匹配时照旧作废（取消令牌语义不能因为「只清同身份」而失效）。
        predict(HEAVY_SLOT, START);
        serverSync("casting", HEAVY_SLOT, START + 5000, "none");      // B 取代 A
        serverSync("interrupt", HEAVY_SLOT, START + 5000, "interrupt_contam");  // 打断的正是 B
        serverSync("complete", HEAVY_SLOT, START + 5000, "completed");          // 迟到的 complete
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("打断命中自己身份 → pending 作废，后续 complete 不触发");
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
    void multiplierZeroMidPulseCancelsBothChannelsImmediately() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertTrue(fov() > 0.0, "脉冲进行中");
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(),
            "抖动进行中（SUSTAIN 20t 远未播完）");

        JuiceConfig.setJuiceMultiplier(0.0f);  // 进行中调 0

        // 通道 1：FOV 脉冲被真正清空（不是把读数遮成 0）
        assertBaseline("倍率 0 → FOV 立即复位（plan §P3「进行中把倍率调 0 立即复位」）");
        // 通道 2：抖动同步停下——这是「遮蔽读数」做不到的部分
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "倍率 0 → 已触发的抖动也必须停（不能只停 FOV 让相机继续震）");
    }

    @Test
    void multiplierRestoreDoesNotReviveCancelledPulse() {
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertTrue(fov() > 0.0, "脉冲进行中");

        JuiceConfig.setJuiceMultiplier(0.0f);
        assertBaseline("倍率 0 → 取消");

        // 恢复倍率时旧脉冲仍在其原时间窗内 —— 但它已被取消，不许诈尸。
        JuiceConfig.setJuiceMultiplier(1.0f);
        assertBaseline("恢复倍率只影响后续 release，不复活已取消的旧脉冲");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同理不复活");

        // 「只影响后续 release」的正面证明：下一发照常触发。
        predict(HEAVY_SLOT, START + 9000);
        serverSync("complete", HEAVY_SLOT, START + 9000, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "恢复倍率后的新 release 正常触发");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("新脉冲照常 decay 回基准");
    }

    @Test
    void multiplierZeroCancelsAnimDrivenChargeShake() {
        // 蓄力 CRESCENDO 长达 160t（8s）——「倍率只遮 FOV 读数」的实现会让它一路震到底。
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_CHARGE);
        advanceMs(2000);
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "蓄力震动进行中");

        JuiceConfig.setJuiceMultiplier(0.0f);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "倍率 0 → 蓄力渐强震动立即停");
        assertBaseline("蓄力段无 FOV 分量，照样在基准");
    }

    @Test
    void multiplierZeroSuppressesAnimDrivenRelease() {
        JuiceConfig.setJuiceMultiplier(0.0f);
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_RELEASE);
        advanceMs(4 * 50);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "倍率 0 → 动画事件震动不触发");
        assertBaseline("倍率 0 → 动画事件 FOV punch 不触发");

        // 关闭期间不留残留状态：恢复倍率后旧动画事件不会诈尸。
        JuiceConfig.setJuiceMultiplier(1.0f);
        assertBaseline("恢复倍率不复活关闭期间被抑制的动画 juice");
    }

    @Test
    void multiplierScalesPulseAmplitude() {
        JuiceConfig.setJuiceMultiplier(0.5f);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK * 0.5, fov(), 1e-6, "倍率 0.5 → 峰值减半");
    }

    @Test
    void bothChannelsBakeMultiplierAtFireTimeAndDoNotRetroScale() {
        // 两个通道对称：fire 时刻把倍率并入 shake 强度与 Pulse 峰值。
        JuiceConfig.setJuiceMultiplier(0.5f);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        long fireNow = now[0];
        CameraShakeController.Offsets scaled = CameraShakeController.activeOffsets(fireNow);
        assertFalse(scaled.isZero(), "倍率 0.5 → shake 以缩放后强度触发（非零抖动）");

        // 触发后把倍率调**大**（非 0，故不走取消路径）：在播 juice 不追溯放大。
        JuiceConfig.setJuiceMultiplier(1.0f);
        CameraShakeController.Offsets afterChange = CameraShakeController.activeOffsets(fireNow);
        assertEquals(scaled.yawDegrees(), afterChange.yawDegrees(), 1e-9f,
            "shake 用触发时刻的倍率，事后调大不追溯");
        assertEquals(scaled.pitchDegrees(), afterChange.pitchDegrees(), 1e-9f, "俯仰分量同理");

        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK * 0.5, fov(), 1e-6,
            "FOV 同样烘焙触发时刻倍率：调大后仍是 0.5 倍峰值（与 shake 对称）");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("烘焙脉冲照常 decay 回基准");
    }

    @Test
    void multiplierZeroAtFireSuppressesBothChannelsAndLeavesNoZombiePulse() {
        JuiceConfig.setJuiceMultiplier(0.0f);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "fire 时倍率 0 → 不触发 shake（juice 全局关闭）");
        assertBaseline("倍率 0 时 FOV 也无脉冲");

        // 关闭期间 fire 不许偷偷建一个「被遮蔽」的脉冲：恢复倍率后它必须不在。
        JuiceConfig.setJuiceMultiplier(1.0f);
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("恢复倍率不复活关闭期间被抑制的 release（无僵尸脉冲）");
    }

    @Test
    void castShakeIsSustainedNotSingleJerk() {
        // 施法 release 的抖动是持续震动（fire 走 SUSTAIN 包络）：同相位 tick 上，平台段内
        // 幅度与起始一致（满幅维持），而非线性「抖一下」衰减到近半。
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");  // fire
        long fireNow = now[0];

        // tick 0 与 tick 8（=SUSTAIN_SAMPLE_MS，落在 20t 的 40%<70% 平台段内）同为满相位
        //（elapsedTick % 4 == 0），故 SUSTAIN 下两点幅度应逐字段相等。
        CameraShakeController.Offsets atStart = CameraShakeController.activeOffsets(fireNow);
        CameraShakeController.Offsets atPlateau =
            CameraShakeController.activeOffsets(fireNow + SUSTAIN_SAMPLE_MS);
        assertFalse(atStart.isZero(), "起始有抖动");
        assertFalse(atPlateau.isZero(), "平台段仍在抖（持续，未提前结束）");
        assertEquals(atStart.yawDegrees(), atPlateau.yawDegrees(), 1e-5f,
            "持续型：平台段同相位幅度不衰减（若退化成线性衰减会减小）");
        assertEquals(atStart.pitchDegrees(), atPlateau.pitchDegrees(), 1e-5f, "俯仰分量同理");

        assertTrue(CameraShakeController.activeOffsets(fireNow + SHAKE_DURATION_MS + 50).isZero(),
            "抖动时长后自然归零");
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

    // ---- 切世界 teardown（plan §P3 teardown 契约三条之一：断线 / 切世界 / 玩家死亡）----

    @Test
    void worldSwitchUnloadingLocalPlayerResetsMidPulse() {
        // 切维度 / 换服：vanilla 不重建 ClientPlayNetworkHandler，DISCONNECT 不触发；
        // Fabric 在 onPlayerRespawn/onGameJoin/clearWorld 对旧世界全量 emit ENTITY_UNLOAD。
        CastFovController.setLocalPlayerEntityPredicateForTest(entity -> true);  // 卸载的是本地玩家实体
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertTrue(fov() > 0.0, "脉冲进行中");
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动进行中");

        CastFovController.onEntityUnload(null);  // 真实 ENTITY_UNLOAD 回调转发的方法
        assertBaseline("切世界 → FOV 立即复位基准");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "切世界 → 抖动也停");
    }

    @Test
    void worldSwitchClearsPendingSoLateCompleteDoesNotRefire() {
        CastFovController.setLocalPlayerEntityPredicateForTest(entity -> true);
        predict(HEAVY_SLOT, START);              // 武装 pending，尚未 release
        CastFovController.onEntityUnload(null);  // 切世界

        serverSync("complete", HEAVY_SLOT, START, "completed");  // 旧世界的迟到 complete
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("切世界清 pending 后，迟到的 complete 不得重新触发 juice");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不触发");
    }

    @Test
    void unloadingNonLocalEntityDoesNotTearDownJuice() {
        // 远端玩家 / 怪物离开视野也会 emit ENTITY_UNLOAD——不能把本地 juice 一起拆了。
        CastFovController.setLocalPlayerEntityPredicateForTest(entity -> false);
        predict(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        double before = fov();
        assertTrue(before > 0.0, "脉冲进行中");

        CastFovController.onEntityUnload(null);  // 非本地玩家实体卸载
        assertEquals(before, fov(), 1e-9, "非本地玩家实体卸载不得复位本地 juice");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲照常自然 decay 回基准");
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

    // ---- 动画事件驱动 juice（heaven_gate：charge 渐强 / release 最大+FOV，与劈下对齐）----

    @Test
    void animDrivenChargeStartsCrescendoThatGrows() {
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_CHARGE);
        long fire = now[0];
        // CRESCENDO 起手幅度 0（蓄力从 0 涨起），同相位 tick 4 → tick 40 幅度递增。
        assertTrue(CameraShakeController.activeOffsets(fire).isZero(), "蓄力起手幅度为 0（渐强从 0）");
        double early = Math.abs(CameraShakeController.activeOffsets(fire + 4 * 50).yawDegrees());
        double later = Math.abs(CameraShakeController.activeOffsets(fire + 40 * 50).yawDegrees());
        assertTrue(early > 0.0, "tick4 已有微弱抖动");
        assertTrue(later > early, "渐强：tick40 幅度 > tick4（同相位）——" + later + " > " + early);
        assertBaseline("蓄力段只震动、无 FOV 脉冲");
    }

    @Test
    void animDrivenReleaseFiresMaxShakeAndFovPunch() {
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_RELEASE);
        long fire = now[0];
        assertFalse(CameraShakeController.activeOffsets(fire).isZero(),
            "劈下即最大震动（SUSTAIN 起手满幅，≠ crescendo 从 0）");
        // FOV punch +12°/8t：半程（4t=200ms）达峰。
        advanceMs(4 * 50);
        assertEquals(12.0, fov(), 1e-6, "release FOV punch 半程达峰 +12°");
        advanceMs(8 * 50);
        CastFovController.tick();
        assertBaseline("release FOV 脉冲结束回基准");
    }

    @Test
    void animDrivenReleaseSupersedesChargeCrescendo() {
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_CHARGE);
        advanceMs(2000);  // 蓄力渐强进行中（幅度已涨到一部分）
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "蓄力中有抖动");

        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_RELEASE);
        long releaseFire = now[0];
        // 顶替生效：新 shake 起手 elapsed=0，SUSTAIN 满幅 → |yaw| 落在 max（≈2.1°）区间；
        // 若仍是 charge crescendo（elapsed=2000ms 才涨到 ~28%）则仅 ~0.3°，据此区分。
        double mag = Math.abs(CameraShakeController.activeOffsets(releaseFire).yawDegrees());
        assertTrue(mag > 1.5, "release 的 SUSTAIN 最大震动顶替了 charge 的 CRESCENDO（|yaw|=" + mag + " > 1.5）");
    }

    @Test
    void animDrivenJuiceOnlyForLocalPlayer() {
        CastFovController.onAnimPlayed(OTHER_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_RELEASE);
        advanceMs(4 * 50);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "非本地玩家的施法不震本地相机");
        assertBaseline("非本地玩家的施法不打本地 FOV");
    }

    @Test
    void animDrivenIgnoresUnregisteredAnim() {
        CastFovController.onAnimPlayed(LOCAL_PLAYER, new Identifier("bong", "sword_cleave"));
        advanceMs(4 * 50);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "未登记动画无震动 juice");
        assertBaseline("未登记动画无 FOV");
    }
}
