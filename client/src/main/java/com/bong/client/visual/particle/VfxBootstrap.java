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
        CalamityVfxPlayer calamity = new CalamityVfxPlayer();
        registry.register(CalamityVfxPlayer.THUNDER,             calamity);
        registry.register(CalamityVfxPlayer.MIASMA,              calamity);
        registry.register(CalamityVfxPlayer.MERIDIAN_SEAL,       calamity);
        registry.register(CalamityVfxPlayer.DAOXIANG_WAVE,       calamity);
        registry.register(CalamityVfxPlayer.HEAVENLY_FIRE,       calamity);
        registry.register(CalamityVfxPlayer.PRESSURE_INVERT,     calamity);
        registry.register(CalamityVfxPlayer.ALL_WITHER,          calamity);
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
        TsyPortalVortexPlayer tsyPortal = new TsyPortalVortexPlayer();
        registry.register(TsyPortalVortexPlayer.MAIN_RIFT,        tsyPortal);
        registry.register(TsyPortalVortexPlayer.DEEP_RIFT,        tsyPortal);
        registry.register(TsyPortalVortexPlayer.COLLAPSE_TEAR,    tsyPortal);
        registry.register(TsyCollapseBurstPlayer.EVENT_ID,        new TsyCollapseBurstPlayer());
        registry.register(TsyFuyaAuraPlayer.EVENT_ID,             new TsyFuyaAuraPlayer());
        TsySearchFeedbackPlayer tsySearch = new TsySearchFeedbackPlayer();
        registry.register(TsySearchFeedbackPlayer.DUST,           tsySearch);
        registry.register(TsySearchFeedbackPlayer.LOOT_POP,       tsySearch);
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
        registry.register(MigrationVisualPlayer.EVENT_ID,        new MigrationVisualPlayer());
        PseudoVeinVisualPlayer pseudoVein = new PseudoVeinVisualPlayer();
        registry.register(PseudoVeinVisualPlayer.RISING,         pseudoVein);
        registry.register(PseudoVeinVisualPlayer.ACTIVE,         pseudoVein);
        registry.register(PseudoVeinVisualPlayer.WARNING,        pseudoVein);
        registry.register(PseudoVeinVisualPlayer.DISSIPATING,    pseudoVein);
        registry.register(PseudoVeinVisualPlayer.AFTERMATH,      pseudoVein);
        registry.register(FaunaBoneShatterPlayer.EVENT_ID,       new FaunaBoneShatterPlayer());
        registry.register(SpiderShimmerPlayer.EVENT_ID,          new SpiderShimmerPlayer());
        registry.register(TuikeFalseSkinParticlePlayer.DON_DUST,
            new TuikeFalseSkinParticlePlayer(TuikeFalseSkinParticlePlayer.DON_DUST));
        registry.register(TuikeFalseSkinParticlePlayer.SHED_BURST,
            new TuikeFalseSkinParticlePlayer(TuikeFalseSkinParticlePlayer.SHED_BURST));
        registry.register(TuikeFalseSkinParticlePlayer.ANCIENT_GLOW,
            new TuikeFalseSkinParticlePlayer(TuikeFalseSkinParticlePlayer.ANCIENT_GLOW));
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
        VortexSpiralPlayer woliuVortex = new VortexSpiralPlayer();
        registry.register(VortexSpiralPlayer.EVENT_ID,           woliuVortex);
        registry.register(VortexSpiralPlayer.VACUUM_PALM,        woliuVortex);
        registry.register(VortexSpiralPlayer.VORTEX_SHIELD,      woliuVortex);
        registry.register(VortexSpiralPlayer.VACUUM_LOCK,        woliuVortex);
        registry.register(VortexSpiralPlayer.VORTEX_RESONANCE,   woliuVortex);
        registry.register(VortexSpiralPlayer.TURBULENCE_BURST,   woliuVortex);
        registry.register(CultivationAbsorbPlayer.EVENT_ID,      new CultivationAbsorbPlayer());
        registry.register(MeridianOpenFlashPlayer.EVENT_ID,      new MeridianOpenFlashPlayer());
        registry.register(BreakthroughFailPlayer.EVENT_ID,       new BreakthroughFailPlayer());
        registry.register(CombatHitDirectionPlayer.HIT,          new CombatHitDirectionPlayer(false));
        registry.register(CombatHitDirectionPlayer.PARRY,        new CombatHitDirectionPlayer(true));
        registry.register(ForgeHammerStrikePlayer.HAMMER,
            new ForgeHammerStrikePlayer(ForgeHammerStrikePlayer.Kind.HAMMER));
        registry.register(ForgeHammerStrikePlayer.INSCRIPTION,
            new ForgeHammerStrikePlayer(ForgeHammerStrikePlayer.Kind.INSCRIPTION));
        registry.register(ForgeHammerStrikePlayer.CONSECRATION,
            new ForgeHammerStrikePlayer(ForgeHammerStrikePlayer.Kind.CONSECRATION));
        registry.register(AlchemyBrewVaporPlayer.BREW,
            new AlchemyBrewVaporPlayer(AlchemyBrewVaporPlayer.Kind.BREW));
        registry.register(AlchemyBrewVaporPlayer.OVERHEAT,
            new AlchemyBrewVaporPlayer(AlchemyBrewVaporPlayer.Kind.OVERHEAT));
        registry.register(AlchemyBrewVaporPlayer.COMPLETE,
            new AlchemyBrewVaporPlayer(AlchemyBrewVaporPlayer.Kind.COMPLETE));
        registry.register(AlchemyBrewVaporPlayer.EXPLODE,
            new AlchemyBrewVaporPlayer(AlchemyBrewVaporPlayer.Kind.EXPLODE));
        registry.register(LingtianActionVfxPlayer.TILL,
            new LingtianActionVfxPlayer(LingtianActionVfxPlayer.Kind.TILL));
        registry.register(LingtianActionVfxPlayer.PLANT,
            new LingtianActionVfxPlayer(LingtianActionVfxPlayer.Kind.PLANT));
        registry.register(LingtianActionVfxPlayer.REPLENISH,
            new LingtianActionVfxPlayer(LingtianActionVfxPlayer.Kind.REPLENISH));
        registry.register(ZhenfaActionVfxPlayer.TRAP,
            new ZhenfaActionVfxPlayer(ZhenfaActionVfxPlayer.Kind.TRAP));
        registry.register(ZhenfaActionVfxPlayer.WARD,
            new ZhenfaActionVfxPlayer(ZhenfaActionVfxPlayer.Kind.WARD));
        registry.register(ZhenfaActionVfxPlayer.DEPLETE,
            new ZhenfaActionVfxPlayer(ZhenfaActionVfxPlayer.Kind.DEPLETE));
        registry.register(SocialLinkVfxPlayer.NICHE_ESTABLISH,
            new SocialLinkVfxPlayer(SocialLinkVfxPlayer.Kind.NICHE_ESTABLISH));
        registry.register(SocialLinkVfxPlayer.PACT_LINK,
            new SocialLinkVfxPlayer(SocialLinkVfxPlayer.Kind.PACT_LINK));
        registry.register(SocialLinkVfxPlayer.FEUD_MARK,
            new SocialLinkVfxPlayer(SocialLinkVfxPlayer.Kind.FEUD_MARK));
        registry.register(PoisonMistPlayer.EVENT_ID,            new PoisonMistPlayer());
        registry.register(MovementVfxPlayer.DASH,
            new MovementVfxPlayer(MovementVfxPlayer.Kind.DASH));
        registry.register(MovementVfxPlayer.SLIDE,
            new MovementVfxPlayer(MovementVfxPlayer.Kind.SLIDE));
        registry.register(MovementVfxPlayer.DOUBLE_JUMP,
            new MovementVfxPlayer(MovementVfxPlayer.Kind.DOUBLE_JUMP));
    }
}
