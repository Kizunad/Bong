package com.bong.client.scroll;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.network.AudioEventPayload;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicLong;

/**
 * plan-scroll-reading-v1 P2 — 卷轴阅读三条差异化 audio recipe（开卷 / 翻页 / 合卷）。
 *
 * <p>三者全部走「client 内联构造 {@link AudioRecipe} 经 {@link SoundRecipePlayer} 播放」的
 * 既有惯例（同 {@code PackOperationVfxPlayer#audioRecipe}、{@code HomeSequence#homeAudioRecipe}），
 * 委托属性均取 {@link AudioAttenuation#PLAYER_LOCAL}（阅读是私人交互，不广播给附近玩家）。
 * committed JSON 资产 {@code assets/bong/audio_recipes/scroll_read_*.json} 作规格 + 测试对拍
 * （{@code ScrollReadAudioRecipeAssetTest} 锁住三者互不相同 + 与 JSON 逐字段一致）。
 *
 * <ul>
 *   <li><b>开卷</b>：{@code ScrollOpenGlowPlayer} 收到 server 同帧 emit 的
 *       {@code bong:scroll_open_glow} 粒子事件时一并触发（单事件驱动"粒子+音效"组合反馈，
 *       同 {@code PackOperationVfxPlayer} 模式）。</li>
 *   <li><b>翻页 / 合卷</b>：纯 client 本地 UI 动作（{@code ScrollReadScreen} 翻页按钮 /
 *       关闭），无需 server 往返。</li>
 * </ul>
 */
public final class ScrollReadAudio {
    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(44_000L);

    private ScrollReadAudio() {}

    public static AudioRecipe openRecipe() {
        return new AudioRecipe(
            "scroll_read_open",
            List.of(new AudioLayer(new Identifier("minecraft", "item.book.page_turn"), 0.8f, 0.9f, 0)),
            Optional.empty(),
            40,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.VOICE,
            AudioBus.UI
        );
    }

    public static AudioRecipe pageTurnRecipe() {
        return new AudioRecipe(
            "scroll_read_page_turn",
            List.of(new AudioLayer(new Identifier("minecraft", "item.book.page_turn"), 0.6f, 1.1f, 0)),
            Optional.empty(),
            35,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.VOICE,
            AudioBus.UI
        );
    }

    public static AudioRecipe closeRecipe() {
        return new AudioRecipe(
            "scroll_read_close",
            List.of(new AudioLayer(new Identifier("minecraft", "item.book.put"), 0.7f, 1.0f, 0)),
            Optional.empty(),
            38,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.VOICE,
            AudioBus.UI
        );
    }

    public static void playOpen() {
        play(openRecipe());
    }

    public static void playPageTurn() {
        play(pageTurnRecipe());
    }

    public static void playClose() {
        play(closeRecipe());
    }

    private static void play(AudioRecipe recipe) {
        SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
            recipe.id(),
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            recipe
        ));
    }
}
