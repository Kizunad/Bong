package com.bong.client.visual.particle;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.world.ClientWorld;

import java.util.List;

/**
 * 把 {@link SkillParticleSpawn} 描述符实例化成真粒子并投进 {@code particleManager}
 * —— plan-skill-anim-fidelity-v1 §P5.1（review r1 #3 引入）。
 *
 * <p>这是三个招式 player 里<b>唯一</b>接触 {@code MinecraftClient} 的地方；「发射什么」全部前移到
 * 各 player 的纯函数 {@code plan(payload)}，于是单测能逐颗断言粒子参数而不需要 MC 运行时。
 *
 * <p>逐形态的构造归各描述符自己的 {@link SkillParticleSpawn#createParticle}（新增形态不实现即
 * 编译失败）；这里只施加对所有形态同构的两项——颜色与 {@code maxAge}，它们在 vanilla
 * {@code Particle} 上本就是 public，没必要让 6 个变体各抄一遍。
 */
final class SkillParticleSpawner {

    private SkillParticleSpawner() {
    }

    static void spawnAll(
        MinecraftClient client,
        ClientWorld world,
        List<SkillParticleSpawn> spawns
    ) {
        for (SkillParticleSpawn spawn : spawns) {
            Particle particle = spawn.createParticle(world);
            particle.setColor(
                ((spawn.rgb() >> 16) & 0xFF) / 255f,
                ((spawn.rgb() >> 8) & 0xFF) / 255f,
                (spawn.rgb() & 0xFF) / 255f
            );
            particle.setMaxAge(spawn.maxAge());
            client.particleManager.addParticle(particle);
        }
    }
}
