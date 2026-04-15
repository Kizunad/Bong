package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.network.VfxParticleBridge;
import net.minecraft.client.MinecraftClient;

/**
 * {@link VfxParticleBridge} 的 MC-in-process 实现：查 {@link VfxRegistry}，找到就派发。
 *
 * <p>与 {@link com.bong.client.animation.ClientAnimationBridge} 同级：封装"把强类型 payload 交给
 * MC 主线程"这一动作。两者的语义差异：
 * <ul>
 *   <li>Animation bridge：在 network 回调里已经派发到 main thread（见 {@code BongNetworkHandler}
 *       里 {@code client.execute(...)}），此处直接同步走</li>
 *   <li>Particle bridge：同理，要求调用方保证在主线程执行。{@link MinecraftClient#particleManager}
 *       不能在网络线程调用</li>
 * </ul>
 */
public final class BongVfxParticleBridge implements VfxParticleBridge {
    private final VfxRegistry registry;

    public BongVfxParticleBridge() {
        this(VfxRegistry.instance());
    }

    /** 主要给单测用：注入自定义 registry。 */
    public BongVfxParticleBridge(VfxRegistry registry) {
        this.registry = registry;
    }

    @Override
    public boolean spawnParticle(VfxEventPayload.SpawnParticle payload) {
        return registry.lookup(payload.eventId()).map(player -> {
            MinecraftClient client = MinecraftClient.getInstance();
            // client 在单测 / pre-init 时可能为 null（registry 注册完但 MC 还没启动）；
            // 这时候当作"未就绪"返回 false，router 记 bridgeMiss。
            if (client == null) {
                return false;
            }
            player.play(client, payload);
            return true;
        }).orElse(false);
    }
}
