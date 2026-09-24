package com.bong.client.audio;

public interface SoundSink {
    boolean play(AudioScheduledSound sound);

    default void stop(long instanceId, int fadeOutTicks) {
    }

    /**
     * 断线边界：丢弃本会话仍由 sink 跟踪的声音实例。
     *
     * <p>默认 no-op 保持注入式非 Minecraft sink 的既有接线；生产
     * {@link MinecraftSoundSink} 会把活动与延迟层全部硬停。
     */
    default void clearOnDisconnect() {
    }
}
