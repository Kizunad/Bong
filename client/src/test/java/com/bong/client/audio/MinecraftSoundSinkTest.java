package com.bong.client.audio;

import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.random.Random;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * MinecraftSoundSink.stopInternal 生命周期单测（plan-bughunt-z-fauna-audio-fade-stop-v1，
 * CodeRabbit PR#1177 Major 返工）。
 *
 * <p>锁住的契约：淡出中的实例必须保留在 activeByInstance 索引里——同一 instanceId
 * 在淡出期间再次收到 stop 时，硬停要能立即掐掉正在淡出的声音、更短的淡出窗口要能
 * 覆盖正在进行的淡出；只有硬停分支移除映射；淡出完成的残留条目由 pruneFinished
 * 惰性清理防泄漏。stopInternal 与 MinecraftClient 解耦（hardStopper 注入），
 * headless JVM 可直接测。
 */
class MinecraftSoundSinkTest {

    private static final Identifier SOUND_ID = new Identifier("minecraft", "ambient.cave");

    private static FadeableSoundInstance instance() {
        return new FadeableSoundInstance(
            SOUND_ID,
            SoundCategory.AMBIENT,
            1.0f,
            1.0f,
            Random.create(),
            false,
            0,
            SoundInstance.AttenuationType.LINEAR,
            0.0,
            64.0,
            0.0,
            false
        );
    }

    // ─── Major 核心：淡出期间的重复 stop 不再被吞 ───────────────────────────

    @Test
    void hardStopDuringFade_stopsImmediatelyAndRemovesMapping() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance sound = instance();
        sink.registerInstanceForTests(7L, sound);
        List<FadeableSoundInstance> hardStopped = new ArrayList<>();

        // 先淡出（20 tick），映射必须保留
        sink.stopInternal(7L, 20, hardStopped::add);
        assertTrue(sound.isFading(), "fade stop 后实例应处于淡出中");
        assertEquals(1, sink.trackedInstanceCountForTests(),
            "淡出中的实例必须保留在索引里（旧实现 remove-first 会在这里丢失映射），实际条目数="
                + sink.trackedInstanceCountForTests());
        assertEquals(0, hardStopped.size(), "fade 分支不应触发硬停回调");

        // 淡出进行到一半，升级为硬停：必须立即掐掉，不能因 instances==null 被静默吞掉
        sound.tick();
        sound.tick();
        sink.stopInternal(7L, 0, hardStopped::add);
        assertEquals(1, hardStopped.size(),
            "淡出期间的硬停请求必须真实下发 soundManager.stop（旧实现会静默丢弃），实际回调次数="
                + hardStopped.size());
        assertEquals(0, sink.trackedInstanceCountForTests(), "硬停后映射应被移除");
    }

    @Test
    void shorterFadeDuringFade_overridesOngoingFadeWindow() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance sound = instance();
        sink.registerInstanceForTests(8L, sound);

        // 长淡出 100 tick，跑 10 tick 后音量 0.9
        sink.stopInternal(8L, 100, ignored -> { });
        for (int i = 0; i < 10; i++) {
            sound.tick();
        }
        assertEquals(0.9f, sound.volumeForTests(), 1e-6f,
            "100-tick 窗口消耗 10 tick 后音量应为 0.9，实际=" + sound.volumeForTests());

        // 第二次 stop 用更短窗口（2 tick）：必须覆盖正在进行的淡出（旧实现映射已丢，这里会被吞）
        sink.stopInternal(8L, 2, ignored -> { });
        sound.tick();
        sound.tick();
        assertEquals(0.0f, sound.volumeForTests(), 1e-6f,
            "更短的 2-tick 窗口应覆盖原 100-tick 淡出并在 2 tick 内归零，实际=" + sound.volumeForTests());
        assertTrue(sound.isDone(), "被更短窗口覆盖后淡出应正常完成");
    }

    // ─── 硬停路径：语义与旧实现一致 ─────────────────────────────────────────

    @Test
    void hardStop_removesMappingAndSecondHardStopIsNoOp() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance sound = instance();
        sink.registerInstanceForTests(9L, sound);
        List<FadeableSoundInstance> hardStopped = new ArrayList<>();

        sink.stopInternal(9L, 0, hardStopped::add);
        assertEquals(1, hardStopped.size(), "首次硬停应下发 1 次 stop 回调");
        assertEquals(0, sink.trackedInstanceCountForTests(), "硬停后映射应被移除");

        sink.stopInternal(9L, 0, hardStopped::add);
        assertEquals(1, hardStopped.size(), "重复硬停应为 no-op（映射已移除），不应再次回调");
    }

    /**
     * 硬停必须让**尚未开声的延迟层**也彻底哑掉：layer 的 delay_ticks 交给 vanilla
     * `SoundSystem#soundsToPlay` 排队，`soundManager.stop()` 只能停已起声的 source，
     * 排队中的实例到点仍会被取出播放。归零音量 + 标完成后，即便被取出也没有可听输出。
     * 回归来源：心跳 loop 的 `entity.player.hurt` 层（delay_ticks=2）曾在重生收掉
     * loop 之后还补响一声。
     */
    @Test
    void hardStop_silencesPendingDelayedLayer() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance pendingDelayedLayer = instance();
        sink.registerInstanceForTests(15L, pendingDelayedLayer);

        sink.stopInternal(15L, 0, ignored -> { });

        assertEquals(0.0f, pendingDelayedLayer.volumeForTests(), 1e-6f,
            "期望硬停把延迟层音量归零，因为 delay 队列里的实例仍会被引擎取出播放"
                + "（soundManager.stop 停不掉未起声的 source）；实际音量="
                + pendingDelayedLayer.volumeForTests());
        assertTrue(pendingDelayedLayer.isDone(),
            "期望硬停后实例标记完成，这样被引擎取出时会立即摘除；实际 isDone=false");
    }

    @Test
    void hardStop_stopsAllInstancesUnderSameId() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance layerA = instance();
        FadeableSoundInstance layerB = instance();
        sink.registerInstanceForTests(10L, layerA);
        sink.registerInstanceForTests(10L, layerB);
        List<FadeableSoundInstance> hardStopped = new ArrayList<>();

        sink.stopInternal(10L, 0, hardStopped::add);
        assertEquals(2, hardStopped.size(),
            "同一 instanceId 下的多层实例（recipe 多 layer）应全部硬停，实际=" + hardStopped.size());
    }

    @Test
    void fadeStop_fadesAllInstancesUnderSameId() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance layerA = instance();
        FadeableSoundInstance layerB = instance();
        sink.registerInstanceForTests(11L, layerA);
        sink.registerInstanceForTests(11L, layerB);

        sink.stopInternal(11L, 5, ignored -> { });
        assertTrue(layerA.isFading(), "layer A 应进入淡出");
        assertTrue(layerB.isFading(), "layer B 应进入淡出");
    }

    // ─── 断线边界：全局 hard-stop + 索引清空 ──────────────────────────────

    @Test
    void clearOnDisconnectHardStopsActiveAndDelayedInstancesThenClearsIndex() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance activeLayer = instance();
        FadeableSoundInstance delayedLayer = instance();
        FadeableSoundInstance anotherLoop = instance();
        sink.registerInstanceForTests(21L, activeLayer);
        sink.registerInstanceForTests(21L, delayedLayer);
        sink.registerInstanceForTests(22L, anotherLoop);
        List<FadeableSoundInstance> hardStopped = new ArrayList<>();

        sink.clearOnDisconnectInternal(hardStopped::add);

        assertEquals(3, hardStopped.size(), "断线必须 hard-stop 所有 instanceId 下的活动与延迟层");
        assertTrue(activeLayer.isDone(), "活动层断线后必须标记 done");
        assertTrue(delayedLayer.isDone(), "延迟层断线后必须标记 done，不能在新 session 补响");
        assertTrue(anotherLoop.isDone(), "不同 loop instance 也必须硬停");
        assertEquals(0.0f, delayedLayer.volumeForTests(), 1e-6f,
            "延迟层必须归零音量，即使 vanilla 延迟队列之后取出它也无可听输出");
        assertEquals(0, sink.trackedInstanceCountForTests(), "断线必须清空 activeByInstance 索引");
    }

    @Test
    void clearOnDisconnectRuntimeFailureStillSilencesEveryInstanceAndClearsIndex() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance first = instance();
        FadeableSoundInstance second = instance();
        sink.registerInstanceForTests(31L, first);
        sink.registerInstanceForTests(32L, second);
        int[] attempts = {0};

        sink.clearOnDisconnectInternal(ignored -> {
            attempts[0]++;
            throw new IllegalStateException("sound manager unavailable");
        });

        assertEquals(2, attempts[0], "单个 hardStopper RuntimeException 不得阻断其余实例清理");
        assertTrue(first.isDone(), "失败实例也必须先标记 done，不能在新 session 补响");
        assertTrue(second.isDone(), "后续实例也必须继续标记 done");
        assertEquals(0.0f, first.volumeForTests(), 1e-6f, "失败实例也必须先静音");
        assertEquals(0.0f, second.volumeForTests(), 1e-6f, "后续实例也必须静音");
        assertEquals(0, sink.trackedInstanceCountForTests(), "RuntimeException 后也必须清空实例索引");
    }

    @Test
    void clearOnDisconnectStillPropagatesHardStopperError() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance sound = instance();
        sink.registerInstanceForTests(33L, sound);

        AssertionError error = assertThrows(
            AssertionError.class,
            () -> sink.clearOnDisconnectInternal(ignored -> {
                throw new AssertionError("fatal hard stop");
            })
        );

        assertEquals("fatal hard stop", error.getMessage(), "Error 必须原样透传");
        assertTrue(sound.isDone(), "Error 前也必须先把当前实例静音并标记 done");
        assertEquals(1, sink.trackedInstanceCountForTests(),
            "Error 透传不伪装成已完成的全局索引清理");
    }

    @Test
    void clearOnDisconnectOnEmptySinkIsIdempotent() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        List<FadeableSoundInstance> hardStopped = new ArrayList<>();

        sink.clearOnDisconnectInternal(hardStopped::add);
        sink.clearOnDisconnectInternal(hardStopped::add);

        assertEquals(0, hardStopped.size(), "空 sink 反复断线清理不得虚构 hard-stop");
        assertEquals(0, sink.trackedInstanceCountForTests(), "空 sink 反复断线清理必须保持空索引");
    }

    // ─── 边界：未知 id / 空索引 ─────────────────────────────────────────────

    @Test
    void stopUnknownId_isNoOpWithoutException() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        List<FadeableSoundInstance> hardStopped = new ArrayList<>();

        sink.stopInternal(999L, 0, hardStopped::add);
        sink.stopInternal(999L, 20, hardStopped::add);

        assertEquals(0, hardStopped.size(), "未知 instanceId 的 stop 应为 no-op");
        assertEquals(0, sink.trackedInstanceCountForTests(), "未知 id 不应产生索引条目");
    }

    // ─── 泄漏防护：淡出完成的条目被惰性清理 ─────────────────────────────────

    @Test
    void finishedFadeEntries_arePrunedOnSubsequentStop() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance sound = instance();
        sink.registerInstanceForTests(12L, sound);

        sink.stopInternal(12L, 2, ignored -> { });
        sound.tick();
        sound.tick();
        assertTrue(sound.isDone(), "2-tick 淡出窗口耗尽后实例应完成");
        assertEquals(1, sink.trackedInstanceCountForTests(),
            "淡出刚完成、尚未有后续 sink 调用时条目仍在（惰性清理语义）");

        // 任意后续 stop 调用触发 prune（play 路径同样触发，但 headless 下无法调 play）
        sink.stopInternal(999L, 0, ignored -> { });
        assertEquals(0, sink.trackedInstanceCountForTests(),
            "后续 sink 调用应把已完成淡出的条目清出索引（防长会话映射泄漏），实际="
                + sink.trackedInstanceCountForTests());
    }

    @Test
    void pruneKeepsUnfinishedInstances() {
        MinecraftSoundSink sink = new MinecraftSoundSink();
        FadeableSoundInstance fadingSound = instance();
        FadeableSoundInstance idleSound = instance();
        sink.registerInstanceForTests(13L, fadingSound);
        sink.registerInstanceForTests(14L, idleSound);

        sink.stopInternal(13L, 10, ignored -> { });
        // 13 在淡出中（未完成），14 常驻未 stop——两者都不应被 prune
        sink.stopInternal(999L, 0, ignored -> { });
        assertEquals(2, sink.trackedInstanceCountForTests(),
            "淡出未完成与常驻实例都不应被 prune，实际=" + sink.trackedInstanceCountForTests());
        assertFalse(fadingSound.isDone(), "淡出中的实例不应被误标完成");
    }
}
