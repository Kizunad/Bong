package com.bong.client.audio;

import com.bong.client.BongClient;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.math.random.Random;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiConsumer;
import java.util.function.Consumer;
import java.util.function.Supplier;

public final class MinecraftSoundSink implements SoundSink {
    private final Map<Long, List<FadeableSoundInstance>> activeByInstance = new ConcurrentHashMap<>();
    private final Supplier<MinecraftClient> clientSupplier;
    private final BiConsumer<MinecraftClient, FadeableSoundInstance> hardStopBridge;

    public MinecraftSoundSink() {
        this(MinecraftClient::getInstance, MinecraftSoundSink::stopWithClientSoundManager);
    }

    MinecraftSoundSink(
        Supplier<MinecraftClient> clientSupplier,
        BiConsumer<MinecraftClient, FadeableSoundInstance> hardStopBridge
    ) {
        this.clientSupplier = clientSupplier;
        this.hardStopBridge = hardStopBridge;
    }

    @Override
    public boolean play(AudioScheduledSound sound) {
        MinecraftClient client = clientSupplier.get();
        if (client == null || client.getSoundManager() == null) {
            return false;
        }

        boolean relative = sound.attenuation() == AudioAttenuation.PLAYER_LOCAL
            || sound.attenuation() == AudioAttenuation.SELF;
        SoundInstance.AttenuationType attenuationType = relative
            ? SoundInstance.AttenuationType.NONE
            : SoundInstance.AttenuationType.LINEAR;
        AudioPosition pos = sound.pos().orElseGet(() -> {
            if (client.player != null) {
                return new AudioPosition(
                    (int) Math.floor(client.player.getX()),
                    (int) Math.floor(client.player.getY()),
                    (int) Math.floor(client.player.getZ())
                );
            }
            return new AudioPosition(0, 0, 0);
        });

        FadeableSoundInstance instance = new FadeableSoundInstance(
            sound.sound(),
            toMinecraftCategory(sound.category()),
            sound.volume(),
            sound.pitch(),
            Random.create(),
            false,
            0,
            attenuationType,
            pos.x(),
            pos.y(),
            pos.z(),
            relative
        );
        pruneFinished();
        activeByInstance.computeIfAbsent(sound.instanceId(), ignored -> new ArrayList<>()).add(instance);
        client.getSoundManager().play(instance, sound.delayTicks());
        return true;
    }

    @Override
    public void stop(long instanceId, int fadeOutTicks) {
        MinecraftClient client = clientSupplier.get();
        if (client == null || client.getSoundManager() == null) {
            return;
        }
        stopInternal(instanceId, fadeOutTicks, instance -> client.getSoundManager().stop(instance));
    }

    @Override
    public void clearOnDisconnect() {
        MinecraftClient client = clientSupplier.get();
        clearOnDisconnectInternal(instance -> hardStopBridge.accept(client, instance));
    }

    private static void stopWithClientSoundManager(
        MinecraftClient client,
        FadeableSoundInstance instance
    ) {
        if (client != null && client.getSoundManager() != null) {
            client.getSoundManager().stop(instance);
        }
    }

    /**
     * 断线时硬停所有本会话实例（包括尚在 vanilla 延迟队列中的 layer）并清空索引。
     *
     * <p>与 {@link #stopInternal(long, int, Consumer)} 的单 instance hard-stop 相同，先将
     * 实例自身标记为完成、音量归零；即使 sound manager 已不可用，延迟层之后被取出时也
     * 没有可听输出。随后无条件清掉 {@code activeByInstance}，不能把旧 session 的 id 留给
     * 新 session。
     */
    void clearOnDisconnectInternal(Consumer<FadeableSoundInstance> hardStopper) {
        for (List<FadeableSoundInstance> instances : activeByInstance.values()) {
            for (FadeableSoundInstance instance : instances) {
                instance.beginFadeOut(0);
                try {
                    hardStopper.accept(instance);
                } catch (RuntimeException exception) {
                    BongClient.LOGGER.error(
                        "Failed to hard-stop sound instance while clearing disconnect state",
                        exception
                    );
                }
            }
        }
        activeByInstance.clear();
    }

    /**
     * stop 的核心逻辑，与 MinecraftClient 解耦以便 headless 单测。
     *
     * <p>淡出中的实例必须保留在 {@code activeByInstance} 索引里：同一 {@code instanceId}
     * 在淡出期间再次收到 stop 时，硬停（{@code fadeOutTicks<=0}）要能立即掐掉正在淡出的
     * 声音，更短的淡出窗口也要能覆盖正在进行的淡出——只有硬停分支才移除映射；
     * 淡出完成（{@code isDone()}）的残留条目由 {@link #pruneFinished()} 在后续
     * play/stop 时惰性清理，防止长会话下映射泄漏。
     */
    void stopInternal(long instanceId, int fadeOutTicks, Consumer<FadeableSoundInstance> hardStopper) {
        if (fadeOutTicks <= 0) {
            List<FadeableSoundInstance> instances = activeByInstance.remove(instanceId);
            if (instances != null) {
                for (FadeableSoundInstance instance : instances) {
                    // 硬停要连**尚未开声的延迟层**一起掐掉：layer 的 delay_ticks 是交给
                    // vanilla `SoundSystem#soundsToPlay` 排队的，`soundManager.stop()`
                    // 只能停已起声的 source，排队中的实例到点仍会被取出播放。
                    // 先把实例自身音量归零 + 标完成（beginFadeOut(0) 语义），这样即便引擎
                    // 之后从延迟队列取出它，getVolume() 也是 0（无可听输出）且 isDone()
                    // 为真会被立即摘除。实机症状：心跳 loop 的 entity.player.hurt 层
                    // （delay_ticks=2）曾在重生收 loop 之后还补响一声。
                    instance.beginFadeOut(0);
                    hardStopper.accept(instance);
                }
            }
        } else {
            List<FadeableSoundInstance> instances = activeByInstance.get(instanceId);
            if (instances != null) {
                for (FadeableSoundInstance instance : instances) {
                    // 交给 Minecraft 自身的 TickableSoundInstance 循环逐 tick 渐弱；
                    // 淡出结束（isDone()==true）后引擎会自己摘除 channel，无需我们再调用
                    // soundManager.stop()（那样会打断刚起步的淡出，变回硬切）。
                    instance.beginFadeOut(fadeOutTicks);
                }
            }
        }
        pruneFinished();
    }

    /** 清掉已淡出完毕（isDone）的实例与空条目，防止淡出路径的映射无限增长。 */
    private void pruneFinished() {
        activeByInstance.values().removeIf(instances -> {
            instances.removeIf(FadeableSoundInstance::isDone);
            return instances.isEmpty();
        });
    }

    /** 测试钩子：绕过需要活体 MinecraftClient 的 play()，直接注册实例进索引。 */
    void registerInstanceForTests(long instanceId, FadeableSoundInstance instance) {
        activeByInstance.computeIfAbsent(instanceId, ignored -> new ArrayList<>()).add(instance);
    }

    /** 测试钩子：当前索引中的 instanceId 条目数。 */
    int trackedInstanceCountForTests() {
        return activeByInstance.size();
    }

    private static SoundCategory toMinecraftCategory(AudioCategory category) {
        return switch (category) {
            case MASTER -> SoundCategory.MASTER;
            case PLAYERS -> SoundCategory.PLAYERS;
            case HOSTILE -> SoundCategory.HOSTILE;
            case AMBIENT -> SoundCategory.AMBIENT;
            case VOICE -> SoundCategory.VOICE;
            case BLOCKS -> SoundCategory.BLOCKS;
        };
    }
}
