package com.bong.client.fauna;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.sound.PositionedSoundInstance;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.random.Random;

/**
 * plan-fauna-stitched-beast-v1 P3 — 幻觉层客户端 tick 驱动控制器。
 *
 * <p>挂在 {@link ClientTickEvents#END_CLIENT_TICK}（20×/s，游戏 tick 频率），
 * 负责所有 <em>tick-based</em> 状态更新：
 * <ul>
 *   <li>{@link HallucinationLayerStore#decrementTick()}：幻觉倒计时递减</li>
 *   <li>{@link HallucinationLayerStore#tickFade}：fade-in / fade-out 进度推进</li>
 *   <li>bar 偏移重随机（每 {@link HallucinationHudOverlay#BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS} tick）</li>
 *   <li>ambient.cave 音效：幻觉激活期间每 tick 渐变 pitch（0.5 → 1.2）播放</li>
 * </ul>
 *
 * <p><b>修复说明（M2 blocker fix）</b>：原实现在 {@link HallucinationHudOverlay#render} 内调用
 * {@code decrementTick()} / {@code tickFade()}，导致 120fps 下 200 tick 约 1.7s 耗尽。
 * 本类将所有递减/进度更新移到游戏 tick（20Hz），使 200 tick = 10s @ 20TPS，与 server 对齐，帧率无关。
 *
 * <p>{@link HallucinationHudOverlay#render} 只读状态，不再变更任何计数器。
 *
 * <p>音效规格（plan P3）：ambient.cave.cave1 渐变 pitch 0.5 → 1.2，volume 0.4，
 * 幻觉激活首 tick 播放，之后每 {@value SOUND_RETRIGGER_INTERVAL_TICKS} tick 重触发（淡出后淡入模拟渐变）。
 *
 * <p>线程安全：全部在 Minecraft 主线程调用（ClientTickEvents 保证）。
 */
public final class HallucinationTickController {

    // ──────────────────────────────────────────────────────────────────────────
    // 音效参数
    // ──────────────────────────────────────────────────────────────────────────

    /** ambient.cave 音效的基础音量。 */
    static final float SOUND_VOLUME = 0.4f;

    /** 幻觉起始 pitch（低沉洞穴感）。 */
    static final float SOUND_PITCH_START = 0.5f;

    /** 幻觉结束 pitch（越来越紧绷）。 */
    static final float SOUND_PITCH_END = 1.2f;

    /**
     * 音效重触发间隔（tick）。ambient.cave 音效约 12-16s，20tick 重触发保证连续感。
     * 每次以当前 pitch 进度重播，模拟渐变效果。
     */
    static final int SOUND_RETRIGGER_INTERVAL_TICKS = 20;

    // ──────────────────────────────────────────────────────────────────────────
    // 内部状态
    // ──────────────────────────────────────────────────────────────────────────

    private static final Random RAND = Random.create();

    /** bar 偏移重随机计数（tick 单位）。 */
    private static int barReshuffleTick = 0;

    /** 音效重触发计数（tick 单位）。 */
    private static int soundRetriggerTick = 0;

    /** 当前是否已注册 tick 监听（防重复注册）。 */
    private static boolean bootstrapped = false;

    private HallucinationTickController() {}

    // ──────────────────────────────────────────────────────────────────────────
    // Bootstrap
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 注册 ClientTickEvents.END_CLIENT_TICK 监听（BongClient.onInitializeClient 调用一次）。
     */
    public static void bootstrap() {
        if (bootstrapped) {
            return;
        }
        bootstrapped = true;
        ClientTickEvents.END_CLIENT_TICK.register(HallucinationTickController::onEndClientTick);
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Tick 处理
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 每游戏 tick（20Hz）调用。
     *
     * <p>更新顺序：
     * <ol>
     *   <li>decrementTick：幻觉倒计时递减</li>
     *   <li>tickFade：fade 进度推进</li>
     *   <li>bar 偏移重随机</li>
     *   <li>音效触发（激活期间）</li>
     * </ol>
     */
    static void onEndClientTick(MinecraftClient client) {
        // 1. 幻觉倒计时递减（tick-based，与 server 对齐）
        HallucinationLayerStore.decrementTick();

        // 2. fade 进度推进（tick-based steps；tickFade 的 sinPhaseIncrement 对应单 tick 相位）
        HallucinationLayerStore.tickFade(
            HallucinationHudOverlay.SIN_PHASE_INCREMENT_PER_TICK,
            HallucinationHudOverlay.FADE_IN_STEP_PER_TICK,
            HallucinationHudOverlay.FADE_OUT_STEP_PER_TICK
        );

        // 3. bar 偏移重随机（每 BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS tick）
        float maxOffset = HallucinationHudOverlay.MAX_BAR_OFFSET;
        if (HallucinationLayerStore.isActive()) {
            barReshuffleTick++;
            if (barReshuffleTick >= HallucinationHudOverlay.BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS) {
                barReshuffleTick = 0;
                float hpOffset = (RAND.nextFloat() * 2 * maxOffset) - maxOffset;
                float qiOffset = (RAND.nextFloat() * 2 * maxOffset) - maxOffset;
                HallucinationLayerStore.updateBarOffsets(hpOffset, qiOffset);
            }
        } else {
            barReshuffleTick = 0;
        }

        // 4. 音效触发（仅激活期间）
        if (HallucinationLayerStore.isActive()) {
            soundRetriggerTick++;
            if (soundRetriggerTick >= SOUND_RETRIGGER_INTERVAL_TICKS) {
                soundRetriggerTick = 0;
                playHallucinationSound(client);
            }
        } else {
            soundRetriggerTick = SOUND_RETRIGGER_INTERVAL_TICKS; // 激活时立即播
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 音效播放
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 播放 ambient.cave 音效，pitch 随幻觉进度从 0.5 渐变到 1.2。
     *
     * <p>pitch 公式：{@code PITCH_START + (PITCH_END - PITCH_START) × pitchProgress}，
     * 其中 pitchProgress = (duration - remaining) / duration（从 0 线性到 1）。
     *
     * <p>使用 {@code minecraft:ambient.cave} vanilla 音效标识符，无需新资产。
     *
     * @param client Minecraft 客户端（主线程调用）
     */
    static void playHallucinationSound(MinecraftClient client) {
        if (client == null || client.getSoundManager() == null) {
            return;
        }

        int remaining = HallucinationLayerStore.getRemainingTicks();
        int duration = HallucinationLayerStore.getDurationTicks();

        float pitchProgress = (duration > 0)
            ? (float) (duration - remaining) / duration
            : 0.0f;
        pitchProgress = Math.max(0.0f, Math.min(1.0f, pitchProgress));
        float pitch = SOUND_PITCH_START + (SOUND_PITCH_END - SOUND_PITCH_START) * pitchProgress;

        // ambient.cave 音效（vanilla，无需额外资产）——使用 AMBIENT 类别无衰减本地播放
        // 直接构造 Identifier 避免依赖 SoundEvents 静态字段命名（Yarn 映射不稳定）
        PositionedSoundInstance sound = new PositionedSoundInstance(
            new Identifier("minecraft", "ambient.cave"),
            SoundCategory.AMBIENT,
            SOUND_VOLUME,
            pitch,
            Random.create(),
            false,
            0,
            SoundInstance.AttenuationType.NONE,
            0, 0, 0,
            true // relative = player-local（无位置衰减）
        );
        client.getSoundManager().play(sound);
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 仅供测试
    // ──────────────────────────────────────────────────────────────────────────

    /** 仅供测试：重置内部 tick 计数。 */
    static void resetForTest() {
        barReshuffleTick = 0;
        soundRetriggerTick = SOUND_RETRIGGER_INTERVAL_TICKS;
        bootstrapped = false;
    }

    /** 仅供测试：获取 barReshuffleTick 当前值。 */
    static int getBarReshuffleTickForTest() {
        return barReshuffleTick;
    }

    /** 仅供测试：获取 soundRetriggerTick 当前值。 */
    static int getSoundRetriggerTickForTest() {
        return soundRetriggerTick;
    }
}
