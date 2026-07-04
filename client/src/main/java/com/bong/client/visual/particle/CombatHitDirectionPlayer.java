package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class CombatHitDirectionPlayer implements VfxPlayer {
    public static final Identifier HIT = new Identifier("bong", "combat_hit");
    public static final Identifier PARRY = new Identifier("bong", "combat_parry");
    // plan-combat-hit-location-v1 P3 — 四肢命中复用 HIT 分支（血色三线，颜色/lifetime 由
    // server payload 差异化），头部命中走专属暴击星形 burst 分支。
    public static final Identifier LIMB = new Identifier("bong", "combat_hit_limb");
    public static final Identifier HEAD_CRIT = new Identifier("bong", "combat_hit_head_crit");

    /** HIT/LIMB 共享同一条三线分支；PARRY 与 HEAD_CRIT 各自独立径向 burst 分支。 */
    public enum Kind { HIT, PARRY, HEAD_CRIT }

    private final Kind kind;

    public CombatHitDirectionPlayer(boolean parry) {
        this(parry ? Kind.PARRY : Kind.HIT);
    }

    public CombatHitDirectionPlayer(Kind kind) {
        this.kind = kind;
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double[] dir = GameplayVfxUtil.direction(payload, new double[] { 1.0, 0.0, 0.0 });
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb());
        int maxAge = GameplayVfxUtil.duration(payload, fallbackMaxAge());

        if (kind == Kind.PARRY) {
            int count = GameplayVfxUtil.count(payload, 8, 1, 18);
            for (int i = 0; i < count; i++) {
                double vx = (world.random.nextDouble() - 0.5) * 0.18;
                double vy = 0.04 + world.random.nextDouble() * 0.14;
                double vz = (world.random.nextDouble() - 0.5) * 0.18;
                GameplayVfxUtil.spawnSprite(client, world, BongParticles.tribulationSparkSprites,
                    ox, oy, oz, vx, vy, vz, rgb, 0.85f, maxAge, 0.10f);
            }
            return;
        }

        if (kind == Kind.HEAD_CRIT) {
            // 暴击星形 burst：径向均匀分布（非沿命中法线），与四肢的沿法线三线形成
            // 视觉区分——头部命中要"炸开"而不是"划过"。复用既有 tribulationSparkSprites
            // 贴图（无新资产），染白金色区别于 PARRY 的默认蓝色。
            int count = GameplayVfxUtil.count(payload, 6, 1, 12);
            for (int i = 0; i < count; i++) {
                double angle = (Math.PI * 2.0 / count) * i;
                double vx = Math.cos(angle) * 0.16;
                double vy = 0.10 + world.random.nextDouble() * 0.08;
                double vz = Math.sin(angle) * 0.16;
                GameplayVfxUtil.spawnSprite(client, world, BongParticles.tribulationSparkSprites,
                    ox, oy, oz, vx, vy, vz, rgb, 0.95f, maxAge, 0.14f);
            }
            return;
        }

        // Kind.HIT：既服务于通用胸/腹/背命中（bong:combat_hit），也服务于四肢命中
        // （bong:combat_hit_limb，同一分支，颜色/lifetime 由 server payload 差异化）。
        for (int i = -1; i <= 1; i++) {
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.swordSlashArcSprites,
                ox,
                oy + i * 0.08,
                oz,
                dir[0] * 0.7,
                0.05,
                dir[2] * 0.7,
                rgb,
                0.8f,
                maxAge,
                0.18
            );
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case PARRY -> 0x4488FF;
            case HEAD_CRIT -> 0xFFE9A0;
            case HIT -> 0xFF3344;
        };
    }

    private int fallbackMaxAge() {
        return switch (kind) {
            case PARRY -> 16;
            case HEAD_CRIT -> 8;
            case HIT -> 12;
        };
    }
}
