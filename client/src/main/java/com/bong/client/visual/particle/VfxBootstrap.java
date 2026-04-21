package com.bong.client.visual.particle;

/**
 * 集中注册 {@link VfxRegistry} 的默认 event → player 绑定（plan-particle-system-v1 §4.4 首批事件）。
 *
 * <p>Phase 1 只接一个 {@code bong:sword_qi_slash} 用于端到端演示。后续 phase 按 plan §4.4 表格逐个加：
 * {@code breakthrough_pillar} / {@code enlightenment_aura} / {@code tribulation_lightning} / ...
 *
 * <p>调用点：{@code BongClient#onInitializeClient}。
 */
public final class VfxBootstrap {
    private VfxBootstrap() {
    }

    public static void registerDefaults() {
        VfxRegistry registry = VfxRegistry.instance();
        registry.register(SwordQiSlashPlayer.EVENT_ID,           new SwordQiSlashPlayer());
        registry.register(BreakthroughPillarPlayer.EVENT_ID,     new BreakthroughPillarPlayer());
        registry.register(EnlightenmentAuraPlayer.EVENT_ID,      new EnlightenmentAuraPlayer());
        registry.register(TribulationLightningPlayer.EVENT_ID,   new TribulationLightningPlayer());
        registry.register(FormationActivatePlayer.EVENT_ID,      new FormationActivatePlayer());
        registry.register(DeathSoulDissipatePlayer.EVENT_ID,     new DeathSoulDissipatePlayer());
    }
}
