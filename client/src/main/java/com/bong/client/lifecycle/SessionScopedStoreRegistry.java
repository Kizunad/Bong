package com.bong.client.lifecycle;

import com.bong.client.agentui.AgentUiStore;
import com.bong.client.agentui.AgentUiVfxStore;
import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.alchemy.state.ContaminationWarningStore;
import com.bong.client.alchemy.state.InventoryMetaStore;
import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.botany.BotanyPlantRenderProfileStore;
import com.bong.client.botany.BotanyPlantStageVisualStore;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.coffin.TutorialCoffinPosStore;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SkillConfigStore;
import com.bong.client.combat.SpellVolumeStore;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.combat.UnlockedStylesStore;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.combat.baomai.v3.BaomaiV3HudStateStore;
import com.bong.client.combat.baomai.v4.CrackReadingHudStateStore;
import com.bong.client.combat.baomai.v4.ResonanceLockHudStateStore;
import com.bong.client.combat.store.AscensionQuotaStore;
import com.bong.client.combat.store.CarrierStateStore;
import com.bong.client.combat.store.DamageFloaterStore;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.combat.store.DuguPoisonStateStore;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.combat.store.FullPowerStateStore;
import com.bong.client.combat.store.HalfStepRechallengeStore;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.combat.store.TribulationBroadcastStore;
import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.combat.store.VortexStateStore;
import com.bong.client.combat.store.WoundsStore;
import com.bong.client.craft.CraftStore;
import com.bong.client.cultivation.BreakthroughRenderStateStore;
import com.bong.client.cultivation.QiColorObservedStore;
import com.bong.client.cultivation.voidaction.VoidActionStore;
import com.bong.client.dying_elder.DyingElderEncounterStore;
import com.bong.client.fauna.HallucinationLayerStore;
import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.hud.AnqiHudStateStore;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.CoffinStateStore;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.hud.PoisonTraitHudStateStore;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.hud.SwordBondHudStateStore;
import com.bong.client.hud.TargetInfoStateStore;
import com.bong.client.hud.ZhenmaiHudStateStore;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.inventory.state.MorphStateStore;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.inventory.state.PlayerRaceIdentityStore;
import com.bong.client.inventory.state.RaceGateMetaStore;
import com.bong.client.inventory.state.RemainsStore;
import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.movement.MovementStateStore;
import com.bong.client.npc.NpcInteractionLogStore;
import com.bong.client.npc.NpcLodStore;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.omen.OmenStateStore;
import com.bong.client.processing.state.FreshnessStore;
import com.bong.client.processing.state.ProcessingSessionStore;
import com.bong.client.scroll.ScrollReadStore;
import com.bong.client.skill.SkillMilestoneStore;
import com.bong.client.skill.SkillRecentEventStore;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.social.NicheGuardianStore;
import com.bong.client.social.SocialStateStore;
import com.bong.client.spirittreasure.SpiritTreasureDialogueStore;
import com.bong.client.spirittreasure.SpiritTreasureStateStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.RealmCollapseHudStateStore;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.tiandao.TiandaoPresenceStore;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.TsyBossHealthStore;
import com.bong.client.tsy.TsyContainerStateStore;
import com.bong.client.tsy.TsyDeathVfxStore;
import com.bong.client.visual.VoidErosionVisualStore;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.bong.client.visual.realm_vision.RealmVisionStateStore;
import com.bong.client.yidao.YidaoHudStateStore;
import com.bong.client.yidao.YidaoNpcAiStateStore;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.Consumer;

public final class SessionScopedStoreRegistry {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/session-store-lifecycle");
    private static final List<SessionStoreHandle> REGISTERED = List.of(
        SessionStoreHandle.forStore(AgentUiStore.class, AgentUiStore::clear),
        SessionStoreHandle.forStore(AgentUiVfxStore.class, AgentUiVfxStore::clear),
        SessionStoreHandle.forStore(
            AlchemyAttemptHistoryStore.class,
            AlchemyAttemptHistoryStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(AlchemyFurnaceStore.class, AlchemyFurnaceStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            AlchemyOutcomeForecastStore.class,
            AlchemyOutcomeForecastStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(AlchemySessionStore.class, AlchemySessionStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            ContaminationWarningStore.class,
            ContaminationWarningStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(InventoryMetaStore.class, InventoryMetaStore::clearOnDisconnect),
        SessionStoreHandle.forStore(RecipeScrollStore.class, RecipeScrollStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            BotanyPlantRenderProfileStore.class,
            BotanyPlantRenderProfileStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(BotanyPlantStageVisualStore.class, BotanyPlantStageVisualStore::clear),
        SessionStoreHandle.forStore(HarvestSessionStore.class, HarvestSessionStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            TutorialCoffinPosStore.class,
            TutorialCoffinPosStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(CastStateStore.class, CastStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(CombatHudStateStore.class, CombatHudStateStore::clear),
        SessionStoreHandle.forStore(DefenseWindowStore.class, DefenseWindowStore::clearOnDisconnect),
        SessionStoreHandle.forStore(EquippedShieldStore.class, EquippedShieldStore::clear),
        SessionStoreHandle.forStore(QuickUseSlotStore.class, QuickUseSlotStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SkillBarStore.class, SkillBarStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SkillConfigStore.class, SkillConfigStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SpellVolumeStore.class, SpellVolumeStore::clearOnDisconnect),
        SessionStoreHandle.forStore(TreasureEquippedStore.class, TreasureEquippedStore::clearOnDisconnect),
        SessionStoreHandle.forStore(UnifiedEventStore.class, UnifiedEventStore::clearOnDisconnect),
        SessionStoreHandle.forStore(UnlockedStylesStore.class, UnlockedStylesStore::clearOnDisconnect),
        SessionStoreHandle.forStore(WeaponEquippedStore.class, WeaponEquippedStore::clearOnDisconnect),
        SessionStoreHandle.forStore(BaomaiV3HudStateStore.class, BaomaiV3HudStateStore::clear),
        SessionStoreHandle.forStore(CrackReadingHudStateStore.class, CrackReadingHudStateStore::clear),
        SessionStoreHandle.forStore(ResonanceLockHudStateStore.class, ResonanceLockHudStateStore::clear),
        SessionStoreHandle.forStore(AscensionQuotaStore.class, AscensionQuotaStore::clearOnDisconnect),
        SessionStoreHandle.forStore(CarrierStateStore.class, CarrierStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(DamageFloaterStore.class, DamageFloaterStore::clearOnDisconnect),
        SessionStoreHandle.forStore(DeathStateStore.class, DeathStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(DerivedAttrsStore.class, DerivedAttrsStore::clearOnDisconnect),
        SessionStoreHandle.forStore(DuguPoisonStateStore.class, DuguPoisonStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(FalseSkinHudStateStore.class, FalseSkinHudStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(FullPowerStateStore.class, FullPowerStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(HalfStepRechallengeStore.class, HalfStepRechallengeStore::clear),
        SessionStoreHandle.forStore(StatusEffectStore.class, StatusEffectStore::clear),
        SessionStoreHandle.forStore(TerminateStateStore.class, TerminateStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(TribulationBroadcastStore.class, TribulationBroadcastStore::clear),
        SessionStoreHandle.forStore(TribulationStateStore.class, TribulationStateStore::clear),
        SessionStoreHandle.forStore(VortexStateStore.class, VortexStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(WoundsStore.class, WoundsStore::clear),
        SessionStoreHandle.forStore(CraftStore.class, CraftStore::clear),
        SessionStoreHandle.forStore(
            BreakthroughRenderStateStore.class,
            BreakthroughRenderStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(QiColorObservedStore.class, QiColorObservedStore::clear),
        SessionStoreHandle.forStore(VoidActionStore.class, VoidActionStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            DyingElderEncounterStore.class,
            DyingElderEncounterStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(
            HallucinationLayerStore.class,
            HallucinationLayerStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(BlueprintScrollStore.class, BlueprintScrollStore::clearOnDisconnect),
        SessionStoreHandle.forStore(ForgeOutcomeStore.class, ForgeOutcomeStore::clearOnDisconnect),
        SessionStoreHandle.forStore(ForgeSessionStore.class, ForgeSessionStore::clearOnDisconnect),
        SessionStoreHandle.forStore(ForgeStationStore.class, ForgeStationStore::clearOnDisconnect),
        SessionStoreHandle.forStore(GatheringSessionStore.class, GatheringSessionStore::clearOnDisconnect),
        SessionStoreHandle.forStore(AnqiHudStateStore.class, AnqiHudStateStore::clear),
        SessionStoreHandle.forStore(BongHudStateStore.class, BongHudStateStore::clear),
        SessionStoreHandle.forStore(CoffinStateStore.class, CoffinStateStore::clear),
        SessionStoreHandle.forStore(DuguV2HudStateStore.class, DuguV2HudStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(LootContainerStateStore.class, LootContainerStateStore::clear),
        SessionStoreHandle.forStore(PoisonTraitHudStateStore.class, PoisonTraitHudStateStore::clear),
        SessionStoreHandle.forStore(SearchHudStateStore.class, SearchHudStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SwordBondHudStateStore.class, SwordBondHudStateStore::clear),
        SessionStoreHandle.forStore(TargetInfoStateStore.class, TargetInfoStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(ZhenmaiHudStateStore.class, ZhenmaiHudStateStore::clear),
        SessionStoreHandle.forStore(
            IdentityPanelStateStore.class,
            IdentityPanelStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(InsightOfferStore.class, InsightOfferStore::clearOnDisconnect),
        SessionStoreHandle.forStore(BodyPlanLayoutStore.class, BodyPlanLayoutStore::clearOnDisconnect),
        SessionStoreHandle.forStore(DroppedItemStore.class, DroppedItemStore::clearOnDisconnect),
        SessionStoreHandle.forStore(InventoryStateStore.class, InventoryStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(MeridianStateStore.class, MeridianStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(MorphStateStore.class, MorphStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(PhysicalBodyStore.class, PhysicalBodyStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            PlayerRaceIdentityStore.class,
            PlayerRaceIdentityStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(RaceGateMetaStore.class, RaceGateMetaStore::clearOnDisconnect),
        SessionStoreHandle.forStore(RemainsStore.class, RemainsStore::clearOnDisconnect),
        SessionStoreHandle.forStore(LingtianSessionStore.class, LingtianSessionStore::clearOnDisconnect),
        SessionStoreHandle.forStore(MovementStateStore.class, MovementStateStore::clear),
        SessionStoreHandle.forStore(NpcInteractionLogStore.class, NpcInteractionLogStore::clearOnDisconnect),
        SessionStoreHandle.forStore(NpcLodStore.class, NpcLodStore::clearAll),
        SessionStoreHandle.forStore(NpcMetadataStore.class, NpcMetadataStore::clearAll),
        SessionStoreHandle.forStore(NpcMoodStore.class, NpcMoodStore::clearAll),
        SessionStoreHandle.forStore(OmenStateStore.class, OmenStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(FreshnessStore.class, FreshnessStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            ProcessingSessionStore.class,
            ProcessingSessionStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(ScrollReadStore.class, ScrollReadStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SkillMilestoneStore.class, SkillMilestoneStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SkillRecentEventStore.class, SkillRecentEventStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SkillSetStore.class, SkillSetStore::clearOnDisconnect),
        SessionStoreHandle.forStore(NicheGuardianStore.class, NicheGuardianStore::clearOnDisconnect),
        SessionStoreHandle.forStore(SocialStateStore.class, SocialStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            SpiritTreasureDialogueStore.class,
            SpiritTreasureDialogueStore::clear
        ),
        SessionStoreHandle.forStore(SpiritTreasureStateStore.class, SpiritTreasureStateStore::clear),
        SessionStoreHandle.forStore(PlayerStateStore.class, PlayerStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(
            RealmCollapseHudStateStore.class,
            RealmCollapseHudStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(SeasonStateStore.class, SeasonStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(TiandaoPresenceStore.class, TiandaoPresenceStore::clear),
        SessionStoreHandle.forStore(ExtractStateStore.class, ExtractStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(TsyBossHealthStore.class, TsyBossHealthStore::reset),
        SessionStoreHandle.forStore(TsyContainerStateStore.class, TsyContainerStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(TsyDeathVfxStore.class, TsyDeathVfxStore::reset),
        SessionStoreHandle.forStore(VoidErosionVisualStore.class, VoidErosionVisualStore::reset),
        SessionStoreHandle.forStore(
            PerceptionEdgeStateStore.class,
            PerceptionEdgeStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(RealmVisionStateStore.class, RealmVisionStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(YidaoHudStateStore.class, YidaoHudStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(YidaoNpcAiStateStore.class, YidaoNpcAiStateStore::clearOnDisconnect)
    );

    private SessionScopedStoreRegistry() {
    }

    public static void clearAllOnDisconnect() {
        clearAllOnDisconnect(
            REGISTERED,
            failure -> LOGGER.error(
                "Failed to clear session store {} on disconnect",
                failure.fqcn(),
                failure.cause()
            )
        );
    }

    static void clearAllOnDisconnect(
        List<SessionStoreHandle> handles,
        Consumer<StoreClearFailure> failureHandler
    ) {
        Objects.requireNonNull(handles, "handles");
        Objects.requireNonNull(failureHandler, "failureHandler");
        validateUniqueFqcns(handles);
        List<StoreClearFailure> failures = new ArrayList<>();
        for (SessionStoreHandle handle : handles) {
            try {
                handle.clearOnDisconnect();
            } catch (RuntimeException exception) {
                failures.add(new StoreClearFailure(handle.fqcn(), exception));
            }
        }
        RuntimeException reportingFailure = null;
        for (StoreClearFailure failure : failures) {
            try {
                failureHandler.accept(failure);
            } catch (RuntimeException exception) {
                if (reportingFailure == null) {
                    reportingFailure = exception;
                } else if (reportingFailure != exception) {
                    reportingFailure.addSuppressed(exception);
                }
            }
        }
        if (reportingFailure != null) {
            throw reportingFailure;
        }
    }

    static List<SessionStoreHandle> registeredHandlesForTests() {
        return REGISTERED;
    }

    static List<String> registeredFqcnsForTests() {
        return registeredHandlesForTests().stream().map(SessionStoreHandle::fqcn).toList();
    }

    static void validateUniqueFqcns(List<SessionStoreHandle> handles) {
        Set<String> seen = new HashSet<>();
        List<String> duplicates = new ArrayList<>();
        for (SessionStoreHandle handle : handles) {
            Objects.requireNonNull(handle, "handle");
            if (!seen.add(handle.fqcn())) {
                duplicates.add(handle.fqcn());
            }
        }
        if (!duplicates.isEmpty()) {
            throw new IllegalArgumentException("Duplicate session store FQCNs: " + duplicates);
        }
    }

    record StoreClearFailure(String fqcn, RuntimeException cause) {
        StoreClearFailure {
            Objects.requireNonNull(fqcn, "fqcn");
            Objects.requireNonNull(cause, "cause");
        }
    }
}
