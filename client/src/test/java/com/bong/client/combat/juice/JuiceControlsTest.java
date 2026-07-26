package com.bong.client.combat.juice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.network.CastSyncHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * plan-fpv-cast-av-v1 §P3「可及性」—— juice 倍率<b>运行时可调入口</b>（keybind）饱和测试。
 *
 * <p>驱动的是 {@code JuiceControls} 真实的按键消费逻辑（{@code END_CLIENT_TICK} 回调转发到
 * {@code consumeCyclePresses}），只把 {@code KeyBinding.wasPressed} 与动作栏回显抽成 seam
 * ——单测环境没有 GLFW / MinecraftClient。断言落在外部可观察量：配置值与回显文案。
 */
class JuiceControlsTest {
    private final List<String> feedback = new ArrayList<>();
    /** 注入时钟：脉冲在 t=0 偏移天然为 0，需推进到半程才能证明取消前真有值。 */
    private final long[] now = {10_000_000L};

    @BeforeEach
    void setUp() {
        JuiceConfig.resetForTests();
        CastFovController.resetForTests();
        CameraShakeController.resetForTests();
        JuiceControls.resetControlsForTests();
        CastStateStore.resetForTests();
        SkillBarStore.resetForTests();
        feedback.clear();
        CastFovController.setClockForTest(() -> now[0]);
        CastFovController.setLocalPlayerPredicateForTest(id -> true);
    }

    @AfterEach
    void tearDown() {
        JuiceConfig.resetForTests();
        CastFovController.resetForTests();
        CameraShakeController.resetForTests();
        JuiceControls.resetControlsForTests();
        CastStateStore.resetForTests();
        SkillBarStore.resetForTests();
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

    /** 按下 n 次的 wasPressed 队列（vanilla 语义：取到 false 为止）。 */
    private static java.util.function.BooleanSupplier presses(int n) {
        int[] left = {n};
        return () -> {
            if (left[0] <= 0) {
                return false;
            }
            left[0]--;
            return true;
        };
    }

    // ---- happy path ----

    @Test
    void singlePressCyclesToNextLevelAndReportsIt() {
        int consumed = JuiceControls.consumeCyclePresses(true, false, presses(1), feedback::add);
        assertEquals(1, consumed, "消费一次按键");
        assertEquals(1.5f, JuiceConfig.juiceMultiplier(), 1e-9f, "默认 1.0 → 下一档 1.5");
        assertEquals(List.of("施法震感：150%"), feedback, "每次切换都回显新档位");
    }

    @Test
    void pressCanReachOffWhichIsThePlanMandatedZero() {
        // plan §P3「0 = 关闭」必须玩家可达：从默认档连按直到关闭。
        int pressed = 0;
        while (JuiceConfig.juiceMultiplier() > 0f && pressed < 10) {
            JuiceControls.consumeCyclePresses(true, false, presses(1), feedback::add);
            pressed++;
        }
        assertEquals(0f, JuiceConfig.juiceMultiplier(), 1e-9f, "连按可达「关闭」档");
        assertTrue(pressed < 10, "档位表有限，不该按满 10 次还到不了关闭");
        assertEquals("施法震感：关闭", feedback.get(feedback.size() - 1),
            "关闭档明写「关闭」而不是 0%（0% 容易被当成显示 bug）");
    }

    // ---- 边界：多次按键 / 零次 ----

    @Test
    void consumesEveryQueuedPressInOneTick() {
        int consumed = JuiceControls.consumeCyclePresses(true, false, presses(3), feedback::add);
        assertEquals(3, consumed, "wasPressed 是队列语义，一 tick 内必须取干净（否则连按丢档）");
        assertEquals(3, feedback.size(), "每次切换各回显一次");
    }

    @Test
    void noPressLeavesConfigUntouched() {
        int consumed = JuiceControls.consumeCyclePresses(true, false, presses(0), feedback::add);
        assertEquals(0, consumed);
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f);
        assertTrue(feedback.isEmpty(), "没按键就不该回显");
    }

    // ---- 门控分支：无玩家 / GUI 打开 ----

    @Test
    void ignoresPressesWhenNoPlayer() {
        int consumed = JuiceControls.consumeCyclePresses(false, false, presses(2), feedback::add);
        assertEquals(0, consumed, "无玩家时不消费");
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f);
    }

    @Test
    void ignoresPressesWhileScreenOpen() {
        int consumed = JuiceControls.consumeCyclePresses(true, true, presses(2), feedback::add);
        assertEquals(0, consumed, "GUI 打开时不消费（在输入框里打字不该改配置）");
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f);
    }

    // ---- 错误分支：无回显 sink ----

    @Test
    void nullFeedbackSinkStillCycles() {
        int consumed = JuiceControls.consumeCyclePresses(true, false, presses(1), null);
        assertEquals(1, consumed);
        assertEquals(1.5f, JuiceConfig.juiceMultiplier(), 1e-9f, "回显缺席不得影响配置切换");
    }

    // ---- 回显文案 ----

    @Test
    void describeCoversOffAndScaledLevels() {
        assertEquals("施法震感：关闭", JuiceControls.describe(0f));
        assertEquals("施法震感：关闭", JuiceControls.describe(-1f), "负值同样按关闭显示");
        assertEquals("施法震感：50%", JuiceControls.describe(0.5f));
        assertEquals("施法震感：100%", JuiceControls.describe(1.0f));
        assertEquals("施法震感：150%", JuiceControls.describe(1.5f));
    }

    // ---- 接线：按键 → 配置 → 控制器（关档立即复位在播 juice） ----

    @Test
    void cyclingToOffCancelsInFlightJuice() {
        // 走完整真实链路把一发 juice 打出去：技能栏预测 → 服务端权威 cast_sync{casting}
        // 武装 → cast_sync{complete} 触发。不能直接调 onAnimPlayed——那条路径现在要求
        // 一枚由权威 CASTING 武装的令牌（review finding A/B）。
        int slot = 3;
        long startedAt = 1_700_000_000_000L;
        SkillBarStore.updateSlot(slot,
            SkillBarEntry.skill("baomai.full_power_release", "全力", 2000, 0, ""));
        CastStateStore.addTransitionListener(CastFovController::onCastState);
        CastStateStore.beginSkillBarCast(slot, 2000, startedAt);
        castSync("casting", slot, startedAt, "none");
        castSync("complete", slot, startedAt, "completed");

        long fire = now[0];
        now[0] += 3 * 50;   // FOV punch（7t）半程达峰
        assertTrue(CastFovController.fovDelta() > 0.0, "关档前 FOV 脉冲有非零偏移");
        assertTrue(!CameraShakeController.activeOffsets(fire).isZero(), "关档前震动在播");

        int guard = 0;
        while (JuiceConfig.juiceMultiplier() > 0f && guard++ < 10) {
            JuiceControls.consumeCyclePresses(true, false, presses(1), feedback::add);
        }

        assertEquals(0f, JuiceConfig.juiceMultiplier(), 1e-9f);
        assertEquals(0.0, CastFovController.fovDelta(), 1e-9, "按键关档 → FOV 立即复位");
        assertTrue(CameraShakeController.activeOffsets(fire).isZero(), "按键关档 → 在播震动也停");
    }
}
