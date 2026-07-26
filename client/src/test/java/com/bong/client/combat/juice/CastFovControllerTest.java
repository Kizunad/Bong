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
import com.bong.client.network.VfxEventAnimationBridge;
import com.bong.client.network.VfxEventRouter;
import net.minecraft.util.Identifier;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.OptionalInt;
import java.util.Set;
import java.util.UUID;
import java.util.stream.Collectors;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * plan-fpv-cast-av-v1 P3 施法 juice 状态机饱和测试。
 *
 * <p>**从真实入口驱动**（§P3 硬约束「不直接调 controller 方法」）：本地预测走
 * {@link CastStateStore#beginSkillBarCast}（{@code SkillBarKeyRouter} 的真实入口，保住 SKILL_BAR
 * source），<b>accepted 武装 / release / interrupt / reject 全部走真实 {@link CastSyncHandler#handle}
 * 消费 server cast_sync</b>。断言只读外部可观察量 {@link CastFovController#fovDelta()}（加法 FOV
 * 偏移），每条路径终点断言归基准 0。
 *
 * <p><b>arming 需要「预测 + 权威」两步</b>（review finding B）：{@link #predict} 只是候选，
 * 它的作用是让 {@code CastSyncHandler.sourceFor} 把随后的权威回执认成 SKILL_BAR；真正武装
 * pending 的是 {@link #accept}（权威 {@code cast_sync{phase:casting}}）。只 predict 不 accept 的
 * 路径必须零 juice，见 {@link #localPredictionAloneNeverArmsSoTimerCompleteIsSilent()}。
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

    /** 动画事件驱动的招（heaven_gate）：令牌必须由它的权威 CASTING 武装。 */
    private static final int GATE_SLOT = 7;
    private static final String GATE_SKILL = CastFovController.HEAVEN_GATE_SKILL_ID;
    /** heaven_gate release FOV punch：peak +12°、时长 8 tick = 400ms。 */
    private static final double GATE_FOV_PEAK = 12.0;
    private static final int GATE_FOV_DURATION_MS = 8 * 50;
    /** heaven_gate release 抖动时长：24 tick = 1200ms。 */
    private static final int GATE_SHAKE_DURATION_MS = 24 * 50;

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
        SkillBarStore.updateSlot(GATE_SLOT, SkillBarEntry.skill(GATE_SKILL, "天门开阖", 4000, 0, ""));
        // 注册真实 cast 转换监听（生产由 bootstrap 挂；单测无 Fabric 事件环境，仅挂 listener）。
        // 必须是**带来源**的 transition listener——Consumer 版拿不到 Origin。
        CastStateStore.addTransitionListener(CastFovController::onCastState);
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

    /**
     * 本地预测开始技能栏施法（{@code SkillBarKeyRouter} 的真实入口）——<b>只建候选，不武装</b>。
     * 它唯一的作用是让随后的权威回执经 {@code CastSyncHandler.sourceFor} 认成 SKILL_BAR。
     */
    private void predict(int slot, long startedAt) {
        CastStateStore.beginSkillBarCast(slot, DURATION_MS, startedAt);
    }

    /** 服务端权威 {@code cast_sync{phase:casting}}（真实 wire 入口）——这才武装 pending。 */
    private void accept(int slot, long startedAt) {
        serverSync("casting", slot, startedAt, "none");
    }

    /** 预测 + 权威确认：生产上一次正常施法起手的完整两步。 */
    private void predictAndAccept(int slot, long startedAt) {
        predict(slot, startedAt);
        accept(slot, startedAt);
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

    /** 预测 + 权威确认一次 heaven_gate 施法 → 武装动画事件 juice 令牌。 */
    private void acceptGateCast(long startedAt) {
        predict(GATE_SLOT, startedAt);
        accept(GATE_SLOT, startedAt);
    }

    /** 服务端 PlayAnim 播出（真实入口 {@code VfxEventRouter} → 本方法）。 */
    private void chargeAnim() {
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_CHARGE);
    }

    private void releaseAnim() {
        CastFovController.onAnimPlayed(LOCAL_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_RELEASE);
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

    // ---- 参数表 pin（review finding E：测试侧字面量期望表，不与生产常量互相比对） ----

    /**
     * plan §P3 参数表的**测试侧字面量镜像**。刻意不引用 {@code CastJuiceProfiles.STRONG} 等
     * 生产常量——那样常量和实现一起漂移时测试仍会绿（旧版就是这么假绿的）。改这张表 = 改
     * plan 定稿参数，两边必须同时改，否则撞红。
     *
     * @param skillId 招式 id
     * @param shakeIntensity 抖动强度
     * @param shakeDurationTicks 抖动时长（tick）
     * @param fovPeakDegrees FOV 峰值加法偏移（度，0 = 无脉冲）
     * @param fovDurationTicks FOV 脉冲时长（tick，0 = 无脉冲）
     */
    private record ExpectedProfile(
        String skillId,
        float shakeIntensity,
        int shakeDurationTicks,
        float fovPeakDegrees,
        int fovDurationTicks
    ) {
    }

    /** plan §P3 表「强/中/弱」三档的字面量。 */
    private static final List<ExpectedProfile> EXPECTED_PROFILES = List.of(
        new ExpectedProfile("baomai.full_power_release", 1.2f, 20, 9.0f, 7),   // 强
        new ExpectedProfile("woliu.turbulence_burst", 0.85f, 18, 6.0f, 6),     // 中
        new ExpectedProfile("zhenmai.sever_chain", 0.85f, 14, 0f, 0),          // 中，无 FOV
        new ExpectedProfile("anqi.echo_fractal", 0.5f, 12, 0f, 0));            // 弱，无 FOV

    @Test
    void registryPinsEverySkillFieldByFieldWithLiteralExpectations() {
        for (ExpectedProfile want : EXPECTED_PROFILES) {
            CastJuiceProfile got = CastJuiceProfiles.get(want.skillId());
            assertNotNull(got, want.skillId() + " 必须登记在 CastJuiceProfiles（plan §P3 参数表）");
            assertEquals(want.skillId(), got.skillId(),
                "profile 的 skillId 必须与查表键一致，否则注册表自相矛盾");
            assertEquals(want.shakeIntensity(), got.shakeIntensity(), 1e-6f,
                want.skillId() + " 抖动强度应为 plan §P3 定稿的 " + want.shakeIntensity()
                    + "，实际 " + got.shakeIntensity() + "——数值漂移必须撞红");
            assertEquals(want.shakeDurationTicks(), got.shakeDurationTicks(),
                want.skillId() + " 抖动时长应为 " + want.shakeDurationTicks() + " tick，实际 "
                    + got.shakeDurationTicks());
            assertEquals(want.fovPeakDegrees(), got.fovPeakDegrees(), 1e-6f,
                want.skillId() + " FOV 峰值应为 +" + want.fovPeakDegrees() + "°，实际 "
                    + got.fovPeakDegrees());
            assertEquals(want.fovDurationTicks(), got.fovDurationTicks(),
                want.skillId() + " FOV 时长应为 " + want.fovDurationTicks() + " tick，实际 "
                    + got.fovDurationTicks());
            // 派生谓词与字面量自洽（hasShake/hasFovPulse 是消费侧真正读的东西）
            assertEquals(want.shakeIntensity() > 0f && want.shakeDurationTicks() > 0,
                got.hasShake(), want.skillId() + " hasShake 与字面量参数不自洽");
            assertEquals(want.fovPeakDegrees() != 0f && want.fovDurationTicks() > 0,
                got.hasFovPulse(), want.skillId() + " hasFovPulse 与字面量参数不自洽");
        }
    }

    @Test
    void registrySetIsExactlyTheExpectedSkills() {
        // 精确相等（不是 containsAll）：多登记一招 = 沉浸式极简被破，少一招 = 交付缺失。
        assertEquals(
            EXPECTED_PROFILES.stream().map(ExpectedProfile::skillId)
                .collect(Collectors.toCollection(LinkedHashSet::new)),
            new LinkedHashSet<>(CastJuiceProfiles.skillIds()),
            "CastState 驱动的注册集合必须与 plan §P3 参数表精确相等");

        // heaven_gate 已从 CastState 驱动移除（cast 条 4s 与引导窗 7s 错开）→ 改动画事件驱动。
        assertNull(CastJuiceProfiles.get(GATE_SKILL),
            "heaven_gate 不再走 CastState 驱动（改 onAnimPlayed 动画事件，见 animDriven* 测试）");
        // 沉浸式极简：普通招零 juice
        assertNull(CastJuiceProfiles.get("sword.cleave"), "普通招不登记 → 无 juice");
        assertNull(CastJuiceProfiles.get(null), "null skillId 不得炸，返回 null");
    }

    @Test
    void namedIntensityTiersPinTheLiteralStrongMediumWeak() {
        // 三档常量本身也 pin 字面量：它们是 plan §P3 表「强/中/弱」的唯一落地。
        assertEquals(1.2f, CastJuiceProfiles.STRONG, 1e-6f, "STRONG = 强 = 1.2");
        assertEquals(0.85f, CastJuiceProfiles.MEDIUM, 1e-6f, "MEDIUM = 中 = 0.85");
        assertEquals(0.5f, CastJuiceProfiles.WEAK, 1e-6f, "WEAK = 弱 = 0.5");
    }

    @Test
    void animDrivenJuiceParametersArePinnedFieldByField() {
        // 动画事件驱动的两段参数经只读 seam 逐字段 pin（含包络——它决定「渐强」还是「满幅」，
        // 改错了手感完全不同却不影响任何幅度断言）。
        assertEquals(Set.of(GATE_SKILL), CastFovController.animDrivenSkillIds(),
            "动画事件驱动的招式集合必须精确等于 {heaven_gate}");
        assertEquals(BongAnimations.SWORD_HEAVEN_GATE_CHARGE,
            CastFovController.chargeAnimId(GATE_SKILL), "charge 动画 id 契约");
        assertEquals(BongAnimations.SWORD_HEAVEN_GATE_RELEASE,
            CastFovController.releaseAnimId(GATE_SKILL), "release 动画 id 契约");

        CastFovController.ChargeShake charge = CastFovController.chargeAnimJuice(GATE_SKILL);
        assertNotNull(charge, "heaven_gate 必须有 charge 段 juice 参数");
        assertEquals(0.8f, charge.peakIntensity(), 1e-6f, "charge 峰值强度 0.8");
        assertEquals(60, charge.buildDurationTicks(),
            "charge 渐强时长 60t = sword_heaven_gate_charge 动画自身 endTick（P2 revert 后跟随值）");
        assertEquals(CameraShakeController.Envelope.CRESCENDO, charge.envelope(),
            "charge 必须是 CRESCENDO 渐强，不是 SUSTAIN 满幅");

        CastFovController.ReleaseBurst release = CastFovController.releaseAnimJuice(GATE_SKILL);
        assertNotNull(release, "heaven_gate 必须有 release 段 juice 参数");
        assertEquals(1.5f, release.shakeIntensity(), 1e-6f, "release 抖动强度 1.5（比 STRONG 还强）");
        assertEquals(24, release.shakeDurationTicks(), "release 抖动时长 24t ≈1.2s");
        assertEquals(12.0f, release.fovPeakDegrees(), 1e-6f, "release FOV punch +12°");
        assertEquals(8, release.fovDurationTicks(), "release FOV 时长 8t");
        assertEquals(CameraShakeController.Envelope.SUSTAIN, release.envelope(),
            "release 必须是 SUSTAIN 满幅撑住，不是 CRESCENDO/DECAY");

        assertNull(CastFovController.chargeAnimJuice("baomai.full_power_release"),
            "非动画事件驱动的招不得有动画段参数");
        assertNull(CastFovController.releaseAnimJuice(null), "null skillId 不得炸");
    }

    // ---- 生产接线（防孤岛：bootstrap 漏挂一行，整条 CastState 驱动会静默失效） ----

    @Test
    void bootstrapRegistersCastStateListenerSoProductionPathIsNotAnIsland() {
        // 其余用例都是 setUp 手工挂 listener（单测无 Fabric 事件环境），于是 bootstrap() 本身
        // 零覆盖——把 CastStateStore.addTransitionListener 那行删掉，整条 CastState 驱动 juice
        // 会静默变成孤岛而测试全绿。这里清空 listener 后走**真正的 bootstrap()**，再用真实入口驱动。
        CastStateStore.resetForTests();   // 清掉 setUp 手工挂的 listener
        CastFovController.bootstrap();    // 生产接线（幂等，AtomicBoolean 一次性）

        predictAndAccept(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6,
            "bootstrap() 必须把 juice 挂上 CastStateStore，否则生产里根本收不到 cast 转换");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("经 bootstrap 接线的脉冲同样 decay 回基准");
    }

    // ---- accepted 门控：本地预测不得武装（review finding B） ----

    @Test
    void localPredictionAloneNeverArmsSoTimerCompleteIsSilent() {
        // 生产真实序列：按键 → SkillBarKeyRouter.beginSkillBarCast（乐观预测）→ 服务端**从未**
        // 回 casting（被拒 / 丢包 / 该招服务端压根不发权威 CASTING）→ CastStateStore.tick 按
        // 本地计时把它推到 COMPLETE。这条路径必须零 juice：预测不是 accepted。
        predict(HEAVY_SLOT, START);
        CastStateStore.tick(START + DURATION_MS + 1);   // 本地计时到点（BongHud 每帧调的真实入口）
        assertEquals(CastState.Phase.COMPLETE, CastStateStore.snapshot().phase(),
            "前提校验：本地计时确实把预测推到了 COMPLETE（否则本用例假绿）");

        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("服务端从未确认 accepted → 本地计时到点也不许触发 FOV 脉冲");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "服务端从未确认 accepted → 抖动同样一点都不许有");
    }

    @Test
    void serverAcceptedThenLocalTimerCompleteFiresNormally() {
        // 上一条的反证：加上权威 casting 回执后，同一条本地计时 COMPLETE 必须正常触发——
        // 证明门控挡的是「缺确认」，不是把整条路径关死。
        predict(HEAVY_SLOT, START);
        accept(HEAVY_SLOT, START);
        CastStateStore.tick(START + DURATION_MS + 1);   // COMPLETE 只是**触发时刻**信号
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6,
            "已 accepted 的施法，本地计时到点的 COMPLETE 照常触发脉冲");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲 decay 完毕");
    }

    @Test
    void onlyAcceptedIdentityFiresWhenServerClockDiffersFromPrediction() {
        // 生产常态：预测的 startedAtMs 是**客户端**时钟，权威回执的是**服务端**时钟，二者不等。
        // 于是 identity 必须以权威回执为准：预测身份的 COMPLETE 不触发，权威身份的才触发。
        long serverStart = START + 37;
        predict(HEAVY_SLOT, START);
        accept(HEAVY_SLOT, serverStart);

        serverSync("complete", HEAVY_SLOT, START, "completed");   // 按**预测**身份来的 release
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("身份不是被 accepted 的那个 → 不触发");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不触发");

        serverSync("complete", HEAVY_SLOT, serverStart, "completed");  // 按**权威**身份来的 release
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "权威身份的 release 正常触发");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲 decay 完毕");
    }

    // ---- 状态机全路径（终点断言基准） ----

    @Test
    void normalReleaseFiresPulseThenDecaysToBaseline() {
        assertBaseline("施法前");
        predictAndAccept(HEAVY_SLOT, START);            // 武装
        assertBaseline("CASTING 中不触发（release 才触发）");
        serverSync("complete", HEAVY_SLOT, START, "completed");  // release

        // 脉冲中点（fire + 100ms = 半程）→ 接近峰值
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "脉冲半程 = sin(π/2)·peak = +9°");

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
        predictAndAccept(HEAVY_SLOT, START + 1);
        serverSync("interrupt", HEAVY_SLOT, START + 1, "reject_qi_insufficient");
        serverSync("complete", HEAVY_SLOT, START + 1, "completed");  // 迟到的 complete 不应复活
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("被拒后即便迟到 complete 也不触发");
    }

    @Test
    void interruptVoidsPendingBeforeRelease() {
        predictAndAccept(HEAVY_SLOT, START);
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
    void outOfOrderInterruptBlocksLaterAuthoritativeCastingOfSameCast() {
        // 上一条用例走纯 server sync，CASTING 那步经 sourceFor 退化成 QUICK_SLOT、本来就
        // 解析不出 profile——即便删掉作废记录它也照样绿。本例先用本地预测把快照恢复成
        // CASTING/SKILL_BAR（玩家重按键的真实序列），让随后那条**权威** CASTING 真能解析出
        // profile，于是「取消令牌记住被作废身份」成为唯一挡住 juice 的机制。
        serverSync("interrupt", HEAVY_SLOT, START, "interrupt_control");  // 打断先到

        predictAndAccept(HEAVY_SLOT, START);  // 同 identity 的权威 CASTING 后到（乱序/重传）
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("已作废身份即便后来收到权威 CASTING 也不得武装 → release 不触发");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不触发");

        // 反证：换一个未被作废的身份，同样的预测+权威+release 必须正常触发，
        // 证明上面的「不触发」来自作废记录，而不是这条路径整体就打不通。
        predictAndAccept(HEAVY_SLOT, START + 5000);
        serverSync("complete", HEAVY_SLOT, START + 5000, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "未被作废的身份走同一路径应正常触发");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲 decay 完毕");
    }

    @Test
    void lateInterruptOfSupersededCastDoesNotKillTheLiveOne() {
        // 回归（PR #1249 review finding 1）：A 起 → B 取代 A → 迟到的 INTERRUPT(A) → COMPLETE(B)
        // **仍然**触发脉冲。旧实现 voidPending 无条件清 pending，迟到的旧打断会误杀新施法；
        // 且旧 identity 含推断得来的 source，INTERRUPT 落地后 COMPLETE 会退化成 QUICK_SLOT 认不出自己。
        predictAndAccept(HEAVY_SLOT, START);                                   // 武装 A
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
        predictAndAccept(HEAVY_SLOT, START);
        serverSync("casting", HEAVY_SLOT, START + 5000, "none");      // B 取代 A
        serverSync("interrupt", HEAVY_SLOT, START + 5000, "interrupt_contam");  // 打断的正是 B
        serverSync("complete", HEAVY_SLOT, START + 5000, "completed");          // 迟到的 complete
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("打断命中自己身份 → pending 作废，后续 complete 不触发");
    }

    @Test
    void duplicateReleaseIsIdempotent() {
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);   // cast1 脉冲半程
        assertTrue(fov() > 0.0, "cast1 脉冲进行中");

        // 前一脉冲 decay 中开新 cast（不同 startedAtMs = 新身份）
        predictAndAccept(HEAVY_SLOT, START + 5000);
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
        predictAndAccept(LIGHT_SLOT, START);
        serverSync("complete", LIGHT_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("普通招（无 profile）release 不产生 FOV 脉冲");
    }

    @Test
    void multiplierZeroMidPulseCancelsBothChannelsImmediately() {
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START + 9000);
        serverSync("complete", HEAVY_SLOT, START + 9000, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "恢复倍率后的新 release 正常触发");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("新脉冲照常 decay 回基准");
    }

    @Test
    void multiplierZeroCancelsAnimDrivenChargeShake() {
        // 蓄力 CRESCENDO 长达 60t（3s）——「倍率只遮 FOV 读数」的实现会让它一路震到底。
        acceptGateCast(START);
        chargeAnim();
        advanceMs(2000);
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "蓄力震动进行中");

        JuiceConfig.setJuiceMultiplier(0.0f);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "倍率 0 → 蓄力渐强震动立即停");
        assertBaseline("蓄力段无 FOV 分量，照样在基准");
    }

    @Test
    void multiplierZeroSuppressesAnimDrivenRelease() {
        acceptGateCast(START);
        JuiceConfig.setJuiceMultiplier(0.0f);
        releaseAnim();
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
        predictAndAccept(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK * 0.5, fov(), 1e-6, "倍率 0.5 → 峰值减半");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("缩放脉冲同样 decay 回同一基准（缩放不改终点）");
    }

    @Test
    void bothChannelsBakeMultiplierAtFireTimeAndDoNotRetroScale() {
        // 两个通道对称：fire 时刻把倍率并入 shake 强度与 Pulse 峰值。
        JuiceConfig.setJuiceMultiplier(0.5f);
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);
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

        advanceMs(SHAKE_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("持续震动路径的 FOV 分量同样回基准");
    }

    @Test
    void deathTeardownResetsMidPulse() {
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);              // 武装 pending，尚未 release
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
        predictAndAccept(HEAVY_SLOT, START);
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
        predictAndAccept(HEAVY_SLOT, START);          // 武装 pending
        CastFovController.teardown();         // 施法中断线：清 pending
        serverSync("complete", HEAVY_SLOT, START, "completed");  // 迟到 complete
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("teardown 清 pending 后，迟到 release 不触发");
    }

    @Test
    void completeIsTerminalAndStaysAtBaselineAfterDecay() {
        predictAndAccept(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        CastState terminal = CastStateStore.snapshot();
        assertEquals(CastState.Phase.COMPLETE, terminal.phase(), "complete 终态");
        assertBaseline("脉冲结束后稳定在基准");
    }

    // ---- 跨 identity 乱序 / 跨 IDLE 幂等（review finding C：终态记录按身份、有界） ----

    @Test
    void idlePhaseDoesNotClearPendingBecauseItCarriesNoIdentity() {
        // IDLE 分支（枚举变体饱和）：CastState.idle() 是 slot=-1/startedAtMs=0 的无身份单例，
        // 无法归属到任何一次施法，故这里**不清场**——旧实现无条件 pending=null 才是
        // 「迟到 IDLE 误杀在飞施法」的根因（见 lateIdleDoesNotKillTheLiveCast）。
        predictAndAccept(HEAVY_SLOT, START);                   // 武装 pending
        serverSync("idle", HEAVY_SLOT, START, "none");         // 无身份 idle 回执
        assertEquals(CastState.Phase.IDLE, CastStateStore.snapshot().phase(), "回到 idle 相");

        serverSync("complete", HEAVY_SLOT, START, "completed");  // idle 之后 release 照常
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6,
            "IDLE 无身份、不作废任何东西 → 已 accepted 的 release 照常触发");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲 decay 完毕");
    }

    @Test
    void lateIdleDoesNotKillTheLiveCast() {
        // reviewer 逐条点名：CASTING(B) → 迟到 IDLE(A) → COMPLETE(B) 仍正常触发。
        // 旧实现 `case IDLE -> pending = null` 会把 B 的 pending 误杀，B 随后静默无 juice。
        predictAndAccept(HEAVY_SLOT, START);                          // A
        serverSync("casting", HEAVY_SLOT, START + 5000, "none");      // B 取代 A
        serverSync("idle", HEAVY_SLOT, START, "none");                // A 的迟到 idle

        serverSync("complete", HEAVY_SLOT, START + 5000, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "迟到的 IDLE 不得牵连在飞的 B");
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "B 的抖动同样照常");

        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("B 的脉冲 decay 完毕");
    }

    @Test
    void twoInterruptsThenLateCastingOfTheFirstDoesNotResurrectIt() {
        // reviewer 逐条点名：INTERRUPT(A) → INTERRUPT(B) → CASTING(A) 不得复活 A。
        // 旧实现只存一个 voidedId，第二条 INTERRUPT 会把 A 的作废记录覆盖掉。
        serverSync("interrupt", HEAVY_SLOT, START, "interrupt_control");          // 作废 A
        serverSync("interrupt", HEAVY_SLOT, START + 5000, "interrupt_movement");  // 作废 B（覆盖点）
        assertEquals(CastFovController.Terminal.VOIDED,
            CastFovController.terminalStateForTest(HEAVY_SLOT, START),
            "A 的作废记录不得被 B 的打断挤掉");

        predictAndAccept(HEAVY_SLOT, START);   // A 的权威 CASTING 迟到（预测先恢复 SKILL_BAR 快照）
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("A 已作废 → 迟到 CASTING 不得复活它");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不触发");
    }

    @Test
    void sameIdentityReplayedAcrossIdleFiresExactlyOnce() {
        // reviewer 逐条点名：COMPLETE(A) → IDLE → CASTING(A) → COMPLETE(A) 只触发一次。
        // 旧实现的 fired 标记住在 pending 里，被 IDLE 清掉后同身份可重建 pending 再触发第二次。
        predictAndAccept(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");   // 第一次触发
        assertEquals(CastFovController.Terminal.FIRED,
            CastFovController.terminalStateForTest(HEAVY_SLOT, START), "触发过的身份记 FIRED");

        // 让第一发的**两个通道**都自然走完（shake 20t=1000ms 比 FOV 7t 长）——这样第二次若
        // 真触发就一定看得见，而不是被第一发的残留掩盖成「反正非零」。
        advanceMs(SHAKE_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("第一发脉冲已走完");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "第一发抖动也已走完");

        serverSync("idle", HEAVY_SLOT, START, "none");            // 300ms 淡出回 idle
        predictAndAccept(HEAVY_SLOT, START);                     // 同身份 CASTING 重放
        serverSync("complete", HEAVY_SLOT, START, "completed");   // 同身份 COMPLETE 重放
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("同一 cast identity 跨 IDLE 重放不得二次触发");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不二次触发");
    }

    @Test
    void firedIdentityIsNotRewrittenByALateInterrupt() {
        // 终态先到者胜：已触发的身份不因迟到的打断被改写成 VOIDED（两者都挡后续武装，但
        // 语义不同——混淆会让「fire 后收打断」的记录失真）。
        predictAndAccept(HEAVY_SLOT, START);
        serverSync("complete", HEAVY_SLOT, START, "completed");
        serverSync("interrupt", HEAVY_SLOT, START, "interrupt_movement");
        assertEquals(CastFovController.Terminal.FIRED,
            CastFovController.terminalStateForTest(HEAVY_SLOT, START),
            "已 FIRED 的身份不得被迟到打断改写成 VOIDED");

        advanceMs(FOV_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("脉冲照常 decay 回基准");
    }

    @Test
    void terminalMemoryIsBoundedAndEvictsOldestIdentities() {
        // 有界是硬要求（无界 Map 会随会话时长单调增长）：灌满容量 + 1 条后，最早那条被淘汰。
        long oldest = START;
        for (int i = 0; i <= CastFovController.TERMINAL_MEMORY; i++) {
            serverSync("interrupt", HEAVY_SLOT, START + i * 1000L, "interrupt_movement");
        }
        assertNull(CastFovController.terminalStateForTest(HEAVY_SLOT, oldest),
            "超出 LRU 容量后最早的身份被淘汰（终态记录必须有界）");
        assertEquals(CastFovController.Terminal.VOIDED,
            CastFovController.terminalStateForTest(
                HEAVY_SLOT, START + CastFovController.TERMINAL_MEMORY * 1000L),
            "最近的身份仍在记录内");
        assertBaseline("整段只有打断，无任何 juice");
    }

    @Test
    void teardownVoidsInFlightIdentitySoLateCastingCannotRearmIt() {
        // reviewer 逐条点名：teardown → 旧 CASTING/COMPLETE 不触发。只清 pending 不够——
        // teardown 后迟到的同身份 CASTING 会重新武装，随后的 COMPLETE 就能在死亡后放出 juice。
        predictAndAccept(HEAVY_SLOT, START);   // 在飞 pending
        CastFovController.teardown();          // 死亡 / 断线 / 切世界
        assertEquals(CastFovController.Terminal.VOIDED,
            CastFovController.terminalStateForTest(HEAVY_SLOT, START),
            "teardown 必须把在飞身份整批记成 VOIDED");

        predictAndAccept(HEAVY_SLOT, START);   // 旧世界/旧会话迟到的 CASTING
        serverSync("complete", HEAVY_SLOT, START, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertBaseline("teardown 后旧身份的 CASTING/COMPLETE 都不得触发 juice");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不触发");

        // 反证：teardown 之后**全新**施法（新 startedAtMs）必须正常触发，不能把玩家永久关停。
        predictAndAccept(HEAVY_SLOT, START + 20_000);
        serverSync("complete", HEAVY_SLOT, START + 20_000, "completed");
        advanceMs(FOV_DURATION_MS / 2);
        assertEquals(HEAVY_FOV_PEAK, fov(), 1e-6, "teardown 后的新施法照常触发");
        advanceMs(FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("新脉冲 decay 完毕");
    }

    // ---- 动画事件驱动 juice（heaven_gate：charge 渐强 / release 最大+FOV，与劈下对齐）----
    //
    // 动画事件只是**触发时刻**信号：每条路径都必须先由权威 CASTING 武装令牌
    //（acceptGateCast），否则零 juice——见 animEventWithoutAcceptedCastFiresNothing 起的一组。

    @Test
    void animDrivenChargeStartsCrescendoThatGrows() {
        acceptGateCast(START);
        chargeAnim();
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
        acceptGateCast(START);
        releaseAnim();
        long fire = now[0];
        assertFalse(CameraShakeController.activeOffsets(fire).isZero(),
            "劈下即最大震动（SUSTAIN 起手满幅，≠ crescendo 从 0）");
        // FOV punch +12°/8t：半程（4t=200ms）达峰。
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertEquals(GATE_FOV_PEAK, fov(), 1e-6, "release FOV punch 半程达峰 +12°");
        advanceMs(GATE_FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("release FOV 脉冲结束回基准");
    }

    @Test
    void animDrivenReleaseSupersedesChargeCrescendo() {
        acceptGateCast(START);
        chargeAnim();
        advanceMs(2000);  // 蓄力渐强进行中（幅度已涨到一部分）
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "蓄力中有抖动");

        releaseAnim();
        long releaseFire = now[0];
        // 顶替生效：新 shake 起手 elapsed=0，SUSTAIN 满幅 → |yaw| ≈ 2.12°；
        // 若仍是 charge crescendo（elapsed=2000ms 于 60t 窗内只涨到 74%）则仅 ≈0.84°，据此区分。
        double mag = Math.abs(CameraShakeController.activeOffsets(releaseFire).yawDegrees());
        assertTrue(mag > 1.5, "release 的 SUSTAIN 最大震动顶替了 charge 的 CRESCENDO（|yaw|=" + mag + " > 1.5）");

        advanceMs(12 * 50);   // release FOV punch 8t 走完
        CastFovController.tick();
        assertBaseline("charge→release 顶替路径的终点同样是单一基准 FOV");
    }

    @Test
    void animDrivenJuiceOnlyForLocalPlayer() {
        acceptGateCast(START);   // 令牌已武装，唯一变量是动画目标是别人
        CastFovController.onAnimPlayed(OTHER_PLAYER, BongAnimations.SWORD_HEAVEN_GATE_RELEASE);
        advanceMs(4 * 50);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "非本地玩家的施法不震本地相机");
        assertBaseline("非本地玩家的施法不打本地 FOV");
    }

    @Test
    void animDrivenIgnoresUnregisteredAnim() {
        acceptGateCast(START);   // 令牌已武装，唯一变量是动画不属于这一招
        CastFovController.onAnimPlayed(LOCAL_PLAYER, new Identifier("bong", "sword_cleave"));
        advanceMs(4 * 50);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "未登记动画无震动 juice");
        assertBaseline("未登记动画无 FOV");
    }

    // ---- 动画事件的令牌门控（review finding A：动画播出 ≠ 已 accepted） ----

    @Test
    void animEventWithoutAcceptedCastFiresNothing() {
        // 服务端从未确认过任何 heaven_gate 施法（被拒 / 未 accepted / 纯杂散事件）：
        // 即便 bridge 真把动画播出来了，也不许放出相机反馈。
        chargeAnim();
        advanceMs(2000);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "无令牌 → 蓄力动画不得触发震动");

        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "无令牌 → 释放动画不得触发震动");
        assertBaseline("无令牌 → 释放动画不得触发 FOV punch");
    }

    @Test
    void localPredictionAloneDoesNotArmAnimToken() {
        // 与 CastState 路径同一条硬约束：按键预测不是 accepted，不发令牌。
        predict(GATE_SLOT, START);
        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "仅预测 → 无令牌 → 零震动");
        assertBaseline("仅预测 → 无令牌 → 零 FOV");
    }

    @Test
    void rejectedGateCastNeverArmsAnimToken() {
        // 施放前被拒（服务端 cast_sync{idle, reject_*} → CastSyncHandler 合成 INTERRUPT）：
        // 从未 accepted → 无令牌；之后哪怕来一条 release 动画也不许触发。
        predict(GATE_SLOT, START);
        serverSync("idle", GATE_SLOT, START, "reject_qi_insufficient");
        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "被拒后动画不触发震动");
        assertBaseline("被拒后动画不触发 FOV");
    }

    @Test
    void nonAnimDrivenAcceptedCastDoesNotAuthorizeGateAnims() {
        // 令牌只由**动画事件驱动的那一招**的权威 CASTING 发：baomai 被 accepted 不等于
        // heaven_gate 的动画可以挪用它。
        predictAndAccept(HEAVY_SLOT, START);
        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "别的招的 accepted 不得授权 heaven_gate 动画");
        assertBaseline("同上：FOV 也不许动");
    }

    @Test
    void duplicateReleaseAnimFiresOnlyOnce() {
        acceptGateCast(START);
        releaseAnim();
        // 让第一次的两个通道都走完，第二次若真触发一定看得见。
        advanceMs(GATE_SHAKE_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("第一次 release 的脉冲已走完");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "第一次 release 的抖动已走完");

        releaseAnim();   // 重复 PlayAnim（重传 / 服务端重复发）
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertBaseline("同一令牌的 release 只许消费一次");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "重复 release 不得重启抖动");
    }

    @Test
    void duplicateChargeAnimDoesNotRestartCrescendo() {
        acceptGateCast(START);
        chargeAnim();
        long fire = now[0];
        advanceMs(2000);
        double growing = Math.abs(CameraShakeController.activeOffsets(fire + 2000).yawDegrees());
        assertTrue(growing > 0.0, "蓄力渐强进行中");

        chargeAnim();   // 重复 charge PlayAnim
        assertEquals(growing,
            Math.abs(CameraShakeController.activeOffsets(now[0]).yawDegrees()), 1e-9,
            "重复 charge 不得把 CRESCENDO 重置回 0 起点（幅度应保持在原进度上）");

        advanceMs(60 * 50);
        CastFovController.tick();
        assertBaseline("蓄力段路径终点同样是基准 FOV");
    }

    @Test
    void chargeAnimAfterReleaseDoesNotReopenJuice() {
        acceptGateCast(START);
        releaseAnim();
        advanceMs(GATE_SHAKE_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("release 的脉冲已走完");

        chargeAnim();   // release 之后迟到的 charge（乱序）
        advanceMs(4 * 50);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "release 已消费令牌 → 迟到的 charge 不得重开蓄力震动");
        assertBaseline("同上：FOV 保持基准");
    }

    @Test
    void interruptVoidsAnimTokenSoLateReleaseAnimDoesNotFire() {
        acceptGateCast(START);
        serverSync("interrupt", GATE_SLOT, START, "interrupt_movement");  // 引导被打断

        releaseAnim();   // 打断后迟到的 release 动画
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "打断作废令牌 → 零震动");
        assertBaseline("打断作废令牌 → 零 FOV");
    }

    @Test
    void teardownVoidsAnimTokenSoLateReleaseAnimDoesNotFire() {
        acceptGateCast(START);
        chargeAnim();
        advanceMs(1000);
        assertFalse(CameraShakeController.activeOffsets(now[0]).isZero(), "蓄力震动进行中");

        CastFovController.teardown();   // 死亡 / 断线 / 切世界
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "teardown 停在播蓄力震动");

        releaseAnim();   // 旧世界/旧会话迟到的 release 动画
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "teardown 作废令牌 → 零震动");
        assertBaseline("teardown 作废令牌 → 零 FOV");

        // 反证：teardown 之后**全新** heaven_gate 施法照常拿到 juice。
        acceptGateCast(START + 20_000);
        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertEquals(GATE_FOV_PEAK, fov(), 1e-6, "teardown 后的新施法照常触发");
        advanceMs(GATE_FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("新脉冲 decay 完毕");
    }

    @Test
    void animTokenExpiresAfterTtlSoStrayLateAnimDoesNotFire() {
        // release 动画因丢包/异常始终没来时令牌不能一直挂着：超过 TTL 视为过期，
        // 一条几十秒后到达的杂散 PlayAnim 不得凭它放出 juice。
        acceptGateCast(START);
        advanceMs(CastFovController.ANIM_TOKEN_TTL_MS + 1);

        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "令牌已过期 → 零震动");
        assertBaseline("令牌已过期 → 零 FOV");
    }

    @Test
    void animTokenStillLiveJustBeforeTtlExpiry() {
        // TTL 边界另一侧（off-by-one）：heaven_gate 的 release 在引导第 140t（7s）才发，
        // 窗口内必须仍然有效。
        acceptGateCast(START);
        advanceMs(CastFovController.ANIM_TOKEN_TTL_MS);

        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertEquals(GATE_FOV_PEAK, fov(), 1e-6, "TTL 边界之内令牌仍有效");
        advanceMs(GATE_FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("脉冲 decay 完毕");
    }

    // ---- 路由接线：真实 VfxEventRouter → onAnimPlayed → 令牌门 → juice ----

    /** 只回定值的动画 bridge（单测无 PlayerAnimator 环境；本文件只关心 ok/decline 两分支）。 */
    private record StubAnimBridge(boolean played) implements VfxEventAnimationBridge {
        @Override
        public boolean playAnim(UUID target, Identifier animId, int priority, OptionalInt fadeIn) {
            return played;
        }

        @Override
        public boolean playAnimInline(
            UUID target, Identifier animId, String animJson, int priority, OptionalInt fadeIn) {
            return played;
        }

        @Override
        public boolean stopAnim(UUID target, Identifier animId, OptionalInt fadeOut) {
            return played;
        }
    }

    /** 走真实 {@code bong:vfx_event} play_anim 报文 + 真实 {@link VfxEventRouter}。 */
    private VfxEventRouter.RouteResult routePlayAnim(
        boolean bridgePlayed, UUID target, String animId
    ) {
        String json = "{\"v\":1,\"type\":\"play_anim\",\"target_player\":\"" + target
            + "\",\"anim_id\":\"" + animId + "\",\"priority\":1000,\"fade_in_ticks\":3}";
        return new VfxEventRouter(new StubAnimBridge(bridgePlayed))
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);
    }

    @Test
    void routerForwardsPlayAnimToJuiceOnlyWhenBridgeActuallyPlayed() {
        acceptGateCast(START);   // 令牌就位，唯一变量是 bridge 有没有真播出

        // bridge 拒付（玩家不在线 / 动画未注册 / 层没停下来）→ 画面上没有这段动画。
        VfxEventRouter.RouteResult miss =
            routePlayAnim(false, LOCAL_PLAYER, "bong:sword_heaven_gate_release");
        assertTrue(miss.isBridgeMiss(), "bridge 返回 false 应转 bridgeMiss");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "动画没播出来就震屏 = juice 与画面脱节，正是本条 juice 要避免的");
        assertBaseline("bridge 拒付时 FOV 也不许动");

        // bridge 播出 + 有令牌 → juice 触发。
        VfxEventRouter.RouteResult ok =
            routePlayAnim(true, LOCAL_PLAYER, "bong:sword_heaven_gate_release");
        assertTrue(ok.isHandled(), "bridge 播出应 handled: " + ok.logMessage());
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertEquals(GATE_FOV_PEAK, fov(), 1e-6,
            "真播出 + 已 accepted → juice 必须触发（否则 heaven_gate 劈下没有反馈）");

        advanceMs(GATE_FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("路由驱动的脉冲同样 decay 回基准");
    }

    @Test
    void routerDoesNotForwardJuiceForRemotePlayers() {
        acceptGateCast(START);   // 本地令牌就位，唯一变量是动画目标是别人
        VfxEventRouter.RouteResult ok =
            routePlayAnim(true, OTHER_PLAYER, "bong:sword_heaven_gate_release");
        assertTrue(ok.isHandled(), "远端玩家的动画照常派发给 bridge");

        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "远端玩家的施法动画不得触发本地 juice");
        assertBaseline("远端玩家的施法动画不得打本地 FOV");
    }

    @Test
    void routerDrivenAnimWithoutAcceptedCastFiresNothing() {
        // 路由链是通的（isHandled），但服务端从未确认过施法 → 令牌门挡住，零 juice。
        VfxEventRouter.RouteResult ok =
            routePlayAnim(true, LOCAL_PLAYER, "bong:sword_heaven_gate_release");
        assertTrue(ok.isHandled(), "前提校验：路由本身是通的（否则本用例假绿）");

        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(),
            "路由通但无 accepted 令牌 → 零震动");
        assertBaseline("路由通但无 accepted 令牌 → 零 FOV");
    }

    @Test
    void animReleaseRecordsFiredTerminalForItsIdentity() {
        // release 消费令牌时必须落 FIRED 终态：① 之后同身份的迟到打断不得把记录改写成 VOIDED；
        // ② TTL 过期清掉令牌后，一条同身份的迟到权威 CASTING 不得重开一枚 flags 全新的令牌
        //    让杂散 release 动画二次放 juice。
        acceptGateCast(START);
        releaseAnim();
        assertEquals(CastFovController.Terminal.FIRED,
            CastFovController.terminalStateForTest(GATE_SLOT, START),
            "动画路径的 release 同样要给该 identity 落 FIRED 终态");

        serverSync("interrupt", GATE_SLOT, START, "interrupt_movement");
        assertEquals(CastFovController.Terminal.FIRED,
            CastFovController.terminalStateForTest(GATE_SLOT, START),
            "已 FIRED 的动画路径身份不得被迟到打断改写成 VOIDED");

        // 两个通道走完 → 基准；随后同身份的迟到 CASTING + release 动画不得二次触发。
        advanceMs(GATE_SHAKE_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("第一次 release 走完");

        acceptGateCast(START);   // 同身份迟到 CASTING（终态已 FIRED → 不得重开令牌）
        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertBaseline("已 FIRED 的身份不得靠迟到 CASTING 重开令牌再放一次 juice");
        assertTrue(CameraShakeController.activeOffsets(now[0]).isZero(), "抖动同样不二次触发");
    }

    @Test
    void unrelatedCastDoesNotDropTheGateTokenBeforeItsReleaseAnim() {
        // heaven_gate 的 cast 条 4s 就走完，release 动画要到引导第 140t（7s）才发。中间这 3s
        // 玩家放别的招会带来一条权威 CASTING——那**不是**天门被取消，不得顶掉天门的令牌，
        // 否则劈下那一刻静默丢 juice。
        acceptGateCast(START);
        predictAndAccept(HEAVY_SLOT, START + 4000);   // 3s 窗口里插一发 baomai
        serverSync("complete", HEAVY_SLOT, START + 4000, "completed");
        advanceMs(SHAKE_DURATION_MS + 50);            // 让 baomai 的两个通道走完
        CastFovController.tick();
        assertBaseline("baomai 的脉冲已走完");

        releaseAnim();   // 天门劈下
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertEquals(GATE_FOV_PEAK, fov(), 1e-6,
            "无关招式的 accepted 不得顶掉天门令牌 —— 劈下那一刻仍须有 juice");
        advanceMs(GATE_FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("天门脉冲 decay 完毕");
    }

    @Test
    void newGateCastSupersedesOldTokenAndItsFiredFlags() {
        // 连续两次 heaven_gate：第二次的权威 CASTING 取代旧令牌，release 照常触发
        //（幂等是**按令牌**的一次性，不是「这辈子只许震一次」）。
        acceptGateCast(START);
        releaseAnim();
        advanceMs(GATE_SHAKE_DURATION_MS + 50);
        CastFovController.tick();
        assertBaseline("第一次 release 走完");

        acceptGateCast(START + 30_000);
        releaseAnim();
        advanceMs(GATE_FOV_DURATION_MS / 2);
        assertEquals(GATE_FOV_PEAK, fov(), 1e-6, "第二次施法的 release 照常触发");
        advanceMs(GATE_FOV_DURATION_MS);
        CastFovController.tick();
        assertBaseline("第二次脉冲 decay 完毕");
    }
}
