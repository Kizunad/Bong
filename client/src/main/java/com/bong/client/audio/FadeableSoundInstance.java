package com.bong.client.audio;

import net.minecraft.client.sound.AbstractSoundInstance;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.client.sound.TickableSoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.random.Random;

/**
 * {@link SoundInstance} 实现，把 {@code fadeOutTicks} 落成真实可执行的音量渐弱停音。
 *
 * <p>Minecraft {@code SoundSystem#tick()} 对每个已注册的 {@link TickableSoundInstance}
 * 每游戏 tick 都会：调用 {@link #tick()} → 检查 {@link #isDone()} → 若未完成则重新读取
 * {@link #getVolume()} 并把新音量下发给底层 OpenAL source（反编译
 * {@code net/minecraft/client/sound/SoundSystem.class} 字节码确认：
 * {@code tickingSounds} 迭代 → {@code tick()} → {@code isDone()} →
 * {@code getAdjustedVolume(instance)} → {@code Channel$SourceManager.run(...)}）。
 * 只要在 {@link #tick()} 里逐 tick 下调 {@link #volume} 字段，引擎就会把渐弱声音真实推给
 * 音频输出——不需要任何额外的 mixer/ducking hack，也不需要手动摘除 source。
 *
 * <p>在 {@link #beginFadeOut(int)} 调用前，实例保持满音量常驻播放（未开始淡出），
 * {@link #isDone()} 恒为 {@code false}——由 {@code SoundManager.play(instance, delayTicks)}
 * 自动把它注册进 {@code tickingSounds}（构造方法 instanceof 检查），无需额外接线。
 */
final class FadeableSoundInstance extends AbstractSoundInstance implements TickableSoundInstance {
    private final float baseVolume;
    private int fadeTicksRemaining = -1;
    private int fadeTicksTotal;
    private boolean done;

    FadeableSoundInstance(
        Identifier sound,
        SoundCategory category,
        float volume,
        float pitch,
        Random random,
        boolean repeat,
        int repeatDelay,
        SoundInstance.AttenuationType attenuationType,
        double x,
        double y,
        double z,
        boolean relative
    ) {
        super(sound, category, random);
        this.baseVolume = volume;
        this.volume = volume;
        this.pitch = pitch;
        this.repeat = repeat;
        this.repeatDelay = repeatDelay;
        this.attenuationType = attenuationType;
        this.x = x;
        this.y = y;
        this.z = z;
        this.relative = relative;
    }

    /**
     * 开始淡出。{@code ticks <= 0} 视为立即停音（音量瞬间归零、标记完成，
     * 与旧行为的"硬停"语义等价，保留给不需要淡出的调用方）。
     * 重复调用会用最新一次的 ticks 重新起算淡出窗口。
     */
    void beginFadeOut(int ticks) {
        if (ticks <= 0) {
            volume = 0.0f;
            done = true;
            return;
        }
        fadeTicksTotal = ticks;
        fadeTicksRemaining = ticks;
    }

    boolean isFading() {
        return fadeTicksRemaining >= 0 && !done;
    }

    /**
     * 测试钩子：直接读取内部 {@code volume} 字段。
     *
     * <p>继承自 {@link AbstractSoundInstance} 的 {@code getVolume()} 会做
     * {@code volume * sound.getVolume().get(random)}——{@code sound} 字段只有
     * 真实 {@code SoundManager} 处理 {@code play()} 时通过
     * {@link #getSoundSet(net.minecraft.client.sound.SoundManager)} 解析后才会填充，
     * 无 MC 客户端的 headless 单测环境下恒为 {@code null}，直接调用 {@code getVolume()}
     * 会 NPE。淡出状态机真正控制、也是本类唯一关心的是这个原始 {@code volume} 字段，
     * 所以测试直接读它，不依赖 SoundManager 解析。
     */
    float volumeForTests() {
        return volume;
    }

    /**
     * 测试钩子：直接读取内部 {@code pitch} 字段，理由同 {@link #volumeForTests()}——
     * 继承的 {@code getPitch()} 同样会乘上未解析的 {@code sound.getPitch()} FloatSupplier。
     */
    float pitchForTests() {
        return pitch;
    }

    @Override
    public void tick() {
        if (done || fadeTicksRemaining < 0) {
            return;
        }
        fadeTicksRemaining--;
        if (fadeTicksRemaining <= 0) {
            volume = 0.0f;
            done = true;
        } else {
            volume = baseVolume * ((float) fadeTicksRemaining / (float) fadeTicksTotal);
        }
    }

    @Override
    public boolean isDone() {
        return done;
    }
}
