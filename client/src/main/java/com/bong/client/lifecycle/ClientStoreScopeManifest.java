package com.bong.client.lifecycle;

import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

public final class ClientStoreScopeManifest {
    private static final Set<String> EXTERNALLY_MANAGED_SESSION_STORES = Set.of(
        "com.bong.client.ui.ClientConnectionStatusStore"
    );

    private static final Set<String> PERSISTENT_CONFIG_STORES = Set.of(
        "com.bong.client.hud.HudLayoutPreferenceStore"
    );

    private static final Set<String> CONSTANT_STORES = Set.of(
        "com.bong.client.combat.ArmorProfileStore"
    );

    private static final Set<String> SESSION_SCOPED_STORES = orderedSet(List.of(
        "com.bong.client.agentui.AgentUiStore",
        "com.bong.client.agentui.AgentUiVfxStore",
        "com.bong.client.alchemy.state.AlchemyAttemptHistoryStore",
        "com.bong.client.alchemy.state.AlchemyFurnaceStore",
        "com.bong.client.alchemy.state.AlchemyOutcomeForecastStore",
        "com.bong.client.alchemy.state.AlchemySessionStore",
        "com.bong.client.alchemy.state.ContaminationWarningStore",
        "com.bong.client.alchemy.state.InventoryMetaStore",
        "com.bong.client.alchemy.state.RecipeScrollStore",
        "com.bong.client.botany.BotanyPlantRenderProfileStore",
        "com.bong.client.botany.BotanyPlantStageVisualStore",
        "com.bong.client.botany.HarvestSessionStore",
        "com.bong.client.coffin.TutorialCoffinPosStore",
        "com.bong.client.combat.CastStateStore",
        "com.bong.client.combat.CombatHudStateStore",
        "com.bong.client.combat.DefenseWindowStore",
        "com.bong.client.combat.EquippedShieldStore",
        "com.bong.client.combat.QuickUseSlotStore",
        "com.bong.client.combat.SkillBarStore",
        "com.bong.client.combat.SkillConfigStore",
        "com.bong.client.combat.SpellVolumeStore",
        "com.bong.client.combat.TreasureEquippedStore",
        "com.bong.client.combat.UnifiedEventStore",
        "com.bong.client.combat.UnlockedStylesStore",
        "com.bong.client.combat.WeaponEquippedStore",
        "com.bong.client.combat.baomai.v3.BaomaiV3HudStateStore",
        "com.bong.client.combat.baomai.v4.CrackReadingHudStateStore",
        "com.bong.client.combat.baomai.v4.ResonanceLockHudStateStore",
        "com.bong.client.combat.store.AscensionQuotaStore",
        "com.bong.client.combat.store.CarrierStateStore",
        "com.bong.client.combat.store.DamageFloaterStore",
        "com.bong.client.combat.store.DeathStateStore",
        "com.bong.client.combat.store.DerivedAttrsStore",
        "com.bong.client.combat.store.DuguPoisonStateStore",
        "com.bong.client.combat.store.FalseSkinHudStateStore",
        "com.bong.client.combat.store.FullPowerStateStore",
        "com.bong.client.combat.store.HalfStepRechallengeStore",
        "com.bong.client.combat.store.StatusEffectStore",
        "com.bong.client.combat.store.TerminateStateStore",
        "com.bong.client.combat.store.TribulationBroadcastStore",
        "com.bong.client.combat.store.TribulationStateStore",
        "com.bong.client.combat.store.VortexStateStore",
        "com.bong.client.combat.store.WoundsStore",
        "com.bong.client.craft.CraftStore",
        "com.bong.client.cultivation.BreakthroughRenderStateStore",
        "com.bong.client.cultivation.QiColorObservedStore",
        "com.bong.client.cultivation.voidaction.VoidActionStore",
        "com.bong.client.dying_elder.DyingElderEncounterStore",
        "com.bong.client.fauna.HallucinationLayerStore",
        "com.bong.client.forge.state.BlueprintScrollStore",
        "com.bong.client.forge.state.ForgeOutcomeStore",
        "com.bong.client.forge.state.ForgeSessionStore",
        "com.bong.client.forge.state.ForgeStationStore",
        "com.bong.client.gathering.GatheringSessionStore",
        "com.bong.client.hud.AnqiHudStateStore",
        "com.bong.client.hud.BongHudStateStore",
        "com.bong.client.hud.CoffinStateStore",
        "com.bong.client.hud.DuguV2HudStateStore",
        "com.bong.client.hud.LootContainerStateStore",
        "com.bong.client.hud.PoisonTraitHudStateStore",
        "com.bong.client.hud.SearchHudStateStore",
        "com.bong.client.hud.SwordBondHudStateStore",
        "com.bong.client.hud.TargetInfoStateStore",
        "com.bong.client.hud.ZhenmaiHudStateStore",
        "com.bong.client.identity.IdentityPanelStateStore",
        "com.bong.client.insight.InsightOfferStore",
        "com.bong.client.inventory.state.BodyPlanLayoutStore",
        "com.bong.client.inventory.state.DroppedItemStore",
        "com.bong.client.inventory.state.InventoryStateStore",
        "com.bong.client.inventory.state.MeridianStateStore",
        "com.bong.client.inventory.state.MorphStateStore",
        "com.bong.client.inventory.state.PhysicalBodyStore",
        "com.bong.client.inventory.state.PlayerRaceIdentityStore",
        "com.bong.client.inventory.state.RaceGateMetaStore",
        "com.bong.client.inventory.state.RemainsStore",
        "com.bong.client.lingtian.state.LingtianSessionStore",
        "com.bong.client.movement.MovementStateStore",
        "com.bong.client.npc.NpcInteractionLogStore",
        "com.bong.client.npc.NpcLodStore",
        "com.bong.client.npc.NpcMetadataStore",
        "com.bong.client.npc.NpcMoodStore",
        "com.bong.client.omen.OmenStateStore",
        "com.bong.client.processing.state.FreshnessStore",
        "com.bong.client.processing.state.ProcessingSessionStore",
        "com.bong.client.scroll.ScrollReadStore",
        "com.bong.client.skill.SkillMilestoneStore",
        "com.bong.client.skill.SkillRecentEventStore",
        "com.bong.client.skill.SkillSetStore",
        "com.bong.client.social.NicheGuardianStore",
        "com.bong.client.social.SocialStateStore",
        "com.bong.client.spirittreasure.SpiritTreasureDialogueStore",
        "com.bong.client.spirittreasure.SpiritTreasureStateStore",
        "com.bong.client.state.PlayerStateStore",
        "com.bong.client.state.RealmCollapseHudStateStore",
        "com.bong.client.state.SeasonStateStore",
        "com.bong.client.tiandao.TiandaoPresenceStore",
        "com.bong.client.tsy.ExtractStateStore",
        "com.bong.client.tsy.TsyBossHealthStore",
        "com.bong.client.tsy.TsyContainerStateStore",
        "com.bong.client.tsy.TsyDeathVfxStore",
        "com.bong.client.ui.ClientConnectionStatusStore",
        "com.bong.client.visual.VoidErosionVisualStore",
        "com.bong.client.visual.realm_vision.PerceptionEdgeStateStore",
        "com.bong.client.visual.realm_vision.RealmVisionStateStore",
        "com.bong.client.yidao.YidaoHudStateStore",
        "com.bong.client.yidao.YidaoNpcAiStateStore"
    ));

    private ClientStoreScopeManifest() {
    }

    public static Set<String> sessionScopedStores() {
        return SESSION_SCOPED_STORES;
    }

    public static Set<String> externallyManagedSessionStores() {
        return EXTERNALLY_MANAGED_SESSION_STORES;
    }

    public static Set<String> registryManagedSessionStores() {
        LinkedHashSet<String> stores = new LinkedHashSet<>(SESSION_SCOPED_STORES);
        stores.removeAll(EXTERNALLY_MANAGED_SESSION_STORES);
        return Collections.unmodifiableSet(stores);
    }

    public static Set<String> persistentConfigStores() {
        return PERSISTENT_CONFIG_STORES;
    }

    public static Set<String> constantStores() {
        return CONSTANT_STORES;
    }

    public static Set<String> allClassifiedStores() {
        LinkedHashSet<String> stores = new LinkedHashSet<>();
        stores.addAll(SESSION_SCOPED_STORES);
        stores.addAll(PERSISTENT_CONFIG_STORES);
        stores.addAll(CONSTANT_STORES);
        return Collections.unmodifiableSet(stores);
    }

    private static Set<String> orderedSet(List<String> values) {
        return Collections.unmodifiableSet(new LinkedHashSet<>(values));
    }
}
