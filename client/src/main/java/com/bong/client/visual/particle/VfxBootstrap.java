package com.bong.client.visual.particle;

/**
 * 集中注册 {@link VfxRegistry} 的默认 event → player 绑定。
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
        JueBiTribulationPlayer jueBi = new JueBiTribulationPlayer();
        registry.register(JueBiTribulationPlayer.BOUNDARY,        jueBi);
        registry.register(JueBiTribulationPlayer.FISSURE,         jueBi);
        registry.register(JueBiTribulationPlayer.ERUPTION,        jueBi);
        registry.register(RealmCollapseBoundaryPlayer.EVENT_ID,  new RealmCollapseBoundaryPlayer());
        registry.register(FormationActivatePlayer.EVENT_ID,      new FormationActivatePlayer());
        registry.register(DeathSoulDissipatePlayer.EVENT_ID,     new DeathSoulDissipatePlayer());
        registry.register(NpcDeathSmokePlayer.EVENT_ID,          new NpcDeathSmokePlayer());
        registry.register(NpcDeathQiBurstPlayer.EVENT_ID,        new NpcDeathQiBurstPlayer());
        NpcRankAuraPlayer npcRankAura = new NpcRankAuraPlayer();
        registry.register(NpcRankAuraPlayer.ELDER,               npcRankAura);
        registry.register(NpcRankAuraPlayer.MASTER,              npcRankAura);
        registry.register(NpcQiAuraRipplePlayer.EVENT_ID,        new NpcQiAuraRipplePlayer());
        registry.register(FlyingSwordDemoPlayer.EVENT_ID,        new FlyingSwordDemoPlayer());
        registry.register(FormationCoreDemoPlayer.EVENT_ID,      new FormationCoreDemoPlayer());
        registry.register(BurstMeridianBengQuanPlayer.EVENT_ID,  new BurstMeridianBengQuanPlayer());
        registry.register(ChargingOrbVfx.EVENT_ID,               new ChargingOrbVfx());
        registry.register(ReleaseLightningVfx.EVENT_ID,          new ReleaseLightningVfx());
        registry.register(ExhaustedGreyMistVfx.EVENT_ID,         new ExhaustedGreyMistVfx());
        BaomaiV3VfxPlayer baomaiV3 = new BaomaiV3VfxPlayer();
        registry.register(BaomaiV3VfxPlayer.GROUND_WAVE_DUST,              baomaiV3);
        registry.register(BaomaiV3VfxPlayer.BLOOD_BURN_CRIMSON,            baomaiV3);
        registry.register(BaomaiV3VfxPlayer.BODY_TRANSCENDENCE_PILLAR,     baomaiV3);
        registry.register(BaomaiV3VfxPlayer.MERIDIAN_RIPPLE_SCAR,          baomaiV3);
        registry.register(FrostBreathPlayer.EVENT_ID,            new FrostBreathPlayer());
        registry.register(BotanyAuraPlayer.EVENT_ID,             new BotanyAuraPlayer());
        registry.register(BotanyHarvestBurstPlayer.EVENT_ID,     new BotanyHarvestBurstPlayer());
        registry.register(BotanyPlantStagePlayer.ROUTE_ID,       new BotanyPlantStagePlayer());
        LingtianPlotRunePlayer lingtianPlotRunes = new LingtianPlotRunePlayer();
        registry.register(LingtianPlotRunePlayer.TILL,           lingtianPlotRunes);
        registry.register(LingtianPlotRunePlayer.PLANT,          lingtianPlotRunes);
        registry.register(LingtianPlotRunePlayer.REPLENISH,      lingtianPlotRunes);
        registry.register(LingtianPlotRunePlayer.HARVEST,        lingtianPlotRunes);
        registry.register(LingtianPlotRunePlayer.DRAIN,          lingtianPlotRunes);
        registry.register(RatSwarmAuraPlayer.EVENT_ID,           new RatSwarmAuraPlayer());
        registry.register(FaunaSpawnDustPlayer.EVENT_ID,         new FaunaSpawnDustPlayer());
        registry.register(FaunaBoneShatterPlayer.EVENT_ID,       new FaunaBoneShatterPlayer());
        registry.register(SpiderShimmerPlayer.EVENT_ID,          new SpiderShimmerPlayer());
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
        registry.register(VortexSpiralPlayer.EVENT_ID,           new VortexSpiralPlayer());
    }
}
