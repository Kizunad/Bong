package com.bong.client.audio;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.environment.EnvironmentAudioLoopState;
import com.bong.client.hud.HudImmersionMode;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.tiandao.TiandaoPresenceState;
import com.bong.client.tiandao.TiandaoPresenceStore;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class SoundRecipePlayerTest {
    @AfterEach
    void resetStores() {
        CombatHudStateStore.resetForTests();
        EnvironmentAudioLoopState.clear();
        HudImmersionMode.resetForTests();
        TiandaoPresenceStore.clear();
    }

    @Test
    void schedulesAllLayersWithVolumeAndPitchModifiers() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);

        player.play(playPayload(recipeWithoutLoop(), 0.5f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        AudioScheduledSound first = sink.played.get(0);
        assertEquals(new Identifier("minecraft", "entity.generic.drink"), first.sound());
        assertEquals(0.2f, first.volume(), 0.0001f);
        assertEquals(1.0f, first.pitch(), 0.0001f);
        assertEquals(0, first.delayTicks());
        AudioScheduledSound second = sink.played.get(1);
        assertEquals(5, second.delayTicks());
    }

    @Test
    void preservesAudioWorldLowPitchFloor() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);

        player.play(playPayload(lowPitchRecipe(), 1.0f, -1.0f));
        player.tick();

        assertEquals(1, sink.played.size());
        assertEquals(0.1f, sink.played.get(0).pitch(), 0.0001f);
    }

    @Test
    void loopReplaysWhileFlagStaysTrueAndStopsWhenFalse() {
        RecordingSink sink = new RecordingSink();
        AtomicBoolean active = new AtomicBoolean(true);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> active.get());

        player.play(playPayload(recipeWithLoop(), 1.0f, 0.0f));
        assertEquals(1, player.activeLoopCountForTests());
        assertEquals(0, sink.played.size(), "play queues until end-of-tick drain");

        player.tick();
        assertEquals(2, sink.played.size(), "initial queued play emits both layers");
        player.tick();
        assertEquals(4, sink.played.size(), "second tick should replay both layers");

        active.set(false);
        player.tick();
        assertEquals(0, player.activeLoopCountForTests(), "false flag should remove loop");
    }

    @Test
    void stopRemovesLoopAndCallsSinkStop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);
        player.play(playPayload(recipeWithLoop(), 1.0f, 0.0f));

        player.stop(new AudioEventPayload.StopSoundRecipe(42, 10));

        assertEquals(0, player.activeLoopCountForTests());
        assertEquals(42L, sink.stoppedInstanceId);
        assertEquals(10, sink.stoppedFadeOutTicks);
    }

    @Test
    void payloadFlagCanOwnLoopLifetimeUntilStop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);

        player.play(playPayloadWithFlag(recipeWithLoop(), "fauna_fuya_pressure:42"));
        assertEquals(1, player.activeLoopCountForTests());

        player.tick();
        assertEquals(2, sink.played.size(), "owned flag should keep initial loop play alive");
        player.tick();
        assertEquals(4, sink.played.size(), "owned flag should keep replaying loop");

        player.stop(new AudioEventPayload.StopSoundRecipe(42, 10));

        assertEquals(0, player.activeLoopCountForTests());
        assertFalse(EnvironmentAudioLoopState.isActive("fauna_fuya_pressure:42"));
    }

    @Test
    void tiandaoFlagFollowsPresenceStateInsteadOfStickyOwnedFlag() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(
            sink,
            SoundRecipePlayer::defaultFlagActiveForTests
        );

        TiandaoPresenceStore.replace(new TiandaoPresenceState(
            true,
            "pressure",
            48.0,
            "spawn",
            0.5,
            0x400800,
            0.08,
            0.35,
            0.95,
            200L
        ));
        player.play(playPayloadWithFlag(recipeWithLoop(), "tiandao:pressure"));
        player.tick();
        assertEquals(2, sink.played.size());

        TiandaoPresenceStore.replace(TiandaoPresenceState.empty());
        player.tick();

        assertEquals(0, player.activeLoopCountForTests());
        assertEquals(42L, sink.stoppedInstanceId);
    }

    /**
     * 重生残留受伤音回归：`heartbeat_low_hp` 第二层是 `minecraft:entity.player.hurt`，
     * server 发这条 loop 时带 payload flag `hp_below_20`，play() 会把它注册成 sticky flag。
     * 若 sticky 优先于内置 hp 谓词，while_flag 就永真 —— 血量回满（含重生回到 20%）后
     * 心跳仍每秒重放一次受伤音。这里锁住「hp flag 跟真实血量走」。
     */
    @Test
    void hpFlagFollowsCombatHudStateInsteadOfStickyOwnedFlag() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(
            sink,
            SoundRecipePlayer::defaultFlagActiveForTests
        );
        // 死亡/濒死：hp 0% → 心跳条件成立
        CombatHudStateStore.replace(CombatHudState.create(0.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));

        player.play(playPayloadWithFlag(recipeWithLoop(), "hp_below_20"));
        player.tick();
        assertEquals(2, sink.played.size(), "低血期间心跳应持续重放");

        // 重生：血量回到 REVIVE_HEALTH_FRACTION = 20%（阈值边界，不再算低血）
        CombatHudStateStore.replace(CombatHudState.create(0.2f, 1.0f, 1.0f, DerivedAttrFlags.none()));
        player.tick();

        assertEquals(
            0,
            player.activeLoopCountForTests(),
            "期望重生（hp 回到 20%）后心跳 loop 被摘掉，因为 hp_below_20 必须按真实血量判定"
                + "而不是被 payload 自注册的 sticky flag 短路成永真；实际 loop 仍活着"
                + "（= 玩家重生后每秒仍听到 entity.player.hurt）"
        );
        assertEquals(42L, sink.stoppedInstanceId, "摘 loop 时应对同一 instance 发 stop");
        assertFalse(
            EnvironmentAudioLoopState.isActive("hp_below_20"),
            "期望摘 loop 时把自注册的 hp_below_20 flag 一起撤掉，否则下一条 loop 又会读到 sticky 永真"
        );
        int playedAfterRevive = sink.played.size();
        player.tick();
        assertEquals(
            playedAfterRevive,
            sink.played.size(),
            "期望重生之后不再有任何心跳重放（一次都不许），实际又响了"
        );
    }

    /**
     * 同一治法要覆盖 recipe 自带的 `while_flag`（server 不带 payload flag 时用它）。
     */
    @Test
    void recipeWhileFlagHpBelow30FollowsCombatHudState() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(
            sink,
            SoundRecipePlayer::defaultFlagActiveForTests
        );
        CombatHudStateStore.replace(CombatHudState.create(0.1f, 1.0f, 1.0f, DerivedAttrFlags.none()));

        // recipeWithLoop() 的 while_flag = hp_below_30，且不带 payload flag → 无 sticky 注册
        player.play(playPayload(recipeWithLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(2, sink.played.size(), "hp 10% < 30% 时 loop 应重放");

        CombatHudStateStore.replace(CombatHudState.create(0.5f, 1.0f, 1.0f, DerivedAttrFlags.none()));
        player.tick();

        assertEquals(0, player.activeLoopCountForTests(), "hp 回到 50% 后 loop 应停");
        assertEquals(42L, sink.stoppedInstanceId);
    }

    /**
     * 反向锁：**没有**内置谓词的 flag（环境雾堤 / fauna 压迫 hum）仍由 server 的
     * play…stop 配对拥有生命周期，不能被这次改动顺手改成"立刻停"。
     */
    @Test
    void unknownFlagLoopStillOwnedByServerLifetimeUnderDefaultProvider() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(
            sink,
            SoundRecipePlayer::defaultFlagActiveForTests
        );

        player.play(playPayloadWithFlag(recipeWithLoop(), "fauna_fuya_pressure:42"));
        player.tick();
        player.tick();

        assertEquals(
            1,
            player.activeLoopCountForTests(),
            "期望无内置谓词的 flag 仍靠 sticky 注册维持 loop（生命周期归 server stop），实际被误停"
        );
        assertEquals(4, sink.played.size(), "sticky flag 期间 loop 应持续重放");

        player.stop(new AudioEventPayload.StopSoundRecipe(42, 0));
        assertEquals(0, player.activeLoopCountForTests());
        assertFalse(EnvironmentAudioLoopState.isActive("fauna_fuya_pressure:42"));
    }

    /**
     * server 侧显式 stop（本次修复新增的重生收尾路径）必须能收掉心跳 loop，
     * 即使此刻血量仍低（濒死中被强制停）。
     */
    @Test
    void serverStopEndsHeartbeatLoopEvenWhileHpStillLow() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(
            sink,
            SoundRecipePlayer::defaultFlagActiveForTests
        );
        CombatHudStateStore.replace(CombatHudState.create(0.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));
        player.play(playPayloadWithFlag(recipeWithLoop(), "hp_below_20"));
        player.tick();

        player.stop(new AudioEventPayload.StopSoundRecipe(42, 0));

        assertEquals(0, player.activeLoopCountForTests(), "server stop 后 loop 必须消失");
        assertEquals(0, sink.stoppedFadeOutTicks, "硬停不该带淡出尾音");
        int playedAtStop = sink.played.size();
        player.tick();
        assertEquals(playedAtStop, sink.played.size(), "stop 之后不许再有重放");
    }

    @Test
    void replacingSameLoopInstanceStopsPreviousSound() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);

        player.play(playPayloadWithFlag(recipeWithLoop(), "tiandao:pressure"));
        player.play(playPayloadWithFlag(recipeWithLoop(), "tiandao:pressure"));

        assertEquals(1, player.activeLoopCountForTests());
        assertEquals(42L, sink.stoppedInstanceId);
    }

    @Test
    void topNQueueKeepsThreeOneShotsAndOneLoopPerTick() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);

        player.play(playPayload(recipe("low", 10, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("mid", 50, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("high", 90, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("higher", 95, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("loop", 5, Optional.of(new AudioLoopConfig(20, "hp_below_30"))), 1.0f, 0.0f));

        player.tick();

        assertEquals(8, sink.played.size(), "four recipes with two layers each should play");
        assertEquals(0, sink.countRecipe("low"), "lowest one-shot should be dropped");
        assertEquals(2, sink.countRecipe("loop"), "one loop slot should be retained");
    }

    @Test
    void highPriorityPreemptsLowerSameCategoryLoop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);
        player.play(playPayload(recipe("heartbeat_low_hp", 70, Optional.of(new AudioLoopConfig(20, "hp_below_30"))), 1.0f, 0.0f));
        player.tick();

        player.play(playPayload(recipe("tribulation_wave_impact", 98, Optional.empty()), 1.0f, 0.0f));

        assertEquals(0, player.activeLoopCountForTests());
        assertEquals(42L, sink.stoppedInstanceId);
    }

    @Test
    void ambientVolumeDucksWhileCombatHudIsActive() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);
        CombatHudStateStore.replace(CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));

        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.393f, sink.played.get(0).volume(), 0.001f);
    }

    @Test
    void ambientDuckingReachesCombatTargetOverTwoSeconds() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);
        CombatHudStateStore.replace(CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));

        for (int i = 0; i < 40; i++) {
            player.tick();
        }
        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.12f, sink.played.get(0).volume(), 0.0001f);
    }

    @Test
    void busMixerKeepsCombatVolumeIndependentFromEnvironment() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        mixer.setVolume(AudioBus.COMBAT, 0.25f);
        mixer.setVolume(AudioBus.ENVIRONMENT, 1.0f);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());

        player.play(playPayload(recipe("combat_hit", 50, Optional.empty()), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.1f, sink.played.get(0).volume(), 0.0001f);
        assertEquals(AudioBus.COMBAT, recipe("combat_hit", 50, Optional.empty()).bus());
    }

    @Test
    void tribulationDucksEnvironmentBusOnly() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());
        player.setMusicState(MusicStateMachine.State.TRIBULATION);

        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.12f, sink.played.get(0).volume(), 0.0001f);
    }

    @Test
    void immersiveModeMutesUiBusUntilRestoreWindow() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        HudImmersionMode.setManualImmersive(true, 0L);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());

        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.0f, sink.played.get(0).volume(), 0.0001f);

        mixer.restoreUiForTicks(5);
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.4f, sink.played.get(2).volume(), 0.0001f);
    }

    @Test
    void combatEdgesRestoreUiBusTemporarilyInImmersiveMode() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        HudImmersionMode.setManualImmersive(true, 0L);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());

        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.0f, sink.played.get(0).volume(), 0.0001f);

        CombatHudStateStore.replace(CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.4f, sink.played.get(2).volume(), 0.0001f);

        for (int i = 0; i < 100; i++) {
            player.tick();
        }
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.0f, sink.played.get(4).volume(), 0.0001f);

        CombatHudStateStore.replace(CombatHudState.create(0.8f, 1.0f, 1.0f, DerivedAttrFlags.none()));
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.4f, sink.played.get(6).volume(), 0.0001f);
    }

    @Test
    void clearOnDisconnectStopsActiveLoopsDropsPendingFlagsAndResetsSessionFields() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);

        player.play(playPayloadWithFlag(recipeWithLoop(), "zone_env:old-loop"));
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        assertEquals(2, player.pendingCountForTests(), "前置：loop 与 one-shot 都应在旧 session pending 队列中");
        player.tick();
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        assertEquals(1, player.pendingCountForTests(), "前置：断线前必须留下尚未 drain 的 one-shot");
        assertEquals(1, player.activeLoopCountForTests(), "前置：旧 session 必须有活动 loop");
        assertTrue(player.tickForTests() > 0, "前置：旧 session tick 必须已推进");
        assertTrue(EnvironmentAudioLoopState.isActive("zone_env:old-loop"), "前置：loop 自有 flag 必须已注册");

        player.clearOnDisconnect();

        assertEquals(0, player.activeLoopCountForTests(), "断线必须移除全部活动 loop");
        assertEquals(0, player.pendingCountForTests(), "断线必须丢弃尚未 drain 的旧 session payload");
        assertFalse(EnvironmentAudioLoopState.isActive("zone_env:old-loop"), "断线必须撤销 loop 派生 flag");
        assertTrue(sink.stoppedIds.contains(42L), "断线必须向 sink 对旧 loop 发 hard stop");
        assertEquals(0, sink.fadeOutTicksFor(42L), "断线 stop 不能留下淡出尾音");
        assertEquals(0L, player.tickForTests(), "断线必须复位 session tick");
        assertEquals(1.0f, player.ambientVolumeFactorForTests(), 0.0001f,
            "断线必须复位战斗环境 ducking 系数");

        player.play(playPayloadWithFlag(recipeWithLoop(), "zone_env:new-loop"));
        player.tick();

        assertEquals(1, player.activeLoopCountForTests(), "新 session 必须可正常建立新 loop");
        assertEquals(1L, player.tickForTests(), "新 session tick 必须从零重新计数");
        assertTrue(EnvironmentAudioLoopState.isActive("zone_env:new-loop"), "新 session flag 必须可重新激活");
    }

    @Test
    void clearOnDisconnectResetsCombatDerivedDuckingForNewSession() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());
        CombatHudStateStore.replace(CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));
        for (int i = 0; i < 40; i++) {
            player.tick();
        }
        assertEquals(0.3f, player.ambientVolumeFactorForTests(), 0.0001f,
            "前置：旧 session 的 combat ducking 必须已达到目标");

        player.clearOnDisconnect();
        CombatHudStateStore.resetForTests();
        HudImmersionMode.setManualImmersive(true, 0L);
        player.tick();
        player.setMusicState(MusicStateMachine.State.TRIBULATION);
        mixer.restoreUiForTicks(100);
        player.clearOnDisconnect();
        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();

        assertEquals(1.0f, player.ambientVolumeFactorForTests(), 0.0001f,
            "新 session 首帧不得继承旧 session 的 combat ducking");
        assertEquals(0.4f, sink.played.get(0).volume(), 0.0001f,
            "新 session 环境层必须按未 duck 的基准音量播放，也不得继承旧 session music state");
        assertEquals(0.0f, sink.played.get(2).volume(), 0.0001f,
            "新 session UI 层必须按当前 immersive 状态计算，而非继承旧 session UI restore 窗口");
    }

    @Test
    void clearOnDisconnectCompletesLocalResetWhenSinkRuntimeOperationsFail() {
        RuntimeFailingSink sink = new RuntimeFailingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        SoundRecipePlayer player = new SoundRecipePlayer(
            sink,
            EnvironmentAudioLoopState::isActive,
            mixer,
            new AudioTelemetry()
        );
        player.play(playPayloadWithFlag(recipeWithLoop(), "zone_env:runtime-failure"));
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.setMusicState(MusicStateMachine.State.TRIBULATION);
        mixer.restoreUiForTicks(100);

        player.clearOnDisconnect();

        assertEquals(1, sink.stopAttempts, "即使 hard stop 抛 RuntimeException，也必须尝试停止旧 loop");
        assertEquals(1, sink.clearAttempts, "loop stop 失败后仍必须尝试 sink 全局清理");
        assertEquals(0, player.activeLoopCountForTests(), "sink 失败不得保留旧 session loop 引用");
        assertEquals(0, player.pendingCountForTests(), "sink 失败不得保留旧 session pending payload");
        assertFalse(EnvironmentAudioLoopState.isActive("zone_env:runtime-failure"),
            "sink 失败不得保留旧 session 派生 flag");
        assertEquals(0L, player.tickForTests(), "sink 失败仍必须复位 session tick");
        assertEquals(1.0f, player.ambientVolumeFactorForTests(), 0.0001f,
            "sink 失败仍必须复位 combat ducking");
        assertEquals(1.0f, mixer.effectiveVolume(AudioBus.ENVIRONMENT), 0.0001f,
            "sink 失败仍必须复位 session music state");

        player.clearOnDisconnect();
        assertEquals(1, sink.stopAttempts, "幂等清理不得重试已摘除的旧 loop 引用");
        assertEquals(2, sink.clearAttempts, "幂等清理仍可再次请求 sink 清理空状态");
    }

    @Test
    void clearOnDisconnectStillPropagatesSinkErrorAfterLocalReset() {
        ErrorFailingSink sink = new ErrorFailingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        player.play(playPayloadWithFlag(recipeWithLoop(), "zone_env:error"));
        player.tick();
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));

        AssertionError error = assertThrows(AssertionError.class, player::clearOnDisconnect);

        assertEquals("fatal stop", error.getMessage(), "Error 必须原样透传，不能按可恢复 RuntimeException 吞掉");
        assertEquals(0, player.activeLoopCountForTests(), "Error 前也必须先摘除旧 session loop 引用");
        assertEquals(0, player.pendingCountForTests(), "Error 前也必须先丢弃旧 session pending payload");
        assertFalse(EnvironmentAudioLoopState.isActive("zone_env:error"),
            "Error 前也必须先撤销旧 session 派生 flag");
        assertEquals(0L, player.tickForTests(), "Error 前也必须先复位 session tick");
        assertEquals(0, sink.clearAttempts, "fatal stop Error 应原样中止后续 sink 操作");
    }

    @Test
    void telemetryFlagsRecipeOverplayWindow() {
        AudioTelemetry telemetry = new AudioTelemetry(1_000L, 2);
        telemetry.record("hit_light", 1_000L);
        telemetry.record("hit_light", 1_100L);
        telemetry.record("hit_light", 1_200L);

        assertEquals(true, telemetry.isOverThreshold("hit_light", 1_200L));
        assertEquals(false, telemetry.isOverThreshold("hit_light", 2_500L));
    }

    private static AudioEventPayload.PlaySoundRecipe playPayload(AudioRecipe recipe, float volumeMul, float pitchShift) {
        return new AudioEventPayload.PlaySoundRecipe(
            recipe.id(),
            42,
            Optional.of(new AudioPosition(1, 64, -2)),
            Optional.empty(),
            volumeMul,
            pitchShift,
            recipe
        );
    }

    private static AudioEventPayload.PlaySoundRecipe playPayloadWithFlag(AudioRecipe recipe, String flag) {
        return new AudioEventPayload.PlaySoundRecipe(
            recipe.id(),
            42,
            Optional.of(new AudioPosition(1, 64, -2)),
            Optional.of(flag),
            1.0f,
            0.0f,
            recipe
        );
    }

    private static AudioRecipe recipeWithoutLoop() {
        return new AudioRecipe(
            "pill_consume",
            List.of(
                new AudioLayer(new Identifier("minecraft", "entity.generic.drink"), 0.4f, 1.0f, 0),
                new AudioLayer(new Identifier("minecraft", "block.brewing_stand.brew"), 0.3f, 1.2f, 5)
            ),
            Optional.empty(),
            40,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.VOICE,
            AudioBus.UI
        );
    }

    private static AudioRecipe recipeWithLoop() {
        return new AudioRecipe(
            "heartbeat_low_hp",
            recipeWithoutLoop().layers(),
            Optional.of(new AudioLoopConfig(2, "hp_below_30")),
            70,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.HOSTILE,
            AudioBus.COMBAT
        );
    }

    private static AudioRecipe lowPitchRecipe() {
        return new AudioRecipe(
            "ambient_north_wastes",
            List.of(new AudioLayer(new Identifier("minecraft", "weather.rain"), 0.08f, 0.1f, 0)),
            Optional.empty(),
            24,
            AudioAttenuation.ZONE_BROADCAST,
            AudioCategory.AMBIENT,
            AudioBus.ENVIRONMENT
        );
    }

    private static AudioRecipe ambientRecipe() {
        return new AudioRecipe(
            "tribulation_thunder_distant",
            recipeWithoutLoop().layers(),
            Optional.empty(),
            95,
            AudioAttenuation.WORLD_3D,
            AudioCategory.AMBIENT,
            AudioBus.ENVIRONMENT
        );
    }

    private static AudioRecipe recipe(String id, int priority, Optional<AudioLoopConfig> loop) {
        return new AudioRecipe(
            id,
            List.of(
                new AudioLayer(new Identifier("minecraft", "audio_test/" + id + "_a"), 0.4f, 1.0f, 0),
                new AudioLayer(new Identifier("minecraft", "audio_test/" + id + "_b"), 0.3f, 1.2f, 5)
            ),
            loop,
            priority,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.HOSTILE,
            AudioBus.COMBAT
        );
    }

    private static final class RecordingSink implements SoundSink {
        final List<AudioScheduledSound> played = new ArrayList<>();
        final List<Long> stoppedIds = new ArrayList<>();
        final List<Integer> stoppedFadeOutTickValues = new ArrayList<>();
        long stoppedInstanceId = -1;
        int stoppedFadeOutTicks = -1;

        @Override
        public boolean play(AudioScheduledSound sound) {
            played.add(sound);
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
            stoppedIds.add(instanceId);
            stoppedFadeOutTickValues.add(fadeOutTicks);
            stoppedInstanceId = instanceId;
            stoppedFadeOutTicks = fadeOutTicks;
        }

        int fadeOutTicksFor(long instanceId) {
            for (int index = stoppedIds.size() - 1; index >= 0; index--) {
                if (stoppedIds.get(index) == instanceId) {
                    return stoppedFadeOutTickValues.get(index);
                }
            }
            return -1;
        }

        long countRecipe(String recipeId) {
            return played.stream()
                .filter(sound -> sound.sound().getPath().contains(recipeId))
                .count();
        }
    }

    private static final class RuntimeFailingSink implements SoundSink {
        int stopAttempts;
        int clearAttempts;

        @Override
        public boolean play(AudioScheduledSound sound) {
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
            stopAttempts++;
            throw new IllegalStateException("runtime stop");
        }

        @Override
        public void clearOnDisconnect() {
            clearAttempts++;
            throw new IllegalStateException("runtime clear");
        }
    }

    private static final class ErrorFailingSink implements SoundSink {
        int clearAttempts;

        @Override
        public boolean play(AudioScheduledSound sound) {
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
            throw new AssertionError("fatal stop");
        }

        @Override
        public void clearOnDisconnect() {
            clearAttempts++;
        }
    }
}
