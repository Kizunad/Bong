package com.bong.client.craft;

import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;

import java.util.Objects;
import java.util.function.IntConsumer;

/**
 * CraftScreen / WorkbenchScreen 共用的 completed/failed 玩家反馈契约。
 *
 * <p>抽出可测 seam，避免单测绑定 Screen 私有字段或完整 owo UI 树，同时保证两屏
 * 使用同一套顺序：completed 先 flashTicks=6 → 完成音 → refresh；failed 只 refresh
 * 且不播放完成音。player 缺失时静默跳过音效，不崩溃。</p>
 */
public final class CraftOutcomeFeedback {
    public static final int COMPLETED_FLASH_TICKS = 6;

    @FunctionalInterface
    public interface CompleteSoundPlayer {
        void play();
    }

    private CraftOutcomeFeedback() {
    }

    /**
     * 应用 outcome 反馈副作用。
     *
     * @param event          store 同步推送的 outcome
     * @param flashTicksSink completed 时写入 flashTicks（通常 = {@link #COMPLETED_FLASH_TICKS}）
     * @param completeSound  completed 时调用；回调负责 player 缺失时静默 no-op；failed 不调用
     * @param refresh        两种 outcome 都要触发的 UI 刷新（顺序上位于 flash/sound 之后）
     */
    public static void apply(
        CraftStore.CraftOutcomeEvent event,
        IntConsumer flashTicksSink,
        CompleteSoundPlayer completeSound,
        Runnable refresh
    ) {
        Objects.requireNonNull(event, "event");
        Objects.requireNonNull(flashTicksSink, "flashTicksSink");
        Objects.requireNonNull(completeSound, "completeSound");
        Objects.requireNonNull(refresh, "refresh");

        apply(CraftOutcomeView.from(event), flashTicksSink, completeSound, refresh);
    }

    /** 应用已经脱离 Store 的 UI outcome 投影。 */
    public static void apply(
        CraftOutcomeView event,
        IntConsumer flashTicksSink,
        CompleteSoundPlayer completeSound,
        Runnable refresh
    ) {
        Objects.requireNonNull(event, "event");
        Objects.requireNonNull(flashTicksSink, "flashTicksSink");
        Objects.requireNonNull(completeSound, "completeSound");
        Objects.requireNonNull(refresh, "refresh");

        if (event.kind() == CraftOutcomeView.Kind.COMPLETED) {
            flashTicksSink.accept(COMPLETED_FLASH_TICKS);
            completeSound.play();
        }
        refresh.run();
    }

    /** ViewModel 入口：屏幕不需要认识 Store 的事件实现类型。 */
    public static void apply(
        CraftScreenViewModel model,
        IntConsumer flashTicksSink,
        CompleteSoundPlayer completeSound,
        Runnable refresh
    ) {
        Objects.requireNonNull(model, "model must not be null");
        model.latestOutcome().ifPresent(event -> apply(event, flashTicksSink, completeSound, refresh));
    }

    /** 生产默认完成音：仅当 client.player 存在时播放，恰好一声 LEVELUP。 */
    public static void playDefaultCompleteSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.ENTITY_PLAYER_LEVELUP, 0.2F, 1.5F);
        }
    }
}
