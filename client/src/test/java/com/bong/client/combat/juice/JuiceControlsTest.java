package com.bong.client.combat.juice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.network.CastSyncHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;

import net.minecraft.text.Text;
import net.minecraft.text.TranslatableTextContent;

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
    private final List<Text> feedback = new ArrayList<>();
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
        assertEquals(1, feedback.size(), "每次切换都回显新档位");
        assertFeedback(feedback.get(0), JuiceControls.FEEDBACK_LEVEL_KEY, "150");
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
        assertFeedback(feedback.get(feedback.size() - 1), JuiceControls.FEEDBACK_OFF_KEY);
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
        assertEquals(0, consumed, "GUI 打开时不切档（在输入框里打字不该改配置）");
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f);
    }

    @Test
    void pressesDuringScreenAreDiscardedNotDeferredToAfterClose() {
        // review finding G：wasPressed 是**累积队列**。门控期间若不读队列，按键只是攒着，
        // 关掉界面后的下一 tick 会被补消费 → 倍率延迟连跳。用**同一个有状态队列** supplier
        // 跨两次调用（先 screenOpen=true 再 false），断言配置全程不变。
        java.util.function.BooleanSupplier sharedQueue = presses(3);

        assertEquals(0, JuiceControls.consumeCyclePresses(true, true, sharedQueue, feedback::add),
            "GUI 打开期间不切档");
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f,
            "GUI 打开期间配置不变");

        assertEquals(0, JuiceControls.consumeCyclePresses(true, false, sharedQueue, feedback::add),
            "关掉 GUI 后旧按键必须已被丢弃，不得补触发");
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f,
            "关掉 GUI 后配置仍是默认值（若攒着补消费，这里会跳到 0.5 甚至更远）");
        assertTrue(feedback.isEmpty(), "被丢弃的按键不得回显");

        // 反证：队列已空 ≠ 通道坏了——新按键照常生效。
        assertEquals(1, JuiceControls.consumeCyclePresses(true, false, presses(1), feedback::add));
        assertEquals(1.5f, JuiceConfig.juiceMultiplier(), 1e-9f, "关闭 GUI 后的新按键照常切档");
    }

    @Test
    void pressesWithNoPlayerAreDiscardedNotDeferred() {
        // 同一条队列语义在「无玩家」门控上也成立（登录中 / 切世界瞬间按到的键不许攒着）。
        java.util.function.BooleanSupplier sharedQueue = presses(2);

        assertEquals(0, JuiceControls.consumeCyclePresses(false, false, sharedQueue, feedback::add));
        assertEquals(0, JuiceControls.consumeCyclePresses(true, false, sharedQueue, feedback::add),
            "玩家就位后旧按键必须已被丢弃");
        assertEquals(JuiceConfig.DEFAULT_JUICE_MULTIPLIER, JuiceConfig.juiceMultiplier(), 1e-9f);
        assertTrue(feedback.isEmpty(), "被丢弃的按键不得回显");
    }

    // ---- 错误分支：无回显 sink ----

    @Test
    void nullFeedbackSinkStillCycles() {
        int consumed = JuiceControls.consumeCyclePresses(true, false, presses(1), null);
        assertEquals(1, consumed);
        assertEquals(1.5f, JuiceConfig.juiceMultiplier(), 1e-9f, "回显缺席不得影响配置切换");
    }

    // ---- 回显文案 ----

    /**
     * 断言回显是**翻译 key + 参数**，不是把中文文案固化成业务契约（review finding H）：
     * 文案改字、加语言都不该让测试红；key/参数变了才该红。
     */
    private static void assertFeedback(Text actual, String expectedKey, String... expectedArgs) {
        assertTrue(actual.getContent() instanceof TranslatableTextContent,
            "动作栏回显必须是 translatable（英文客户端不能看到硬编码中文），实际内容类型 "
                + actual.getContent().getClass().getSimpleName());
        TranslatableTextContent content = (TranslatableTextContent) actual.getContent();
        assertEquals(expectedKey, content.getKey(), "翻译 key 不符");
        assertEquals(List.of((Object[]) expectedArgs), List.of(content.getArgs()),
            "翻译参数不符（百分数由代码按 Locale.ROOT 格式化后传入，不进 lang）");
    }

    @Test
    void describeUsesTranslationKeysForOffAndScaledLevels() {
        assertFeedback(JuiceControls.describe(0f), JuiceControls.FEEDBACK_OFF_KEY);
        assertFeedback(JuiceControls.describe(-1f), JuiceControls.FEEDBACK_OFF_KEY);
        assertFeedback(JuiceControls.describe(0.5f), JuiceControls.FEEDBACK_LEVEL_KEY, "50");
        assertFeedback(JuiceControls.describe(1.0f), JuiceControls.FEEDBACK_LEVEL_KEY, "100");
        assertFeedback(JuiceControls.describe(1.5f), JuiceControls.FEEDBACK_LEVEL_KEY, "150");
    }

    @Test
    void bothLangFilesDeclareEveryFeedbackKey() throws Exception {
        // 只用 key 不落 lang = 玩家看到裸 key。双份 lang 都必须有，且百分比档带 %s 占位。
        for (String lang : List.of("en_us", "zh_cn")) {
            java.nio.file.Path file = java.nio.file.Path.of(
                "src/main/resources/assets/bong-client/lang/" + lang + ".json");
            com.google.gson.JsonObject json = com.google.gson.JsonParser
                .parseString(java.nio.file.Files.readString(file)).getAsJsonObject();
            assertTrue(json.has(JuiceControls.FEEDBACK_OFF_KEY),
                lang + ".json 缺 " + JuiceControls.FEEDBACK_OFF_KEY);
            assertTrue(json.has(JuiceControls.FEEDBACK_LEVEL_KEY),
                lang + ".json 缺 " + JuiceControls.FEEDBACK_LEVEL_KEY);
            String level = json.get(JuiceControls.FEEDBACK_LEVEL_KEY).getAsString();
            assertTrue(level.contains("%s"),
                lang + ".json 的百分比档文案必须含 %s 占位（否则档位数字显示不出来）：" + level);
        }
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
