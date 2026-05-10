package com.bong.client.visual.particle;

import net.fabricmc.fabric.api.client.particle.v1.ParticleFactoryRegistry;
import net.fabricmc.fabric.api.particle.v1.FabricParticleTypes;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.particle.DefaultParticleType;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;

/**
 * Bong 自定义粒子注册入口（plan-particle-system-v1 §4.1）。
 *
 * <p>每个粒子贴图通过 Fabric 的 ParticleType + Factory 机制挂到 MC 粒子 atlas。
 * VfxPlayer 直接 new 粒子实例 + setSpritePublic 注入 sprite，
 * 以保留 color / alpha / shape 的 per-instance 可控性——MC 的 Factory 签名
 * 只接 velocity，其他参数用 default 的话 VfxPlayer 无法表达 strength/color。
 *
 * <p>所以这里同时：
 * <ol>
 *   <li>注册 ParticleType 让 atlas 把贴图烘进 PARTICLE_SHEET_TRANSLUCENT</li>
 *   <li>注册 Factory 时把 SpriteProvider 缓存到 static 字段，给 VfxPlayer 用</li>
 * </ol>
 */
public final class BongParticles {

    // 9 种粒子（plan-particle-system-v1 §4.1）。
    public static final DefaultParticleType SWORD_QI_TRAIL      = FabricParticleTypes.simple();
    public static final DefaultParticleType SWORD_SLASH_ARC     = FabricParticleTypes.simple();
    public static final DefaultParticleType QI_AURA             = FabricParticleTypes.simple();
    public static final DefaultParticleType RUNE_CHAR           = FabricParticleTypes.simple();
    public static final DefaultParticleType LINGQI_RIPPLE       = FabricParticleTypes.simple();
    public static final DefaultParticleType BREAKTHROUGH_PILLAR = FabricParticleTypes.simple();
    public static final DefaultParticleType ENLIGHTENMENT_DUST  = FabricParticleTypes.simple();
    public static final DefaultParticleType TRIBULATION_SPARK   = FabricParticleTypes.simple();
    public static final DefaultParticleType FLYING_SWORD_TRAIL  = FabricParticleTypes.simple();
    public static final DefaultParticleType VORTEX_SPIRAL       = FabricParticleTypes.simple();
    public static final DefaultParticleType DUGU_DARK_GREEN_MIST = FabricParticleTypes.simple();
    public static final DefaultParticleType DUGU_TAINT_PULSE     = FabricParticleTypes.simple();
    public static final DefaultParticleType DUGU_REVERSE_BURST   = FabricParticleTypes.simple();
    public static final DefaultParticleType CLOUD_DUST           = FabricParticleTypes.simple();

    // SpriteProvider 缓存，由 Factory 注册回调注入，VfxPlayer 通过它取 sprite。
    public static volatile SpriteProvider swordQiTrailSprites;
    public static volatile SpriteProvider swordSlashArcSprites;
    public static volatile SpriteProvider qiAuraSprites;
    public static volatile SpriteProvider runeCharSprites;           // 多 variant（5 字 × 4 体 = 20）
    public static volatile SpriteProvider lingqiRippleSprites;
    public static volatile SpriteProvider breakthroughPillarSprites;
    public static volatile SpriteProvider enlightenmentDustSprites;
    public static volatile SpriteProvider tribulationSparkSprites;
    public static volatile SpriteProvider flyingSwordTrailSprites;
    public static volatile SpriteProvider vortexSpiralSprites;
    public static volatile SpriteProvider duguDarkGreenMistSprites;
    public static volatile SpriteProvider duguTaintPulseSprites;
    public static volatile SpriteProvider duguReverseBurstSprites;
    public static volatile SpriteProvider cloudDustSprites;

    private BongParticles() {
    }

    /** Common 侧：注册 ParticleType 到 registry。 */
    public static void register() {
        reg("sword_qi_trail",      SWORD_QI_TRAIL);
        reg("sword_slash_arc",     SWORD_SLASH_ARC);
        reg("qi_aura",             QI_AURA);
        reg("rune_char",           RUNE_CHAR);
        reg("lingqi_ripple",       LINGQI_RIPPLE);
        reg("breakthrough_pillar", BREAKTHROUGH_PILLAR);
        reg("enlightenment_dust",  ENLIGHTENMENT_DUST);
        reg("tribulation_spark",   TRIBULATION_SPARK);
        reg("flying_sword_trail",  FLYING_SWORD_TRAIL);
        reg("vortex_spiral",       VORTEX_SPIRAL);
        reg("dugu_dark_green_mist", DUGU_DARK_GREEN_MIST);
        reg("dugu_taint_pulse",     DUGU_TAINT_PULSE);
        reg("dugu_reverse_burst",   DUGU_REVERSE_BURST);
        reg("cloud256_dust",        CLOUD_DUST);
    }

    /** Client 侧：注册 Factory，抓住 SpriteProvider 引用。 */
    public static void registerClient() {
        ParticleFactoryRegistry pfr = ParticleFactoryRegistry.getInstance();
        pfr.register(SWORD_QI_TRAIL,      provider -> { swordQiTrailSprites      = provider; return lineFactory(provider); });
        pfr.register(SWORD_SLASH_ARC,     provider -> { swordSlashArcSprites     = provider; return lineFactory(provider); });
        pfr.register(BREAKTHROUGH_PILLAR, provider -> { breakthroughPillarSprites = provider; return lineFactory(provider); });
        pfr.register(TRIBULATION_SPARK,   provider -> { tribulationSparkSprites  = provider; return lineFactory(provider); });
        pfr.register(FLYING_SWORD_TRAIL,  provider -> { flyingSwordTrailSprites  = provider; return ribbonFactory(provider); });
        pfr.register(LINGQI_RIPPLE,       provider -> { lingqiRippleSprites      = provider; return groundDecalFactory(provider); });
        pfr.register(QI_AURA,             provider -> { qiAuraSprites            = provider; return spriteFactory(provider); });
        pfr.register(RUNE_CHAR,           provider -> { runeCharSprites          = provider; return spriteFactory(provider); });
        pfr.register(ENLIGHTENMENT_DUST,  provider -> { enlightenmentDustSprites = provider; return spriteFactory(provider); });
        pfr.register(VORTEX_SPIRAL,       provider -> { vortexSpiralSprites      = provider; return vortexSpiralFactory(provider); });
        pfr.register(DUGU_DARK_GREEN_MIST, provider -> { duguDarkGreenMistSprites = provider; return spriteFactory(provider); });
        pfr.register(DUGU_TAINT_PULSE,     provider -> { duguTaintPulseSprites    = provider; return groundDecalFactory(provider); });
        pfr.register(DUGU_REVERSE_BURST,   provider -> { duguReverseBurstSprites  = provider; return lineFactory(provider); });
        pfr.register(CLOUD_DUST,           provider -> { cloudDustSprites          = provider; return spriteFactory(provider); });
    }

    private static void reg(String id, DefaultParticleType type) {
        Registry.register(Registries.PARTICLE_TYPE, new Identifier("bong", id), type);
    }

    // ---- Factory 辅助 ---------------------------------------------------------

    private static net.minecraft.client.particle.ParticleFactory<DefaultParticleType> lineFactory(SpriteProvider provider) {
        return (type, world, x, y, z, vx, vy, vz) -> {
            BongLineParticle p = new BongLineParticle(world, x, y, z, vx, vy, vz);
            p.setSpritePublic(provider.getSprite(world.random));
            return p;
        };
    }

    private static net.minecraft.client.particle.ParticleFactory<DefaultParticleType> ribbonFactory(SpriteProvider provider) {
        return (type, world, x, y, z, vx, vy, vz) -> {
            BongRibbonParticle p = new BongRibbonParticle(world, x, y, z, vx, vy, vz);
            p.setSpritePublic(provider.getSprite(world.random));
            return p;
        };
    }

    private static net.minecraft.client.particle.ParticleFactory<DefaultParticleType> groundDecalFactory(SpriteProvider provider) {
        return (type, world, x, y, z, vx, vy, vz) -> {
            BongGroundDecalParticle p = new BongGroundDecalParticle(world, x, y, z);
            p.setSpritePublic(provider.getSprite(world.random));
            return p;
        };
    }

    private static net.minecraft.client.particle.ParticleFactory<DefaultParticleType> spriteFactory(SpriteProvider provider) {
        return (type, world, x, y, z, vx, vy, vz) -> {
            BongSpriteParticle p = new BongSpriteParticle(world, x, y, z, vx, vy, vz);
            p.setSpritePublic(provider.getSprite(world.random));
            return p;
        };
    }

    private static net.minecraft.client.particle.ParticleFactory<DefaultParticleType> vortexSpiralFactory(SpriteProvider provider) {
        return (type, world, x, y, z, vx, vy, vz) -> {
            VortexSpiralParticle p = new VortexSpiralParticle(world, x, y, z, vx, vy, vz, x, y, z);
            p.setSpritePublic(provider.getSprite(world.random));
            return p;
        };
    }
}
