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
        registry.register(TribulationOmenCloudPlayer.EVENT_ID,   new TribulationOmenCloudPlayer());
        registry.register(TribulationBoundaryPlayer.EVENT_ID,    new TribulationBoundaryPlayer());
        registry.register(RealmCollapseBoundaryPlayer.EVENT_ID,  new RealmCollapseBoundaryPlayer());
        registry.register(FormationActivatePlayer.EVENT_ID,      new FormationActivatePlayer());
        registry.register(DeathSoulDissipatePlayer.EVENT_ID,     new DeathSoulDissipatePlayer());
        registry.register(FlyingSwordDemoPlayer.EVENT_ID,        new FlyingSwordDemoPlayer());
        registry.register(FormationCoreDemoPlayer.EVENT_ID,      new FormationCoreDemoPlayer());
        registry.register(BurstMeridianBengQuanPlayer.EVENT_ID,  new BurstMeridianBengQuanPlayer());
        registry.register(ChargingOrbVfx.EVENT_ID,               new ChargingOrbVfx());
        registry.register(ReleaseLightningVfx.EVENT_ID,          new ReleaseLightningVfx());
        registry.register(ExhaustedGreyMistVfx.EVENT_ID,         new ExhaustedGreyMistVfx());
        registry.register(FrostBreathPlayer.EVENT_ID,            new FrostBreathPlayer());
        YidaoPeacePulsePlayer yidao = new YidaoPeacePulsePlayer();
        registry.register(YidaoPeacePulsePlayer.MERIDIAN_REPAIR,       yidao);
        registry.register(YidaoPeacePulsePlayer.CONTAM_PURGE,          yidao);
        registry.register(YidaoPeacePulsePlayer.EMERGENCY_RESUSCITATE, yidao);
        registry.register(YidaoPeacePulsePlayer.LIFE_EXTENSION,        yidao);
        registry.register(YidaoPeacePulsePlayer.MASS_MERIDIAN_REPAIR,  yidao);
        registry.register(
            new net.minecraft.util.Identifier("bong", "jiemai_burst_blood"),
            new SwordQiSlashPlayer()
        );
        registry.register(
            new net.minecraft.util.Identifier("bong", "jiemai_neutralize_dust"),
            new SwordQiSlashPlayer()
        );
        registry.register(
            new net.minecraft.util.Identifier("bong", "jiemai_sever_flash"),
            new SwordQiSlashPlayer()
        );
    }
}
