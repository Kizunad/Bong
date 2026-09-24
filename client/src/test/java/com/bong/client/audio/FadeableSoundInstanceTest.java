package com.bong.client.audio;

import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.random.Random;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * FadeableSoundInstance 单测（plan-bughunt-z-fauna-audio-fade-stop-v1）。
 *
 * <p>根因：{@code MinecraftSoundSink.stop()} 此前无条件 {@code soundManager.stop(instance)}，
 * 把上层传下来的 {@code fadeOutTicks} 全部吞掉（Fuya 压迫 hum / 环境 loop / 音乐切换全部硬切）。
 * 修复把 fadeOutTicks 落成 {@link FadeableSoundInstance} 的真实音量渐弱状态机——Minecraft
 * {@code SoundSystem.tick()} 对 {@link net.minecraft.client.sound.TickableSoundInstance}
 * 每 tick 重新读取 {@code getVolume()}，所以这里只测状态机本身（不需要真实运行的
 * MinecraftClient，纯 JVM 可跑）：淡出前满音量常驻、淡出中逐 tick 线性衰减、淡出完成后
 * volume==0 且 isDone()==true 交给引擎自行摘除 channel。
 */
class FadeableSoundInstanceTest {

    private static final Identifier SOUND_ID = new Identifier("minecraft", "ambient.cave");

    private static FadeableSoundInstance instance(float volume) {
        return new FadeableSoundInstance(
            SOUND_ID,
            SoundCategory.AMBIENT,
            volume,
            1.0f,
            Random.create(),
            false,
            0,
            SoundInstance.AttenuationType.LINEAR,
            10.0,
            64.0,
            -20.0,
            false
        );
    }

    // ─── happy path：未开始淡出 = 满音量常驻 ────────────────────────────────

    @Test
    void notFading_holdsFullVolumeAndNeverDone() {
        FadeableSoundInstance sound = instance(0.8f);

        // 未调用 beginFadeOut 前，tick() 反复调用也不应改变任何状态——
        // 这是"环境音乐/压迫 hum 播放中、尚未被要求停止"的正常态。
        sound.tick();
        sound.tick();
        sound.tick();

        assertEquals(0.8f, sound.volumeForTests(), 1e-6f,
            "未触发淡出时 tick() 不应改变音量，实际=" + sound.volumeForTests());
        assertFalse(sound.isDone(), "未触发淡出的常驻音效不应被标记完成");
        assertFalse(sound.isFading(), "未调用 beginFadeOut 时 isFading() 应为 false");
    }

    // ─── 边界：fadeOutTicks <= 0 = 立即停音（旧硬切语义保留给显式 0） ────────

    @Test
    void beginFadeOut_zeroTicks_immediatelySilencedAndDone() {
        FadeableSoundInstance sound = instance(1.0f);

        sound.beginFadeOut(0);

        assertEquals(0.0f, sound.volumeForTests(), 1e-6f, "fadeOutTicks=0 应立即把音量归零");
        assertTrue(sound.isDone(), "fadeOutTicks=0 应立即标记完成（等价旧硬停）");
        assertFalse(sound.isFading(), "已经 done 的实例不应再算作 fading 中");
    }

    @Test
    void beginFadeOut_negativeTicks_treatedAsImmediateStop() {
        FadeableSoundInstance sound = instance(1.0f);

        sound.beginFadeOut(-5);

        assertEquals(0.0f, sound.volumeForTests(), 1e-6f, "负数 ticks 是防御性输入，应等价于立即停音");
        assertTrue(sound.isDone(), "负数 ticks 应立即标记完成，不应挂起等待不存在的淡出窗口");
    }

    // ─── happy path：多 tick 线性淡出 ────────────────────────────────────────

    @Test
    void beginFadeOut_fourTicks_volumeRampsLinearlyThenDone() {
        FadeableSoundInstance sound = instance(1.0f);

        sound.beginFadeOut(4);
        assertFalse(sound.isDone(), "beginFadeOut 刚调用、tick() 还未跑过时不应立即 done");
        assertTrue(sound.isFading(), "淡出窗口开启后 isFading() 应为 true");

        sound.tick();
        assertEquals(0.75f, sound.volumeForTests(), 1e-6f, "第 1/4 tick 后音量应为 3/4 base");
        assertFalse(sound.isDone(), "第 1/4 tick 后淡出未结束，不应标记完成");

        sound.tick();
        assertEquals(0.5f, sound.volumeForTests(), 1e-6f, "第 2/4 tick 后音量应为 1/2 base");
        assertFalse(sound.isDone(), "第 2/4 tick 后淡出未结束，不应标记完成");

        sound.tick();
        assertEquals(0.25f, sound.volumeForTests(), 1e-6f, "第 3/4 tick 后音量应为 1/4 base");
        assertFalse(sound.isDone(), "第 3 个 tick（还剩 1 tick）不应提前标记完成");

        sound.tick();
        assertEquals(0.0f, sound.volumeForTests(), 1e-6f, "第 4/4 tick 后音量应归零");
        assertTrue(sound.isDone(), "淡出窗口耗尽后应标记完成，交给引擎摘除 channel");
        assertFalse(sound.isFading(), "done 之后不应再算 fading 中");
    }

    @Test
    void beginFadeOut_respectsNonUnitBaseVolume() {
        // baseVolume != 1.0 时比例衰减仍要按 baseVolume 折算，不能硬编码绝对值。
        FadeableSoundInstance sound = instance(0.6f);

        sound.beginFadeOut(2);
        sound.tick();
        assertEquals(0.3f, sound.volumeForTests(), 1e-6f, "base=0.6 时第 1/2 tick 应为 0.3");

        sound.tick();
        assertEquals(0.0f, sound.volumeForTests(), 1e-6f, "第 2/2 tick 后应归零");
        assertTrue(sound.isDone(), "base=0.6 的 2-tick 窗口耗尽后应标记完成");
    }

    // ─── 边界：单 tick 淡出窗口 ─────────────────────────────────────────────

    @Test
    void beginFadeOut_oneTick_completesOnFirstTick() {
        FadeableSoundInstance sound = instance(1.0f);

        sound.beginFadeOut(1);
        assertFalse(sound.isDone(), "调用 beginFadeOut(1) 后、tick() 前不应立即 done");

        sound.tick();
        assertEquals(0.0f, sound.volumeForTests(), 1e-6f);
        assertTrue(sound.isDone(), "唯一一个 tick 消耗完后应立即完成");
    }

    // ─── 状态转换：done 之后 tick() 必须幂等 ────────────────────────────────

    @Test
    void tick_afterDone_isIdempotentAndStaysSilent() {
        FadeableSoundInstance sound = instance(1.0f);
        sound.beginFadeOut(2);
        sound.tick();
        sound.tick();
        assertTrue(sound.isDone(), "2-tick 窗口耗尽后应标记完成，为后续幂等断言建立前置");

        // done 之后继续 tick（引擎摘除 channel 前可能还会跑一两个 tick）不应抛异常，
        // 不应让 fadeTicksRemaining 继续往负数走、更不应让音量重新变化。
        sound.tick();
        sound.tick();
        sound.tick();

        assertEquals(0.0f, sound.volumeForTests(), 1e-6f, "done 之后音量必须保持静音");
        assertTrue(sound.isDone(), "done 之后应恒为 done");
    }

    // ─── 状态转换：淡出中途被重新触发 = 用最新窗口重新起算 ───────────────────

    @Test
    void beginFadeOut_calledAgainMidFade_restartsFromLatestWindow() {
        FadeableSoundInstance sound = instance(1.0f);

        sound.beginFadeOut(10);
        sound.tick();
        sound.tick();
        assertEquals(0.8f, sound.volumeForTests(), 1e-6f, "10-tick 窗口内已消耗 2 tick，应为 8/10 base");

        // 中途重新调用 stop（例如同一 flag 再次触发）：应重新起算，而不是叠加/忽略。
        sound.beginFadeOut(4);
        assertFalse(sound.isDone(), "重新起算 4-tick 新窗口后不应立即完成");

        sound.tick();
        assertEquals(0.75f, sound.volumeForTests(), 1e-6f, "重新起算后第 1/4 tick 应为 3/4 base（新 base 取当前调用时刻的 baseVolume）");
        sound.tick();
        sound.tick();
        sound.tick();
        assertEquals(0.0f, sound.volumeForTests(), 1e-6f);
        assertTrue(sound.isDone(), "重新起算的新窗口耗尽后仍应正确完成");
    }

    // ─── 契约保真：位置/朝向/分类等字段必须与旧 PositionedSoundInstance 语义一致 ──

    @Test
    void constructorFields_arePassedThroughUnchanged() {
        FadeableSoundInstance sound = new FadeableSoundInstance(
            SOUND_ID,
            SoundCategory.HOSTILE,
            0.9f,
            1.3f,
            Random.create(),
            true,
            5,
            SoundInstance.AttenuationType.NONE,
            11.0,
            65.0,
            -21.0,
            true
        );

        assertEquals(SOUND_ID, sound.getId(), "sound id 必须原样透传");
        assertEquals(SoundCategory.HOSTILE, sound.getCategory(), "category 必须原样透传");
        assertEquals(0.9f, sound.volumeForTests(), 1e-6f, "初始音量必须等于构造传入值");
        assertEquals(1.3f, sound.pitchForTests(), 1e-6f, "pitch 必须原样透传");
        assertTrue(sound.isRepeatable(), "repeat 标志必须原样透传");
        assertEquals(5, sound.getRepeatDelay(), "repeatDelay 必须原样透传");
        assertEquals(SoundInstance.AttenuationType.NONE, sound.getAttenuationType(), "attenuationType 必须原样透传");
        assertEquals(11.0, sound.getX(), 1e-9, "x 坐标必须原样透传");
        assertEquals(65.0, sound.getY(), 1e-9, "y 坐标必须原样透传");
        assertEquals(-21.0, sound.getZ(), 1e-9, "z 坐标必须原样透传");
        assertTrue(sound.isRelative(), "relative 标志必须原样透传（PLAYER_LOCAL/SELF 衰减用）");
    }
}
