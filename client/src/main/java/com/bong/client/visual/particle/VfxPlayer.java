package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;

/**
 * plan-particle-system-v1 §2.7：一次粒子事件的完整编排入口。
 *
 * <p>实现方要把 {@link VfxEventPayload.SpawnParticle} 翻译成具体的粒子生成（可以是多个
 * {@link BongLineParticle} / Ribbon / GroundDecal 组合，也可以配合音效或短暂 HUD 叠色）。
 *
 * <p>{@code client} 参数恒为 {@link MinecraftClient#getInstance()}，抽出来是为了让实现可 mock。
 */
@FunctionalInterface
public interface VfxPlayer {
    void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload);
}
