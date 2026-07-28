package com.bong.client.lifecycle;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiStore;
import com.bong.client.agentui.AgentUiVfxState;
import com.bong.client.agentui.AgentUiVfxStore;
import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.alchemy.state.ContaminationWarningStore;
import com.bong.client.alchemy.state.InventoryMetaStore;
import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.botany.BotanyPlantRenderProfile;
import com.bong.client.botany.BotanyPlantRenderProfileStore;
import com.bong.client.botany.BotanyPlantStageVisual;
import com.bong.client.botany.BotanyPlantStageVisualStore;
import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.botany.PlantGrowthStage;
import com.bong.client.coffin.TutorialCoffinPosStore;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SkillConfigStore;
import com.bong.client.combat.SpellVolumeStore;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.combat.UnlockedStyles;
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
import com.google.gson.JsonObject;
import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.cultivation.BreakthroughCinematicPayload;
import com.bong.client.cultivation.BreakthroughRenderState;
import com.bong.client.cultivation.BreakthroughRenderStateStore;
import com.bong.client.cultivation.ColorKind;
import com.bong.client.cultivation.QiColorObservedState;
import com.bong.client.cultivation.QiColorObservedStore;
import com.bong.client.cultivation.voidaction.VoidActionStore;
import com.bong.client.dying_elder.DyingElderEncounterStore;
import com.bong.client.fauna.HallucinationLayerStore;
import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.gathering.GatheringSessionViewModel;
import com.bong.client.hud.AnqiHudStateStore;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.CoffinStateStore;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.hud.PoisonTraitHudStateStore;
import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.hud.SwordBondHudState;
import com.bong.client.hud.SwordBondHudStateStore;
import com.bong.client.hud.TargetInfoState;
import com.bong.client.hud.TargetInfoStateStore;
import com.bong.client.hud.ZhenmaiHudStateStore;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.insight.InsightCategory;
import com.bong.client.insight.InsightChoice;
import com.bong.client.insight.InsightDecision;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.insight.InsightOfferViewModel;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MorphEntry;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.RaceGate;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
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
import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.network.VfxEventPayload;
import com.bong.client.npc.NpcInteractionLogEntry;
import com.bong.client.npc.NpcInteractionLogStore;
import com.bong.client.npc.NpcLodSnapshot;
import com.bong.client.npc.NpcLodStore;
import com.bong.client.npc.NpcMetadata;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.npc.NpcMoodState;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.omen.OmenStateStore;
import com.bong.client.processing.state.FreshnessStore;
import com.bong.client.processing.state.ProcessingSessionStore;
import com.bong.client.scroll.ScrollOpenViewModel;
import com.bong.client.scroll.ScrollReadStore;
import com.bong.client.skill.SkillMilestoneStore;
import com.bong.client.skill.SkillRecentEventStore;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.social.NicheGuardianStore;
import com.bong.client.social.SocialStateStore;
import com.bong.client.spirittreasure.SpiritTreasureDialogue;
import com.bong.client.spirittreasure.SpiritTreasureDialogueStore;
import com.bong.client.spirittreasure.SpiritTreasureState;
import com.bong.client.spirittreasure.SpiritTreasureStateStore;
import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.RealmCollapseHudState;
import com.bong.client.state.RealmCollapseHudStateStore;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import com.bong.client.tiandao.TiandaoPresenceState;
import com.bong.client.tiandao.TiandaoPresenceStore;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.TsyBossHealthState;
import com.bong.client.tsy.TsyBossHealthStore;
import com.bong.client.tsy.TsyContainerStateStore;
import com.bong.client.tsy.TsyDeathVfxState;
import com.bong.client.tsy.TsyDeathVfxStore;
import com.bong.client.visual.VoidErosionVisualStore;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.bong.client.visual.realm_vision.RealmVisionState;
import com.bong.client.visual.realm_vision.RealmVisionStateStore;
import com.bong.client.visual.realm_vision.SenseKind;
import com.bong.client.yidao.YidaoHudStateStore;
import com.bong.client.yidao.YidaoNpcAiStateStore;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.MethodSource;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BooleanSupplier;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SessionScopedStoreRegistryProductionAdapterTest {
    private static final String SCREEN_XML =
        "<owo-ui><components><flow-layout direction=\"vertical\"/></components></owo-ui>";

    @BeforeEach
    void setUp() {
        resetTestOnlyState();
        SessionScopedStoreRegistry.clearAllOnDisconnect();
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        SessionScopedStoreRegistry.clearAllOnDisconnect();
        resetTestOnlyState();
    }

    @ParameterizedTest(name = "{0}")
    @MethodSource("productionAdapters")
    void eachRegisteredHandleClearsItsDeclaredStoreWithoutTouchingCanary(ProductionAdapterCase adapter) {
        List<SessionStoreHandle> handles = SessionScopedStoreRegistry.registeredHandlesForTests();
        assertEquals(105, handles.size(), "P2 必须对生产 REGISTERED 的全部 105 个 handle 逐项验真");
        SessionStoreHandle handle = handles.get(adapter.index());
        assertSame(
            adapter.storeType(),
            handle.storeType(),
            "测试 case 必须按生产声明顺序取得对应 Class handle，不能重建 cleaner 映射"
        );

        adapter.seed().run();
        assertFalse(adapter.isCleared().getAsBoolean(), "测试前必须建立目标 Store 的旧 session 状态：" + adapter);
        boolean targetIsFreshnessCanary = adapter.storeType() == FreshnessStore.class;
        if (targetIsFreshnessCanary) {
            NpcMetadataStore.upsert(metadata(9_001, "adapter-canary"));
            assertNotNull(NpcMetadataStore.get(9_001), "测试前必须建立 NPC 旁观 canary：" + adapter);
        } else {
            FreshnessStore.upsert("adapter-canary", 0.75f, "must-survive-other-cleaners");
            assertNotNull(FreshnessStore.get("adapter-canary"), "测试前必须建立 freshness 旁观 canary：" + adapter);
        }

        handle.clearOnDisconnect();

        assertTrue(adapter.isCleared().getAsBoolean(), "声明 handle 必须清掉其自身 Store：" + adapter);
        if (targetIsFreshnessCanary) {
            assertNotNull(
                NpcMetadataStore.get(9_001),
                "FreshnessStore handle 不得越权清理旁观 NpcMetadataStore：" + adapter
            );
            NpcMetadataStore.clearAll();
        } else {
            assertNotNull(
                FreshnessStore.get("adapter-canary"),
                "单个 production handle 不得越权清理旁观 FreshnessStore：" + adapter
            );
            FreshnessStore.clearOnDisconnect();
        }
    }

    private static Stream<ProductionAdapterCase> productionAdapters() {
        return Stream.concat(
            Stream.of(
                adapter(0, AgentUiStore.class,
                    () -> AgentUiStore.setActive(screen("old-active")),
                    () -> AgentUiStore.getActive() == null),
                adapter(1, AgentUiVfxStore.class,
                    () -> AgentUiVfxStore.setActive(new AgentUiVfxState(1_000L, true)),
                    () -> AgentUiVfxStore.getActive() == null),
                adapter(3, AlchemyFurnaceStore.class,
                    () -> AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(
                        new BlockPos(1, 64, 1), 2, 40f, 100f, "old", true)),
                    () -> AlchemyFurnaceStore.Snapshot.empty().equals(AlchemyFurnaceStore.snapshot())),
                adapter(5, AlchemySessionStore.class,
                    () -> AlchemySessionStore.replace(new AlchemySessionStore.Snapshot(
                        "old", true, 1, 10, 1f, 2f, 0.5f, 1d, 2d, "old", List.of(), List.of())),
                    () -> AlchemySessionStore.Snapshot.empty().equals(AlchemySessionStore.snapshot())),
                adapter(9, BotanyPlantRenderProfileStore.class,
                    () -> BotanyPlantRenderProfileStore.replaceAll(List.of(new BotanyPlantRenderProfile(
                        "old", "grass", 0x123456, null, BotanyPlantRenderProfile.ModelOverlay.NONE))),
                    () -> BotanyPlantRenderProfileStore.snapshot().isEmpty()),
                adapter(10, BotanyPlantStageVisualStore.class,
                    () -> BotanyPlantStageVisualStore.upsert(new BotanyPlantStageVisual(
                        "old", "old", PlantGrowthStage.MATURE, new double[] {1d, 2d, 3d}, 0x123456, 1d, 100L, 1L)),
                    () -> BotanyPlantStageVisualStore.snapshot().isEmpty()),
                adapter(12, TutorialCoffinPosStore.class,
                    () -> TutorialCoffinPosStore.set(new BlockPos(1, 64, 1)),
                    () -> TutorialCoffinPosStore.snapshot().isEmpty()),
                adapter(14, CombatHudStateStore.class,
                    () -> CombatHudStateStore.replace(CombatHudState.create(
                        0.2f, 0.3f, 0.4f, DerivedAttrFlags.none())),
                    () -> CombatHudStateStore.snapshot() == CombatHudState.empty()),
                adapter(16, EquippedShieldStore.class,
                    () -> EquippedShieldStore.equip(new EquippedShield(1L, "old", 10f, 20f)),
                    () -> EquippedShieldStore.snapshot() == null),
                adapter(17, QuickUseSlotStore.class,
                    () -> QuickUseSlotStore.replace(QuickSlotConfig.of(
                        new QuickSlotEntry[] {new QuickSlotEntry("old", "old", 1, 1, "")}, new long[] {2_000L})),
                    () -> QuickUseSlotStore.snapshot() == QuickSlotConfig.empty()),
                adapter(18, SkillBarStore.class,
                    () -> {
                        SkillBarStore.replace(SkillBarConfig.of(
                            new SkillBarEntry[] {SkillBarEntry.item("old", "old", 1, 1, "")},
                            new long[] {2_000L}
                        ));
                        SkillBarStore.setSelectedSlot(0);
                    },
                    () -> SkillBarStore.snapshot() == SkillBarConfig.empty()
                        && SkillBarStore.selectedSlot() == SkillBarStore.NO_SELECTED_SLOT),
                adapter(19, SkillConfigStore.class,
                    () -> {
                        JsonObject config = new JsonObject();
                        config.addProperty("old", 1);
                        SkillConfigStore.updateLocal("old", config);
                    },
                    () -> SkillConfigStore.snapshot().isEmpty()),
                adapter(20, SpellVolumeStore.class,
                    () -> SpellVolumeStore.show(2f, 20f, 0.5f),
                    () -> SpellVolumeStore.snapshot() == com.bong.client.combat.SpellVolumeState.idle()),
                adapter(21, TreasureEquippedStore.class,
                    () -> TreasureEquippedStore.putOrClear("main", new EquippedTreasure("main", 1L, "old", "old")),
                    () -> TreasureEquippedStore.get("main") == null),
                adapter(23, UnlockedStylesStore.class,
                    () -> UnlockedStylesStore.replace(UnlockedStyles.all()),
                    () -> UnlockedStylesStore.snapshot() == UnlockedStyles.none()),
                adapter(24, WeaponEquippedStore.class,
                    () -> WeaponEquippedStore.putOrClear("main_hand", new EquippedWeapon(
                        "main_hand", 1L, "old", "old", 1f, 2f, 1)),
                    () -> WeaponEquippedStore.get("main_hand") == null),
                adapter(25, BaomaiV3HudStateStore.class,
                    () -> BaomaiV3HudStateStore.recordBloodBurn(20),
                    () -> !BaomaiV3HudStateStore.snapshot(System.currentTimeMillis()).bloodBurnActive()),
                adapter(28, AscensionQuotaStore.class,
                    () -> AscensionQuotaStore.replace(new AscensionQuotaStore.State(1, 2, 1, 3d, 4d, "old")),
                    () -> AscensionQuotaStore.State.EMPTY.equals(AscensionQuotaStore.snapshot())),
                adapter(29, CarrierStateStore.class,
                    () -> CarrierStateStore.replace(new CarrierStateStore.State(
                        CarrierStateStore.Phase.CHARGED, 1f, 2f, 3f, 4L, 5L)),
                    () -> CarrierStateStore.State.NONE.equals(CarrierStateStore.snapshot())),
                adapter(30, DamageFloaterStore.class,
                    () -> DamageFloaterStore.publish(new DamageFloaterStore.Floater(
                        1d, 2d, 3d, "old", 0, DamageFloaterStore.Kind.HIT, 1_000L)),
                    () -> DamageFloaterStore.snapshot(1_000L).isEmpty()),
                adapter(31, DeathStateStore.class,
                    () -> DeathStateStore.replace(new DeathStateStore.State(
                        true, "old", 1f, List.of("old"), 1L, true, true)),
                    () -> DeathStateStore.State.HIDDEN.equals(DeathStateStore.snapshot())),
                adapter(32, DerivedAttrsStore.class,
                    () -> DerivedAttrsStore.replace(new DerivedAttrsStore.State(
                        true, 1f, 1L, true, 1L, true, "old", 1f, 1, true)),
                    () -> DerivedAttrsStore.State.NONE.equals(DerivedAttrsStore.snapshot())),
                adapter(33, DuguPoisonStateStore.class,
                    () -> DuguPoisonStateStore.replace(new DuguPoisonStateStore.State(
                        true, "old", "old", 1L, 1, 1d, 1d, 1d, 1L)),
                    () -> DuguPoisonStateStore.State.NONE.equals(DuguPoisonStateStore.snapshot())),
                adapter(35, FullPowerStateStore.class,
                    () -> FullPowerStateStore.updateCharging(new FullPowerStateStore.ChargingState(
                        true, "old", 1d, 2d, 1L, 1L)),
                    () -> !FullPowerStateStore.charging().active()),
                adapter(37, StatusEffectStore.class,
                    () -> StatusEffectStore.replace(List.of(new StatusEffectStore.Effect(
                        "old", "old", StatusEffectStore.Kind.BUFF, 1, 1L, 0, "old", 1))),
                    () -> StatusEffectStore.snapshot().isEmpty()),
                adapter(39, TribulationBroadcastStore.class,
                    () -> TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
                        true, "old", "warn", 1d, 1d, 1L, false, 1d)),
                    () -> TribulationBroadcastStore.all().isEmpty()),
                adapter(40, TribulationStateStore.class,
                    () -> TribulationStateStore.replace(new TribulationStateStore.State(
                        true, "old", "old", "old", "wave", 1d, 1d, 1, 1, 1L, 1L, 1L, false, false, List.of(), "")),
                    () -> TribulationStateStore.all().isEmpty()),
                adapter(42, WoundsStore.class,
                    () -> WoundsStore.replace(List.of(new WoundsStore.Wound(
                        "old", "cut", 1f, WoundsStore.HealingState.BLEEDING, 1f, false, 1L))),
                    () -> WoundsStore.snapshot().isEmpty()),
                adapter(45, QiColorObservedStore.class,
                    () -> QiColorObservedStore.replace(new QiColorObservedState(
                        "old-observer", "old-target", ColorKind.Sharp, null, false, false, 1d)),
                    () -> QiColorObservedStore.snapshot() == null),
                adapter(54, AnqiHudStateStore.class,
                    () -> {
                        AnqiHudStateStore.updateAim(0.5f, 1_000L, 10_000L, 1L);
                        AnqiHudStateStore.updateEcho(2, 1_000L, 10_000L, 2L);
                        AnqiHudStateStore.updateCharge(0.4f, 1_000L, 10_000L, 3L);
                        AnqiHudStateStore.updateAbrasion("old", 2f, 1_000L, 10_000L, 4L);
                        AnqiHudStateStore.updateMultiShot(3, 1_000L, 10_000L, 5L);
                    },
                    () -> AnqiHudStateStore.snapshot(1_000L).equals(com.bong.client.hud.AnqiHudState.empty())),
                adapter(58, LootContainerStateStore.class,
                    () -> LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
                        1L, "old", "old", 1, 1, 1_000L, List.of())),
                    () -> LootContainerStateStore.current() == null),
                adapter(59, PoisonTraitHudStateStore.class,
                    () -> PoisonTraitHudStateStore.update(new PoisonTraitHudStateStore.State(
                        true, 1f, 1f, 2f, 1_000L, 1f)),
                    () -> PoisonTraitHudStateStore.snapshot() == PoisonTraitHudStateStore.State.NONE),
                adapter(61, SwordBondHudStateStore.class,
                    () -> SwordBondHudStateStore.replace(new SwordBondHudState(
                        true, 1, "old", 1f, 1f, true)),
                    () -> SwordBondHudStateStore.snapshot() == SwordBondHudState.INACTIVE),
                adapter(63, ZhenmaiHudStateStore.class,
                    () -> ZhenmaiHudStateStore.flashParry(1_000L, 10_000L),
                    () -> ZhenmaiHudStateStore.snapshot(1_000L).parry() == ZhenmaiHudStateStore.Slot.EMPTY),
                adapter(68, InventoryStateStore.class,
                    () -> InventoryStateStore.replace(InventoryModel.builder()
                        .gridItem(InventoryItem.simple("old", "old"), 0, 0)
                        .build()),
                    () -> InventoryStateStore.snapshot().isEmpty()),
                adapter(76, MovementStateStore.class,
                    () -> MovementStateStore.replace(new MovementState(
                        1d, true, MovementState.Action.DASHING, MovementState.ZoneKind.DEAD,
                        1L, 1.8d, 1d, 2d, false, 1L, "old", 0L, 0L, 0L), 1_000L),
                    () -> MovementStateStore.snapshot().isEmpty()),
                adapter(84, ScrollReadStore.class,
                    () -> ScrollReadStore.replace(new ScrollOpenViewModel("old", "old", List.of("old"))),
                    () -> ScrollReadStore.snapshot() == null),
                adapter(90, SpiritTreasureDialogueStore.class,
                    () -> SpiritTreasureDialogueStore.append(new SpiritTreasureDialogue(
                        "old-request", "old-character", "old-treasure", "old", "old", "old", 0.1d, "old", 1L)),
                    () -> SpiritTreasureDialogueStore.recentFor("old-treasure").isEmpty()),
                adapter(91, SpiritTreasureStateStore.class,
                    () -> SpiritTreasureStateStore.replace(List.of(new SpiritTreasureState(
                        "old", "old", 1L, true, true, 0.5d, false, "", "", List.of())), 1L),
                    () -> SpiritTreasureStateStore.snapshot().isEmpty()),
                adapter(97, TsyBossHealthStore.class,
                    () -> TsyBossHealthStore.replace(new TsyBossHealthState(true, "旧守灵", "通灵", 0.66, 2, 4, 1_000L)),
                    () -> TsyBossHealthState.empty().equals(TsyBossHealthStore.snapshot())),
                adapter(100, VoidErosionVisualStore.class,
                    () -> {
                        VoidErosionVisualStore.replace("offline:old-a", 4, 420.0, true, 0.4f, true);
                        VoidErosionVisualStore.replace("offline:old-b", 2, 90.0, false, 0.7f, false);
                    },
                    () -> VoidErosionVisualStore.allSnapshots().isEmpty()),
                adapter(101, PerceptionEdgeStateStore.class,
                    () -> PerceptionEdgeStateStore.replace(new PerceptionEdgeState(List.of(
                        new PerceptionEdgeState.SenseEntry(SenseKind.LIVING_QI, 1d, 2d, 3d, 1d)), 1L)),
                    () -> PerceptionEdgeStateStore.snapshot().equals(PerceptionEdgeState.empty())),
                adapter(102, RealmVisionStateStore.class,
                    () -> RealmVisionStateStore.replace(new RealmVisionState(
                        new com.bong.client.visual.realm_vision.RealmVisionCommand(
                            1d, 2d, 0x123456, null, 0.5d, 0xFF123456, 0.5d, 0.5d),
                        null, 1, 1, 1L, 1)),
                    () -> RealmVisionStateStore.snapshot().equals(RealmVisionState.empty()))
            ),
            existingP1ProductionAdapters()
        );
    }

    private static Stream<ProductionAdapterCase> existingP1ProductionAdapters() {
        return Stream.of(
            adapter(2, AlchemyAttemptHistoryStore.class,
                () -> AlchemyAttemptHistoryStore.append(new AlchemyAttemptHistoryStore.Entry("old", "old", "old", "old", "old", false), 1_000L),
                () -> AlchemyAttemptHistoryStore.snapshot().isEmpty()),
            adapter(4, AlchemyOutcomeForecastStore.class,
                () -> AlchemyOutcomeForecastStore.replace(new AlchemyOutcomeForecastStore.Snapshot(1f, 2f, 3f, 4f, 5f, "old", "old", "old")),
                () -> AlchemyOutcomeForecastStore.Snapshot.neutral().equals(AlchemyOutcomeForecastStore.snapshot())),
            adapter(6, ContaminationWarningStore.class,
                () -> ContaminationWarningStore.replace(new ContaminationWarningStore.Snapshot(1f, 2f, false, 3f, 4f, false, "old")),
                () -> ContaminationWarningStore.Snapshot.neutral().equals(ContaminationWarningStore.snapshot())),
            adapter(7, InventoryMetaStore.class,
                () -> InventoryMetaStore.replace(new InventoryMetaStore.Snapshot(List.of(), 1, 1f, 2f, List.of("old"))),
                () -> InventoryMetaStore.Snapshot.empty().equals(InventoryMetaStore.snapshot())),
            adapter(8, RecipeScrollStore.class,
                () -> RecipeScrollStore.replace(new RecipeScrollStore.Snapshot(List.of(new RecipeScrollStore.RecipeEntry("old", "old", "old")), 0)),
                () -> RecipeScrollStore.Snapshot.empty().equals(RecipeScrollStore.snapshot())),
            adapter(11, HarvestSessionStore.class,
                () -> HarvestSessionStore.replace(HarvestSessionViewModel.create(
                    "old", "target", "old", "old", BotanyHarvestMode.MANUAL,
                    0.5d, true, false, false, false, "old", 1_000L)),
                () -> HarvestSessionStore.snapshot().isEmpty()),
            adapter(13, CastStateStore.class,
                () -> CastStateStore.beginSkillBarCast(3, 100, 1_000L),
                () -> CastStateStore.snapshot() == com.bong.client.combat.CastState.idle()),
            adapter(15, DefenseWindowStore.class,
                () -> DefenseWindowStore.open(100, 1_000L),
                () -> !DefenseWindowStore.snapshot().active()),
            adapter(22, UnifiedEventStore.class,
                () -> UnifiedEventStore.stream().publish(
                    UnifiedEvent.Channel.COMBAT,
                    UnifiedEvent.Priority.P2_NORMAL,
                    "old",
                    "old",
                    0,
                    1L
                ),
                () -> UnifiedEventStore.stream().snapshot().isEmpty()),
            adapter(26, CrackReadingHudStateStore.class,
                () -> CrackReadingHudStateStore.accept(108L, List.of(
                    new CrackReadingHudStateStore.MeridianEntry("Lung", "MicroTear", true, false)), true, 1_000L),
                () -> CrackReadingHudStateStore.snapshot() == CrackReadingHudStateStore.State.EMPTY),
            adapter(27, ResonanceLockHudStateStore.class,
                () -> ResonanceLockHudStateStore.onLockStarted("offline:old", 10L, 30L),
                () -> ResonanceLockHudStateStore.snapshot() == ResonanceLockHudStateStore.State.UNLOCKED),
            adapter(34, FalseSkinHudStateStore.class,
                () -> FalseSkinHudStateStore.replace(falseSkin("old-player")),
                () -> FalseSkinHudStateStore.State.NONE.equals(FalseSkinHudStateStore.snapshot())),
            adapter(36, HalfStepRechallengeStore.class,
                () -> HalfStepRechallengeStore.replace(
                    new HalfStepRechallengeStore.State(true, "old-char", 300L, 200L, 1_000L)),
                () -> HalfStepRechallengeStore.State.NONE.equals(HalfStepRechallengeStore.snapshot())),
            adapter(38, TerminateStateStore.class,
                () -> TerminateStateStore.replace(new TerminateStateStore.State(true, "old", "old", "old")),
                () -> TerminateStateStore.State.HIDDEN.equals(TerminateStateStore.snapshot())),
            adapter(41, VortexStateStore.class,
                () -> VortexStateStore.replace(new VortexStateStore.State(
                    true, 1f, 1f, 1f, 1L, 1, "old", 1f, 1L, "old", 1f, 1f, 1L)),
                () -> VortexStateStore.State.NONE.equals(VortexStateStore.snapshot())),
            adapter(43, CraftStore.class,
                () -> CraftStore.replaceRecipes(List.of(recipe("old-recipe"))),
                () -> CraftStore.recipes().isEmpty()),
            adapter(44, BreakthroughRenderStateStore.class,
                () -> BreakthroughRenderStateStore.replace(new BreakthroughRenderState(
                    new BreakthroughCinematicPayload(
                        "old", BreakthroughCinematicPayload.Phase.PRELUDE, 0, 1,
                        "醒灵", "引气", BreakthroughCinematicPayload.Result.PENDING, false,
                        0d, 0d, 0d, 1d, false, false, 1d, 1d, "old", "old", 1L
                    ),
                    1L
                )),
                () -> BreakthroughRenderStateStore.snapshot() == null),
            adapter(46, VoidActionStore.class,
                () -> VoidActionStore.setTargetZone("old"),
                () -> VoidActionStore.snapshot().equals(VoidActionStore.Snapshot.empty())),
            adapter(47, DyingElderEncounterStore.class,
                () -> DyingElderEncounterStore.activate("旧域", 112, 1_000L),
                () -> !DyingElderEncounterStore.isActive() && DyingElderEncounterStore.getElderEntityId() == 0),
            adapter(48, HallucinationLayerStore.class,
                () -> HallucinationLayerStore.activate(200),
                () -> !HallucinationLayerStore.isActive() && HallucinationLayerStore.getRemainingTicks() == 0),
            adapter(49, BlueprintScrollStore.class,
                () -> BlueprintScrollStore.replace(List.of(new BlueprintScrollStore.Entry("old", "old", 1, 1)), 0),
                () -> BlueprintScrollStore.entries().isEmpty()),
            adapter(50, ForgeOutcomeStore.class,
                () -> ForgeOutcomeStore.replace(new ForgeOutcomeStore.Snapshot(
                    1L, "old", "good", "old", 1f, "old", "", 1, false)),
                () -> ForgeOutcomeStore.Snapshot.empty().equals(ForgeOutcomeStore.lastOutcome())),
            adapter(51, ForgeSessionStore.class,
                () -> ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
                    1L, "old", "old", true, "step", 1, 1, "{}")),
                () -> ForgeSessionStore.Snapshot.empty().equals(ForgeSessionStore.snapshot())),
            adapter(52, ForgeStationStore.class,
                () -> ForgeStationStore.replace(new ForgeStationStore.Snapshot(
                    new BlockPos(1, 64, 1), "old", 1, 1f, "old", true)),
                () -> ForgeStationStore.Snapshot.empty().equals(ForgeStationStore.snapshot())),
            adapter(53, GatheringSessionStore.class,
                () -> GatheringSessionStore.replace(gathering("old-gather", 1_000L)),
                () -> GatheringSessionStore.snapshot().isEmpty()),
            adapter(55, BongHudStateStore.class,
                () -> BongHudStateStore.replace(BongHudStateSnapshot.create(
                    ZoneState.create("old", "旧域", 0.08, 6, 1_000L), NarrationState.empty(), VisualEffectState.none())),
                () -> BongHudStateStore.snapshot().isEmpty()),
            adapter(56, CoffinStateStore.class,
                () -> CoffinStateStore.replace(new CoffinStateStore.State(true, 0.7, "jade")),
                () -> CoffinStateStore.OUT.equals(CoffinStateStore.snapshot())),
            adapter(57, DuguV2HudStateStore.class,
                () -> DuguV2HudStateStore.replace(dugu("旧局中毒", true)),
                () -> DuguV2HudStateStore.State.NONE.equals(DuguV2HudStateStore.snapshot())),
            adapter(60, SearchHudStateStore.class,
                () -> SearchHudStateStore.markStarted("旧石匣", 100),
                () -> SearchHudState.idle().equals(SearchHudStateStore.snapshot())),
            adapter(62, TargetInfoStateStore.class,
                () -> TargetInfoStateStore.replaceForTests(TargetInfoState.create(
                    TargetInfoState.Kind.MOB, "old", "old", "", 1f, 1f, 1_000L)),
                () -> TargetInfoStateStore.snapshot().isEmpty()),
            adapter(64, IdentityPanelStateStore.class,
                () -> IdentityPanelStateStore.replace(identity(1, "旧身份")),
                () -> IdentityPanelState.empty().equals(IdentityPanelStateStore.snapshot())),
            adapter(65, InsightOfferStore.class,
                () -> InsightOfferStore.replace(new InsightOfferViewModel(
                    "old", "old", "old", 0.5d, 1, 1, 2_000L,
                    List.of(new InsightChoice("old", InsightCategory.QI, "old", "old", "old", "")))),
                () -> InsightOfferStore.snapshot() == null),
            adapter(66, BodyPlanLayoutStore.class,
                () -> {
                    BodyPlanLayoutStore.putLayout(new BodyPlanLayout("old", List.of(), List.of(), List.of(), List.of()));
                    BodyPlanLayoutStore.putLayout(new BodyPlanLayout("cached", List.of(), List.of(), List.of(), List.of()));
                    BodyPlanLayoutStore.setCurrentPlanId("old");
                },
                () -> BodyPlanLayoutStore.currentPlanId() == null
                    && BodyPlanLayoutStore.current() == null
                    && BodyPlanLayoutStore.byId("old") == null
                    && BodyPlanLayoutStore.byId("cached") == null),
            adapter(67, DroppedItemStore.class,
                () -> DroppedItemStore.putOrReplace(drop(1L, "old-item", 1.0)),
                () -> DroppedItemStore.snapshot().isEmpty()),
            adapter(69, MeridianStateStore.class,
                () -> MeridianStateStore.replace(MeridianBody.builder().realm("old").build()),
                () -> MeridianStateStore.snapshot() == null),
            adapter(70, MorphStateStore.class,
                () -> {
                    MorphStateStore.applyDelta(1, new MorphEntry(0, "old", "old"));
                    MorphStateStore.applyDelta(2, new MorphEntry(0, "old-2", "old-2"));
                },
                () -> MorphStateStore.morphOf(1).isEmpty() && MorphStateStore.morphOf(2).isEmpty()),
            adapter(71, PhysicalBodyStore.class,
                () -> PhysicalBodyStore.replace(PhysicalBody.builder().build()),
                () -> PhysicalBodyStore.snapshot() == null),
            adapter(72, PlayerRaceIdentityStore.class,
                () -> PlayerRaceIdentityStore.replace("old", "old-form", "old-plan", true, true),
                () -> PlayerRaceIdentityStore.raceId().isEmpty()
                    && PlayerRaceIdentityStore.formRaceId().isEmpty()
                    && PlayerRaceIdentityStore.formBodyPlanId().isEmpty()
                    && !PlayerRaceIdentityStore.intrinsicIsHumanoid()
                    && !PlayerRaceIdentityStore.formIsHumanoid()),
            adapter(73, RaceGateMetaStore.class,
                () -> RaceGateMetaStore.replace(
                    Map.of("old", new RaceGate(RaceGate.KIND_HUMANOID, List.of())),
                    Map.of("old-skill", new RaceGate(RaceGate.KIND_HUMANOID, List.of()))
                ),
                () -> !RaceGateMetaStore.hasReceived()
                    && RaceGateMetaStore.gateForItem("old") == null
                    && RaceGateMetaStore.gateForTechnique("old-skill") == null),
            adapter(74, RemainsStore.class,
                () -> RemainsStore.putOrReplace(remains("old-remains", 1.0)),
                () -> RemainsStore.snapshot().isEmpty()),
            adapter(75, LingtianSessionStore.class,
                () -> LingtianSessionStore.replace(new LingtianSessionStore.Snapshot(
                    true, LingtianSessionStore.Kind.TILL, 1, 2, 3, 1, 2, "old", "old", 0.1f, true)),
                () -> LingtianSessionStore.Snapshot.empty().equals(LingtianSessionStore.snapshot())),
            adapter(77, NpcInteractionLogStore.class,
                () -> NpcInteractionLogStore.record(new NpcInteractionLogEntry(1, "old", "old", "old", 1_000L)),
                () -> NpcInteractionLogStore.snapshot().isEmpty() && !NpcInteractionLogStore.visible()),
            adapter(78, NpcLodStore.class,
                () -> NpcLodStore.upsert(new NpcLodSnapshot(102, "rogue", "引气", 0.8f, 1.0, 64.0, 2.0)),
                () -> NpcLodStore.get(102) == null),
            adapter(79, NpcMetadataStore.class,
                () -> NpcMetadataStore.upsert(metadata(101, "旧客")),
                () -> NpcMetadataStore.get(101) == null),
            adapter(80, NpcMoodStore.class,
                () -> NpcMoodStore.upsert(new NpcMoodState(103, "hostile", 0.9, "凝脉", "旧局", 1_000L)),
                () -> NpcMoodStore.get(103) == null),
            adapter(81, OmenStateStore.class,
                () -> OmenStateStore.note(new VfxEventPayload.SpawnParticle(
                    new Identifier("bong", "world_omen_pseudo_vein"),
                    new double[] {1d, 2d, 3d},
                    java.util.Optional.empty(),
                    java.util.OptionalInt.empty(),
                    java.util.Optional.of(0.5d),
                    java.util.OptionalInt.empty(),
                    java.util.OptionalInt.of(20)
                ), 1_000L),
                () -> OmenStateStore.snapshot(1_000L).entries().isEmpty()),
            adapter(82, FreshnessStore.class,
                () -> FreshnessStore.upsert("old", 0.5f, "old-profile"),
                () -> FreshnessStore.get("old") == null),
            adapter(83, ProcessingSessionStore.class,
                () -> ProcessingSessionStore.replace(new ProcessingSessionStore.Snapshot(
                    true, "old", ProcessingSessionStore.Kind.DRYING, "old", 1, 2, "old-player")),
                () -> ProcessingSessionStore.Snapshot.empty().equals(ProcessingSessionStore.snapshot())),
            adapter(85, SkillMilestoneStore.class,
                () -> SkillMilestoneStore.replace(
                    List.of(new com.bong.client.skill.SkillMilestoneSnapshot(
                        com.bong.client.skill.SkillId.HERBALISM, 1, 1_000L, "old", 1L)),
                    "old"),
                () -> SkillMilestoneStore.snapshot().isEmpty() && SkillMilestoneStore.summary().isEmpty()),
            adapter(86, SkillRecentEventStore.class,
                () -> SkillRecentEventStore.append(new SkillRecentEventStore.Entry(
                    com.bong.client.skill.SkillId.HERBALISM, "old", "old", 1_000L)),
                () -> SkillRecentEventStore.snapshot().isEmpty()),
            adapter(87, SkillSetStore.class,
                () -> SkillSetStore.updateEntry(
                    com.bong.client.skill.SkillId.HERBALISM,
                    new com.bong.client.skill.SkillSetSnapshot.Entry(1, 1L, 2L, 1L, 10, 1L, 1_000L)),
                () -> SkillSetStore.snapshot().skills().isEmpty()
                    && SkillSetStore.snapshot().consumedScrolls().isEmpty()),
            adapter(88, NicheGuardianStore.class,
                () -> NicheGuardianStore.recordFatigue("old", 1),
                () -> NicheGuardianStore.guardianStatuses().isEmpty()
                    && NicheGuardianStore.intrusionAlerts().isEmpty()),
            adapter(89, SocialStateStore.class,
                () -> SocialStateStore.replaceAnonymity("old", List.of(
                    new SocialStateStore.SocialRemoteIdentity("old-remote", false, "old", "old", "old", List.of()))),
                () -> SocialStateStore.anonymity().equals(SocialStateStore.SocialAnonymitySnapshot.empty())
                    && SocialStateStore.exposures().isEmpty()
                    && SocialStateStore.relationships().isEmpty()
                    && SocialStateStore.renownDeltas().isEmpty()
                    && SocialStateStore.sparringInvite() == null
                    && SocialStateStore.tradeOffer() == null),
            adapter(92, PlayerStateStore.class,
                () -> PlayerStateStore.replace(com.bong.client.state.PlayerStateViewModel.create(
                    "old", "old", 1d, 2d, 0d, 0d,
                    com.bong.client.state.PlayerStateViewModel.PowerBreakdown.empty(),
                    com.bong.client.state.PlayerStateViewModel.SocialSnapshot.empty(),
                    "old", "old", 0d)),
                () -> PlayerStateStore.snapshot().isEmpty()),
            adapter(93, RealmCollapseHudStateStore.class,
                () -> RealmCollapseHudStateStore.replace(RealmCollapseHudState.create("old", "旧局", 1_000L, 100)),
                () -> RealmCollapseHudStateStore.snapshot().isEmpty()),
            adapter(94, SeasonStateStore.class,
                () -> SeasonStateStore.replace(new com.bong.client.state.SeasonState(
                    com.bong.client.state.SeasonState.Phase.WINTER, 1L, 2L, 1L)),
                () -> SeasonStateStore.snapshot().equals(com.bong.client.state.SeasonState.summerAt(0L))),
            adapter(95, TiandaoPresenceStore.class,
                () -> TiandaoPresenceStore.replace(activeTiandaoPresence()),
                () -> !TiandaoPresenceStore.snapshot().active()),
            adapter(96, ExtractStateStore.class,
                () -> ExtractStateStore.markStarted(1L, "old", 2, 1_000L),
                () -> com.bong.client.tsy.ExtractState.empty().equals(ExtractStateStore.snapshot())),
            adapter(98, TsyContainerStateStore.class,
                () -> TsyContainerStateStore.upsert(new com.bong.client.tsy.TsyContainerView(
                    1L, "old", "old", 1d, 2d, 3d, "", false, "")),
                () -> TsyContainerStateStore.snapshot().isEmpty()),
            adapter(99, TsyDeathVfxStore.class,
                () -> TsyDeathVfxStore.trigger(1_000L),
                () -> TsyDeathVfxState.empty().equals(TsyDeathVfxStore.snapshot())),
            adapter(103, YidaoHudStateStore.class,
                () -> YidaoHudStateStore.replace(new YidaoHudStateStore.Snapshot(
                    "old", 1, 1f, 1d, "old", List.of("old"), 0.5f, 0.5d, 1, 1, 1)),
                () -> YidaoHudStateStore.Snapshot.EMPTY.equals(YidaoHudStateStore.snapshot())),
            adapter(104, YidaoNpcAiStateStore.class,
                () -> YidaoNpcAiStateStore.replace(new YidaoNpcAiStateStore.Snapshot(
                    "old", "old", 1, 1, false)),
                () -> YidaoNpcAiStateStore.activeCount() == 0
                    && YidaoNpcAiStateStore.Snapshot.EMPTY.equals(YidaoNpcAiStateStore.snapshot()))
        );
    }

    private static ProductionAdapterCase adapter(
        int index,
        Class<?> storeType,
        Runnable seed,
        BooleanSupplier isCleared
    ) {
        return new ProductionAdapterCase(index, storeType, seed, isCleared);
    }

    private static TiandaoPresenceState activeTiandaoPresence() {
        return new TiandaoPresenceState(
            true,
            "pressure",
            50.0,
            "old_zone",
            0.5,
            0x400000,
            0.2,
            0.3,
            0.9,
            1_000L
        );
    }

    private record ProductionAdapterCase(
        int index,
        Class<?> storeType,
        Runnable seed,
        BooleanSupplier isCleared
    ) {
        @Override
        public String toString() {
            return index + ":" + storeType.getSimpleName();
        }
    }

    @Test
    void registryPreservesLongLivedStoreResourcesAcrossNewSessionWrites() {
        AtomicInteger alchemyNotifications = new AtomicInteger();
        AtomicInteger castNotifications = new AtomicInteger();
        AtomicInteger castTransitions = new AtomicInteger();
        AtomicInteger quickUseNotifications = new AtomicInteger();
        AtomicInteger bodyPlanNotifications = new AtomicInteger();
        AtomicInteger meridianNotifications = new AtomicInteger();
        AtomicInteger morphNotifications = new AtomicInteger();
        AtomicInteger physicalBodyNotifications = new AtomicInteger();
        AtomicInteger raceGateNotifications = new AtomicInteger();
        AlchemySessionStore.addListener(ignored -> alchemyNotifications.incrementAndGet());
        CastStateStore.addListener(ignored -> castNotifications.incrementAndGet());
        CastStateStore.addTransitionListener((state, origin) -> castTransitions.incrementAndGet());
        QuickUseSlotStore.Update subscribed = QuickUseSlotStore.subscribeAndGet(
            ignored -> quickUseNotifications.incrementAndGet()
        );
        BodyPlanLayoutStore.addListener(ignored -> bodyPlanNotifications.incrementAndGet());
        MeridianStateStore.addListener(ignored -> meridianNotifications.incrementAndGet());
        MorphStateStore.addListener(morphNotifications::incrementAndGet);
        PhysicalBodyStore.addListener(ignored -> physicalBodyNotifications.incrementAndGet());
        RaceGateMetaStore.addListener(raceGateNotifications::incrementAndGet);

        AlchemySessionStore.Snapshot oldAlchemy = new AlchemySessionStore.Snapshot(
            "old", true, 1, 10, 1f, 2f, 0.5f, 1d, 2d, "old", List.of(), List.of()
        );
        AlchemySessionStore.replace(oldAlchemy);
        CastStateStore.beginSkillBarCast(3, 100, 1_000L);
        QuickUseSlotStore.replaceAuthoritative(
            QuickSlotConfig.of(
                new QuickSlotEntry[] {new QuickSlotEntry("old", "old", 1, 1, "")},
                new long[] {2_000L}
            ),
            "old-ack",
            true
        );
        BodyPlanLayout oldLayout = new BodyPlanLayout("old", List.of(), List.of(), List.of(), List.of());
        BodyPlanLayoutStore.putLayout(oldLayout);
        BodyPlanLayoutStore.setCurrentPlanId("old");
        MeridianStateStore.replace(MeridianBody.builder().realm("old").build());
        MorphStateStore.applyDelta(1, new MorphEntry(0, "old", "old"));
        PhysicalBodyStore.replace(PhysicalBody.builder().build());
        RaceGateMetaStore.replace(
            Map.of("old", new RaceGate(RaceGate.KIND_HUMANOID, List.of())),
            Map.of("old-skill", new RaceGate(RaceGate.KIND_HUMANOID, List.of()))
        );
        long sequenceBeforeClear = QuickUseSlotStore.subscribeAndGet(ignored -> {}).sequence();
        assertTrue(sequenceBeforeClear > subscribed.sequence(), "旧 session 写入必须推进 quick-use 单调 sequence");

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertEquals(AlchemySessionStore.Snapshot.empty(), AlchemySessionStore.snapshot());
        assertSame(com.bong.client.combat.CastState.idle(), CastStateStore.snapshot());
        QuickUseSlotStore.Update clearedQuickUse = QuickUseSlotStore.subscribeAndGet(ignored -> {});
        assertSame(QuickSlotConfig.empty(), clearedQuickUse.config());
        assertEquals(QuickUseSlotStore.Source.LOCAL, clearedQuickUse.source());
        assertNull(clearedQuickUse.ackRequestId());
        assertNull(clearedQuickUse.bindAccepted());
        assertTrue(clearedQuickUse.sequence() > sequenceBeforeClear, "production clear 不得回退 quick-use sequence");
        assertNull(BodyPlanLayoutStore.currentPlanId());
        assertNull(BodyPlanLayoutStore.current());
        assertNull(BodyPlanLayoutStore.byId("old"), "production clear 必须清完整 body-plan cache");
        assertNull(MeridianStateStore.snapshot());
        assertTrue(MorphStateStore.morphOf(1).isEmpty());
        assertNull(PhysicalBodyStore.snapshot());
        assertFalse(RaceGateMetaStore.hasReceived());
        assertNull(RaceGateMetaStore.gateForItem("old"));
        assertNull(RaceGateMetaStore.gateForTechnique("old-skill"));
        assertEquals(2, alchemyNotifications.get(), "炼丹 listener 必须收到旧态与断线空态");
        assertEquals(2, castNotifications.get(), "施法 listener 必须收到旧态与断线 idle");
        assertEquals(2, castTransitions.get(), "施法 transition listener 必须收到旧态与断线 idle");
        assertEquals(2, quickUseNotifications.get(), "quick-use listener 必须收到旧态与断线空态");
        assertEquals(2, bodyPlanNotifications.get(), "body-plan listener 必须收到旧态与断线空态");
        assertEquals(2, meridianNotifications.get(), "经脉 listener 必须收到旧态与断线空态");
        assertEquals(2, morphNotifications.get(), "易形 listener 必须收到旧态与断线空态");
        assertEquals(2, physicalBodyNotifications.get(), "肉身 listener 必须收到旧态与断线空态");
        assertEquals(2, raceGateNotifications.get(), "种族门 listener 必须收到旧态与断线空态");

        AlchemySessionStore.Snapshot freshAlchemy = new AlchemySessionStore.Snapshot(
            "fresh", true, 2, 20, 2f, 3f, 0.25f, 2d, 3d, "fresh", List.of(), List.of()
        );
        AlchemySessionStore.replace(freshAlchemy);
        CastStateStore.beginCast(2, 200, 2_000L);
        QuickUseSlotStore.replaceAuthoritative(
            QuickSlotConfig.of(
                new QuickSlotEntry[] {new QuickSlotEntry("fresh", "fresh", 1, 1, "")},
                new long[] {3_000L}
            ),
            "fresh-ack",
            true
        );
        BodyPlanLayout freshLayout = new BodyPlanLayout("fresh", List.of(), List.of(), List.of(), List.of());
        BodyPlanLayoutStore.putLayout(freshLayout);
        BodyPlanLayoutStore.setCurrentPlanId("fresh");
        MeridianStateStore.replace(MeridianBody.builder().realm("fresh").build());
        MorphStateStore.applyDelta(2, new MorphEntry(0, "fresh", "fresh"));
        PhysicalBody freshPhysicalBody = PhysicalBody.builder().build();
        PhysicalBodyStore.replace(freshPhysicalBody);
        RaceGate freshGate = new RaceGate(RaceGate.KIND_HUMANOID, List.of());
        RaceGateMetaStore.replace(Map.of("fresh", freshGate), Map.of("fresh-skill", freshGate));

        assertEquals(freshAlchemy, AlchemySessionStore.snapshot());
        assertTrue(CastStateStore.snapshot().isCasting());
        assertEquals("fresh", QuickUseSlotStore.snapshot().slot(0).itemId());
        assertTrue(QuickUseSlotStore.subscribeAndGet(ignored -> {}).sequence() > clearedQuickUse.sequence());
        assertSame(freshLayout, BodyPlanLayoutStore.current());
        assertEquals("fresh", MeridianStateStore.snapshot().realm());
        assertTrue(MorphStateStore.morphOf(2).isPresent());
        assertSame(freshPhysicalBody, PhysicalBodyStore.snapshot());
        assertSame(freshGate, RaceGateMetaStore.gateForItem("fresh"));
        assertEquals(3, alchemyNotifications.get(), "原炼丹 listener 必须接收新 session 写入");
        assertEquals(3, castNotifications.get(), "原施法 listener 必须接收新 session 写入");
        assertEquals(3, castTransitions.get(), "原施法 transition listener 必须接收新 session 写入");
        assertEquals(3, quickUseNotifications.get(), "原 quick-use listener 必须接收新 session 写入");
        assertEquals(3, bodyPlanNotifications.get(), "原 body-plan listener 必须接收新 session 写入");
        assertEquals(3, meridianNotifications.get(), "原经脉 listener 必须接收新 session 写入");
        assertEquals(3, morphNotifications.get(), "原易形 listener 必须接收新 session 写入");
        assertEquals(3, physicalBodyNotifications.get(), "原肉身 listener 必须接收新 session 写入");
        assertEquals(3, raceGateNotifications.get(), "原种族门 listener 必须接收新 session 写入");
    }

    @Test
    void registryPreservesDispatcherStreamAndSkillBarListenerResources() {
        AtomicInteger dispatched = new AtomicInteger();
        AtomicInteger offers = new AtomicInteger();
        AtomicInteger skillBarNotifications = new AtomicInteger();
        InsightOfferStore.setDispatcher(decision -> dispatched.incrementAndGet());
        var dispatcher = InsightOfferStore.dispatcher();
        InsightOfferStore.addListener(ignored -> offers.incrementAndGet());
        SkillBarStore.addListener(ignored -> skillBarNotifications.incrementAndGet());
        var eventStream = UnifiedEventStore.stream();
        InsightOfferViewModel oldOffer = new InsightOfferViewModel(
            "old", "old", "old", 0.5d, 1, 1, 2_000L,
            List.of(new InsightChoice("old", InsightCategory.QI, "old", "old", "old", ""))
        );
        InsightOfferStore.replace(oldOffer);
        SkillBarStore.replace(SkillBarConfig.of(
            new SkillBarEntry[] {SkillBarEntry.item("old", "old", 0, 0, "")},
            new long[] {2_000L}
        ));
        SkillBarStore.setSelectedSlot(0);
        eventStream.publish(
            UnifiedEvent.Channel.COMBAT,
            UnifiedEvent.Priority.P2_NORMAL,
            "old",
            "old",
            0,
            1L
        );

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertNull(InsightOfferStore.snapshot());
        assertSame(dispatcher, InsightOfferStore.dispatcher(), "production clear 不得替换 insight dispatcher seam");
        assertSame(eventStream, UnifiedEventStore.stream(), "production clear 不得替换 HUD 持有的 event stream");
        assertTrue(eventStream.snapshot().isEmpty());
        assertSame(SkillBarConfig.empty(), SkillBarStore.snapshot());
        assertEquals(SkillBarStore.NO_SELECTED_SLOT, SkillBarStore.selectedSlot());
        assertEquals(2, offers.get(), "insight listener 必须收到旧态与断线空态");
        assertEquals(2, skillBarNotifications.get(), "skill-bar listener 必须收到旧态与断线空态");

        InsightOfferViewModel freshOffer = new InsightOfferViewModel(
            "fresh", "fresh", "fresh", 0.5d, 1, 1, 3_000L,
            List.of(new InsightChoice("fresh", InsightCategory.QI, "fresh", "fresh", "fresh", ""))
        );
        InsightOfferStore.replace(freshOffer);
        InsightOfferStore.submit(InsightDecision.chosen("fresh", "fresh"));
        SkillBarStore.updateSlot(1, SkillBarEntry.item("fresh", "fresh", 0, 0, ""));
        eventStream.publish(
            UnifiedEvent.Channel.WORLD,
            UnifiedEvent.Priority.P2_NORMAL,
            "fresh",
            "fresh",
            0,
            2_000L
        );

        assertEquals(1, dispatched.get(), "registry clear 后必须继续通过同一 dispatcher 派发新 session 决策");
        assertEquals(4, offers.get(), "原 insight listener 必须接收新 offer 与 submit 空态");
        assertEquals(3, skillBarNotifications.get(), "原 skill-bar listener 必须接收新 session 写入");
        assertEquals(1, eventStream.snapshot().size(), "保留的 event stream 必须接收新 session event");
    }

    @Test
    void productionRegisteredStoresDeclareCanonicalCleaner() throws Exception {
        String registrySource = Files.readString(
            productionSourceRoot().resolve("com/bong/client/lifecycle/SessionScopedStoreRegistry.java")
        );
        for (SessionStoreHandle handle : SessionScopedStoreRegistry.registeredHandlesForTests()) {
            String simpleName = handle.storeType().getSimpleName();
            List<String> cleanerNames = registeredCleanerNames(registrySource, simpleName);
            assertEquals(
                List.of("clearOnDisconnect"),
                cleanerNames,
                "每个 production Store 必须恰好登记 canonical cleaner clearOnDisconnect：" + handle.fqcn()
            );
            Path source = productionSourceRoot().resolve(handle.fqcn().replace('.', '/') + ".java");
            assertTrue(Files.exists(source), "registry handle 必须对应 production Store source：" + handle.fqcn());
            JavaLifecycleSourceInspector.assertDeclaresProductionCleaner(
                Files.readString(source),
                cleanerNames.get(0),
                handle.fqcn()
            );
        }
    }

    @Test
    void productionSourcesConfineTestResetCallsToTestResetMethods() throws Exception {
        Path sourceRoot = productionSourceRoot();
        try (Stream<Path> paths = Files.walk(sourceRoot.resolve("com/bong/client"))) {
            List<Path> sources = paths
                .filter(Files::isRegularFile)
                .filter(path -> path.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList();
            assertFalse(sources.isEmpty(), "production source guard 必须实际扫描 client Java source");
            for (Path source : sources) {
                JavaLifecycleSourceInspector.assertTestResetCallsAreConfinedToTestResetMethods(
                    Files.readString(source),
                    sourceRoot.relativize(source).toString()
                );
            }
        }
    }

    @Test
    void productionCleanerDeclarationGuardTargetsDeclaredStoreWhenHelperTypePrecedesIt() {
        String fixture = """
            final class PreludeType {
                static void clearOnDisconnect() { }
            }
            final class FixtureStore {
                public static void clearOnDisconnect() { }
            }
            """;

        assertDoesNotThrow(() -> JavaLifecycleSourceInspector.assertDeclaresProductionCleaner(
            fixture,
            "clearOnDisconnect",
            "com.example.FixtureStore"
        ));
    }

    @Test
    void productionCleanerDeclarationGuardRejectsMissingDeclaredStoreType() {
        AssertionError failure = assertThrows(
            AssertionError.class,
            () -> JavaLifecycleSourceInspector.assertDeclaresProductionCleaner(
                "final class DifferentStore { static void clearOnDisconnect() { } }",
                "clearOnDisconnect",
                "com.example.FixtureStore"
            )
        );
        assertTrue(
            failure.getMessage().contains("无法定位 production Store 类型"),
            "source guard 必须 fail-closed，不能把其它顶层类型当成目标 Store；实际=" + failure.getMessage()
        );
    }

    @Test
    void productionTestResetGuardRejectsEveryDirectCallOrReferenceOutsideTestResetMethods() {
        List<String> forbiddenFixtures = List.of(
            """
                public final class FixtureStore {
                    public static void clearOnDisconnect() { Helper.clear(); }
                    public static void resetForTests() { }
                }
                final class Helper {
                    static void clear() { FixtureStore.resetForTests(); }
                }
                """,
            """
                public final class FixtureStore {
                    static final Runnable RESET = FixtureStore::resetForTests;
                    public static void clearOnDisconnect() { RESET.run(); }
                    public static void resetForTests() { }
                }
                """,
            """
                public final class FixtureStore {
                    public static void clearOnDisconnect() { Helper.clear(); }
                    public static void resetForTest() { }
                    static final class Helper {
                        static void clear() { FixtureStore.resetForTest(); }
                    }
                }
                """,
            """
                public final class FixtureStore {
                    FixtureStore() { clearForTests(); }
                    public static void clearOnDisconnect() { new FixtureStore(); }
                    public static void clearForTests() { }
                }
                """
        );

        for (String fixture : forbiddenFixtures) {
            AssertionError failure = assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertTestResetCallsAreConfinedToTestResetMethods(
                    fixture,
                    "FixtureStore.java"
                )
            );
            assertTrue(
                failure.getMessage().contains("test reset"),
                "字段、构造器、嵌套或跨顶层 helper 的 test reset 直连都必须撞红；实际="
                    + failure.getMessage()
            );
        }
    }

    @Test
    void productionTestResetGuardAllowsTestResetCompositionAndUnrelatedOverloads() {
        String fixture = """
            public final class FixtureStore {
                private static final java.util.List<String> CACHE = java.util.List.of();
                public static void clearOnDisconnect() { CACHE.clear(); }
                public static void clear() { }
                public static void clear(int unused) { }
                public static void resetForTests() { clearForTests(); }
                public static void clearForTests() { }
            }
            """;

        assertDoesNotThrow(() -> JavaLifecycleSourceInspector.assertTestResetCallsAreConfinedToTestResetMethods(
            fixture,
            "FixtureStore.java"
        ));
    }

    @Test
    void registryClearsRealmNpcTsyAndCoffinAdaptersThenAcceptsFreshState() {
        RealmCollapseHudStateStore.replace(
            RealmCollapseHudState.create("old_zone", "旧局坍缩", 1_000L, 100)
        );
        NpcMetadata oldMetadata = metadata(10, "旧客");
        NpcMetadataStore.upsert(oldMetadata);
        NpcLodSnapshot oldLod = new NpcLodSnapshot(11, "rogue", "引气", 0.8f, 1.0, 64.0, 2.0);
        NpcLodStore.upsert(oldLod);
        NpcMoodStore.upsert(new NpcMoodState(12, "hostile", 0.9, "凝脉", "旧局杀意", 1_000L));
        TsyBossHealthState oldBoss = new TsyBossHealthState(true, "旧守灵", "通灵", 0.66, 2, 4, 1_000L);
        TsyBossHealthStore.replace(oldBoss);
        TsyDeathVfxStore.trigger(1_000L);
        CoffinStateStore.State oldCoffin = new CoffinStateStore.State(true, 0.7, "jade");
        CoffinStateStore.replace(oldCoffin);

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertTrue(RealmCollapseHudStateStore.snapshot().isEmpty(), "registry 必须清空旧坍缩 HUD");
        assertTrue(NpcMetadataStore.snapshot().isEmpty(), "registry 必须清空全部 NPC metadata");
        assertEquals(0, NpcLodStore.size(), "registry 必须清空全部 NPC LOD snapshot");
        assertTrue(NpcMoodStore.snapshot().isEmpty(), "registry 必须清空全部 NPC mood");
        assertEquals(TsyBossHealthState.empty(), TsyBossHealthStore.snapshot(), "TSY boss adapter 必须绑定 reset");
        assertEquals(TsyDeathVfxState.empty(), TsyDeathVfxStore.snapshot(), "TSY death VFX adapter 必须绑定 reset");
        assertEquals(CoffinStateStore.OUT, CoffinStateStore.snapshot(), "卧棺 adapter 必须绑定 clear");

        RealmCollapseHudStateStore.replace(
            RealmCollapseHudState.create("fresh_zone", "新局坍缩", 2_000L, 100)
        );
        NpcMetadata freshMetadata = metadata(20, "新客");
        NpcMetadataStore.upsert(freshMetadata);
        NpcLodSnapshot freshLod = new NpcLodSnapshot(21, "beast", "凝脉", 0.3f, 3.0, 65.0, 4.0);
        NpcLodStore.upsert(freshLod);
        NpcMoodStore.upsert(new NpcMoodState(22, "alert", 0.4, null, null, 2_000L));
        TsyBossHealthState freshBoss = new TsyBossHealthState(true, "新守灵", "固元", 0.5, 1, 3, 2_000L);
        TsyBossHealthStore.replace(freshBoss);
        TsyDeathVfxStore.trigger(2_000L);
        CoffinStateStore.State freshCoffin = new CoffinStateStore.State(true, 0.5, "stone");
        CoffinStateStore.replace(freshCoffin);

        assertEquals("fresh_zone", RealmCollapseHudStateStore.snapshot().zone());
        assertEquals(freshMetadata, NpcMetadataStore.get(20));
        assertEquals(freshLod, NpcLodStore.get(21));
        assertEquals("alert", NpcMoodStore.get(22).mood());
        assertEquals(freshBoss, TsyBossHealthStore.snapshot());
        assertEquals(2_000L, TsyDeathVfxStore.snapshot().startedAtMillis());
        assertEquals(freshCoffin, CoffinStateStore.snapshot());
    }

    @Test
    void registryClearsGatheringBaomaiAndVoidAdaptersThenAcceptsFreshState() {
        AtomicInteger gatheringNotifications = new AtomicInteger();
        GatheringSessionStore.addListener(state -> gatheringNotifications.incrementAndGet());
        GatheringSessionStore.replace(gathering("old-gather", 1_000L));
        CrackReadingHudStateStore.accept(
            42L,
            List.of(new CrackReadingHudStateStore.MeridianEntry("Lung", "MicroTear", true, false)),
            true,
            1_000L
        );
        ResonanceLockHudStateStore.onLockStarted("offline:old", 10L, 30L);
        VoidErosionVisualStore.replace("offline:old-a", 4, 420.0, true, 0.4f, true);
        VoidErosionVisualStore.replace("offline:old-b", 2, 90.0, false, 0.7f, false);

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertTrue(GatheringSessionStore.snapshot().isEmpty(), "registry 必须清空旧采集 session");
        assertEquals(2, gatheringNotifications.get(), "采集 clear 必须通知且不得删除长期 listener");
        assertSame(CrackReadingHudStateStore.State.EMPTY, CrackReadingHudStateStore.snapshot());
        assertSame(ResonanceLockHudStateStore.State.UNLOCKED, ResonanceLockHudStateStore.snapshot());
        assertTrue(VoidErosionVisualStore.allSnapshots().isEmpty(), "VoidErosion reset 必须清整个 entity cache");
        assertNull(VoidErosionVisualStore.snapshotForEntity("offline:old-a"));
        assertNull(VoidErosionVisualStore.snapshotForEntity("offline:old-b"));

        GatheringSessionStore.replace(gathering("fresh-gather", 2_000L));
        CrackReadingHudStateStore.accept(
            84L,
            List.of(new CrackReadingHudStateStore.MeridianEntry("Heart", "Severed", false, true)),
            false,
            1_000L
        );
        ResonanceLockHudStateStore.onLockStarted("offline:fresh", 40L, 60L);
        VoidErosionVisualStore.replace("offline:fresh", 1, 25.0, false, 0.85f, false);

        assertEquals("fresh-gather", GatheringSessionStore.snapshot().sessionId());
        assertEquals(3, gatheringNotifications.get(), "新 session replace 仍必须通知旧采集 listener");
        assertEquals(84L, CrackReadingHudStateStore.snapshot().targetEntityId);
        assertEquals("offline:fresh", ResonanceLockHudStateStore.snapshot().partnerId);
        assertNotNull(VoidErosionVisualStore.snapshotForEntity("offline:fresh"));
        assertEquals(1, VoidErosionVisualStore.snapshotForEntity("offline:fresh").stage());
    }

    @Test
    void registryClearsHallucinationAndDyingElderCompletelyThenAcceptsFreshState() {
        HallucinationLayerStore.activate(200);
        HallucinationLayerStore.updateBarOffsets(0.15f, -0.18f);
        HallucinationLayerStore.tickFade(0.5f, 0.1f, 0.05f);
        DyingElderEncounterStore.activate("旧域", 42, 1_000L);
        DyingElderEncounterStore.update("dan_received", 1_001L);
        DyingElderEncounterStore.setBetrayProbability(0.8);
        DyingElderEncounterStore.setQiFraction(0.7f);
        DyingElderEncounterStore.setSpiritEyeActive(true);

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertFalse(HallucinationLayerStore.isActive());
        assertEquals(0, HallucinationLayerStore.getRemainingTicks());
        assertEquals(0, HallucinationLayerStore.getDurationTicks());
        assertEquals(0.0f, HallucinationLayerStore.getFadeProgress());
        assertEquals(0.0f, HallucinationLayerStore.getHpBarDisplayOffset());
        assertEquals(0.0f, HallucinationLayerStore.getQiBarDisplayOffset());
        assertEquals(0.0f, HallucinationLayerStore.getSinPhase());
        assertFalse(DyingElderEncounterStore.isActive());
        assertEquals("", DyingElderEncounterStore.getZoneDisplayName());
        assertEquals(0, DyingElderEncounterStore.getElderEntityId());
        assertEquals("appeared", DyingElderEncounterStore.getEventKind());
        assertNull(DyingElderEncounterStore.getBetrayProbability());
        assertEquals(0.0f, DyingElderEncounterStore.getQiFraction());
        assertFalse(DyingElderEncounterStore.isSpiritEyeActive());
        assertEquals(0L, DyingElderEncounterStore.getReceivedTick());

        HallucinationLayerStore.activate(100);
        DyingElderEncounterStore.activate("新域", 43, 2_000L);

        assertTrue(HallucinationLayerStore.isActive());
        assertEquals(100, HallucinationLayerStore.getRemainingTicks());
        assertTrue(DyingElderEncounterStore.isActive());
        assertEquals("新域", DyingElderEncounterStore.getZoneDisplayName());
        assertEquals(43, DyingElderEncounterStore.getElderEntityId());
    }

    @Test
    void registryClearsHudAndAgentUiActiveScreenThenAcceptsFreshState() {
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            ZoneState.create("old_zone", "旧域", 0.08, 6, 1_000L),
            NarrationState.create("broadcast", null, "旧旁白", "narration"),
            VisualEffectState.create("near_death_vignette", 0.9, 30_000L, 0L)
        ));
        SearchHudStateStore.markStarted("旧石匣", 100);
        AgentUiScreen oldScreen = screen("old-active");
        AgentUiStore.setActive(oldScreen);

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        BongHudStateSnapshot clearedHud = BongHudStateStore.snapshot();
        assertTrue(clearedHud.isEmpty());
        assertTrue(clearedHud.zoneState().isEmpty());
        assertTrue(clearedHud.narrationState().isEmpty());
        assertTrue(clearedHud.visualEffectState().isEmpty());
        assertEquals(SearchHudState.idle(), SearchHudStateStore.snapshot());
        assertNull(AgentUiStore.getActive(), "AgentUi adapter 必须清 activeScreen");

        BongHudStateSnapshot freshHud = BongHudStateSnapshot.create(
            ZoneState.create("fresh_zone", "新域", 0.95, 0, 2_000L),
            NarrationState.empty(),
            VisualEffectState.none()
        );
        BongHudStateStore.replace(freshHud);
        SearchHudStateStore.markStarted("新骨匣", 80);
        AgentUiScreen freshScreen = screen("fresh-active");
        AgentUiStore.setActive(freshScreen);

        assertSame(freshHud, BongHudStateStore.snapshot());
        assertEquals("新骨匣", SearchHudStateStore.snapshot().containerKindZh());
        assertSame(freshScreen, AgentUiStore.getActive());
    }

    @Test
    void registryClearsAgentUiPendingErrorClose() {
        ClientRequestSender.setBackendForTests((channel, payload) -> {});
        AgentUiScreen pendingScreen = screen("old-pending");
        AgentUiStore.setActive(pendingScreen);
        pendingScreen.close();
        assertNull(AgentUiStore.getActive(), "本地关闭后 activeScreen 应先清空");

        SessionScopedStoreRegistry.clearAllOnDisconnect();
        AgentUiStore.receiveClose("old-pending", "session_expired");

        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "registry 必须清 pendingErrorClose；否则迟到错误 close 会向新 session 弹旧 toast"
        );
    }

    @Test
    void registryClearsCombatCoffinAndInventoryAdaptersThenAcceptsFreshState() {
        HalfStepRechallengeStore.replace(new HalfStepRechallengeStore.State(true, "old-char", 300L, 200L, 1_000L));
        TutorialCoffinPosStore.set(new BlockPos(1, 64, 1));
        RemainsStore.putOrReplace(remains("old-remains", 1.0));
        DroppedItemStore.putOrReplace(drop(1L, "old-item", 1.0));
        FalseSkinHudStateStore.replace(falseSkin("old-player"));
        DuguV2HudStateStore.replace(dugu("旧局中毒", true));

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertEquals(HalfStepRechallengeStore.State.NONE, HalfStepRechallengeStore.snapshot());
        assertTrue(TutorialCoffinPosStore.snapshot().isEmpty());
        assertTrue(RemainsStore.snapshot().isEmpty());
        assertNull(RemainsStore.get("old-remains"));
        assertNull(RemainsStore.nearestTo(0.0, 0.0, 0.0));
        assertTrue(DroppedItemStore.snapshot().isEmpty());
        assertNull(DroppedItemStore.get(1L));
        assertNull(DroppedItemStore.nearestTo(0.0, 0.0, 0.0));
        assertEquals(FalseSkinHudStateStore.State.NONE, FalseSkinHudStateStore.snapshot());
        assertEquals(DuguV2HudStateStore.State.NONE, DuguV2HudStateStore.snapshot());
        assertFalse(DuguV2HudStateStore.snapshot().selfRevealed());
        assertEquals(0f, DuguV2HudStateStore.snapshot().revealRisk());

        HalfStepRechallengeStore.State freshHalfStep =
            new HalfStepRechallengeStore.State(true, "fresh-char", 400L, 300L, 2_000L);
        HalfStepRechallengeStore.replace(freshHalfStep);
        BlockPos freshPos = new BlockPos(2, 65, 2);
        TutorialCoffinPosStore.set(freshPos);
        RemainsStore.Entry freshRemains = remains("fresh-remains", 2.0);
        RemainsStore.putOrReplace(freshRemains);
        DroppedItemStore.Entry freshDrop = drop(2L, "fresh-item", 2.0);
        DroppedItemStore.putOrReplace(freshDrop);
        FalseSkinHudStateStore.State freshFalseSkin = falseSkin("fresh-player");
        FalseSkinHudStateStore.replace(freshFalseSkin);
        DuguV2HudStateStore.State freshDugu = dugu("新局中毒", false);
        DuguV2HudStateStore.replace(freshDugu);

        assertEquals(freshHalfStep, HalfStepRechallengeStore.snapshot());
        assertEquals(freshPos, TutorialCoffinPosStore.snapshot().orElseThrow());
        assertEquals(freshRemains, RemainsStore.get("fresh-remains"));
        assertEquals(freshDrop, DroppedItemStore.get(2L));
        assertEquals(freshFalseSkin, FalseSkinHudStateStore.snapshot());
        assertEquals(freshDugu, DuguV2HudStateStore.snapshot());
    }

    @Test
    void registryClearsCraftAndIdentityDataWithoutDeletingLongLivedListeners() {
        AtomicInteger recipeNotifications = new AtomicInteger();
        AtomicInteger sessionNotifications = new AtomicInteger();
        AtomicInteger outcomeNotifications = new AtomicInteger();
        AtomicInteger unlockNotifications = new AtomicInteger();
        AtomicInteger identityNotifications = new AtomicInteger();
        CraftStore.addRecipeListener(recipes -> recipeNotifications.incrementAndGet());
        CraftStore.addSessionListener(session -> sessionNotifications.incrementAndGet());
        CraftStore.addOutcomeListener(outcome -> outcomeNotifications.incrementAndGet());
        CraftStore.addUnlockListener(unlock -> unlockNotifications.incrementAndGet());
        IdentityPanelStateStore.addListener(state -> identityNotifications.incrementAndGet());

        CraftRecipe oldRecipe = recipe("old-recipe");
        CraftStore.replaceRecipes(List.of(oldRecipe));
        CraftStore.replaceSession(new CraftSessionStateView(true, oldRecipe.id(), 5L, 10L));
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(oldRecipe.id(), "old-output", 1, 10L));
        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(
            oldRecipe.id(),
            new CraftStore.RecipeUnlockedEvent.Scroll("old-scroll"),
            11L
        ));
        IdentityPanelState oldIdentity = identity(1, "旧身份");
        IdentityPanelStateStore.replace(oldIdentity);
        assertEquals(1, recipeNotifications.get());
        assertEquals(1, sessionNotifications.get());
        assertEquals(1, outcomeNotifications.get());
        assertEquals(1, unlockNotifications.get());
        assertEquals(1, identityNotifications.get());

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertTrue(CraftStore.recipes().isEmpty());
        assertEquals(CraftSessionStateView.IDLE, CraftStore.sessionState());
        assertTrue(CraftStore.lastOutcome().isEmpty());
        assertTrue(CraftStore.lastUnlocked().isEmpty());
        assertEquals(2, recipeNotifications.get(), "Craft clear 必须通知且不得删除长期 recipe listener");
        assertEquals(2, sessionNotifications.get(), "Craft clear 必须通知且不得删除长期 session listener");
        assertEquals(1, outcomeNotifications.get(), "clear 不得伪造空 outcome 事件或删除 listener");
        assertEquals(1, unlockNotifications.get(), "clear 不得伪造空 unlock 事件或删除 listener");
        assertEquals(IdentityPanelState.empty(), IdentityPanelStateStore.snapshot());
        assertEquals(2, identityNotifications.get(), "Identity clear 必须通知且不得删除长期 listener");

        CraftRecipe freshRecipe = recipe("fresh-recipe");
        CraftStore.replaceRecipes(List.of(freshRecipe));
        CraftStore.replaceSession(new CraftSessionStateView(true, freshRecipe.id(), 15L, 20L));
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(freshRecipe.id(), "fresh-output", 2, 20L));
        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(
            freshRecipe.id(),
            new CraftStore.RecipeUnlockedEvent.Insight("fresh-insight"),
            21L
        ));
        IdentityPanelState freshIdentity = identity(2, "新身份");
        IdentityPanelStateStore.replace(freshIdentity);

        assertEquals(freshRecipe, CraftStore.recipe(freshRecipe.id()).orElseThrow());
        assertEquals(3, recipeNotifications.get(), "新 session replace 仍必须通知旧 listener");
        assertEquals(3, sessionNotifications.get(), "新 session state 仍必须通知旧 listener");
        assertEquals(2, outcomeNotifications.get(), "registry clear 后 outcome listener 仍必须工作");
        assertEquals(2, unlockNotifications.get(), "registry clear 后 unlock listener 仍必须工作");
        assertEquals(freshIdentity, IdentityPanelStateStore.snapshot());
        assertEquals(3, identityNotifications.get(), "新 session identity 仍必须通知旧 listener");
    }

    private static NpcMetadata metadata(int id, String name) {
        return new NpcMetadata(id, "rogue", "凝脉", null, null, 0, name, "壮年", "……", null);
    }

    private static GatheringSessionViewModel gathering(String id, long updatedAtMillis) {
        return GatheringSessionViewModel.create(
            id, 1L, 2L, "残草", "herb", "fine", "rough_hoe", false, false, updatedAtMillis
        );
    }

    private static AgentUiScreen screen(String requestId) {
        return AgentUiScreen.create(requestId, SCREEN_XML, 600, 0L);
    }

    private static RemainsStore.Entry remains(String id, double x) {
        return new RemainsStore.Entry(
            id, x, 64.0, x, "minecraft:overworld", "遗骸", 3, 12L
        );
    }

    private static DroppedItemStore.Entry drop(long instanceId, String itemId, double x) {
        return new DroppedItemStore.Entry(
            instanceId,
            "main_pack",
            0,
            0,
            x,
            64.0,
            x,
            InventoryItem.simple(itemId, itemId)
        );
    }

    private static CraftRecipe recipe(String id) {
        return new CraftRecipe(
            id,
            CraftCategory.MISC,
            id,
            List.of(),
            0.0,
            0L,
            "output_" + id,
            1,
            CraftRecipe.Requirements.NONE,
            true
        );
    }

    private static IdentityPanelState identity(int id, String name) {
        return new IdentityPanelState(
            id,
            0L,
            0L,
            List.of(new IdentityPanelEntry(id, name, 0, false, List.of()))
        );
    }

    private static FalseSkinHudStateStore.State falseSkin(String targetId) {
        return new FalseSkinHudStateStore.State(
            targetId, "rotten_wood_armor", 1, 10f, 1f, 1L, List.of()
        );
    }

    private static DuguV2HudStateStore.State dugu(String hint, boolean revealed) {
        return new DuguV2HudStateStore.State(
            true, 0.8f, hint, 0.6f, 70f, revealed, true, 9L, 1f, 99f, 10L
        );
    }

    private static Path productionSourceRoot() {
        Path workingDirectory = Path.of("").toAbsolutePath().normalize();
        Path clientRoot = Files.isDirectory(workingDirectory.resolve("src"))
            ? workingDirectory
            : workingDirectory.resolve("client");
        return clientRoot.resolve("src/main/java");
    }

    private static List<String> registeredCleanerNames(String registrySource, String storeSimpleName) {
        java.util.regex.Pattern registration = java.util.regex.Pattern.compile(
            "SessionStoreHandle\\.forStore\\(\\s*"
                + java.util.regex.Pattern.quote(storeSimpleName)
                + "\\.class,\\s*"
                + java.util.regex.Pattern.quote(storeSimpleName)
                + "::([A-Za-z0-9_]+)\\s*\\)",
            java.util.regex.Pattern.DOTALL
        );
        java.util.regex.Matcher matcher = registration.matcher(registrySource);
        List<String> names = new java.util.ArrayList<>();
        while (matcher.find()) {
            names.add(matcher.group(1));
        }
        return names;
    }

    private static void resetTestOnlyState() {
        RealmCollapseHudStateStore.resetForTests();
        NpcMetadataStore.clearAll();
        NpcLodStore.clearAll();
        NpcMoodStore.clearAll();
        TsyBossHealthStore.resetForTests();
        TsyDeathVfxStore.resetForTests();
        CoffinStateStore.resetForTests();
        GatheringSessionStore.resetForTests();
        CrackReadingHudStateStore.clear();
        ResonanceLockHudStateStore.clear();
        VoidErosionVisualStore.reset();
        HallucinationLayerStore.clearOnDisconnect();
        DyingElderEncounterStore.clearOnDisconnect();
        TiandaoPresenceStore.clear();
        BongHudStateStore.clear();
        SearchHudStateStore.resetForTests();
        AgentUiStore.clear();
        HalfStepRechallengeStore.resetForTests();
        TutorialCoffinPosStore.resetForTests();
        RemainsStore.resetForTests();
        DroppedItemStore.resetForTests();
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        IdentityPanelStateStore.resetForTest();
        FalseSkinHudStateStore.resetForTests();
        DuguV2HudStateStore.resetForTests();
        BongToast.resetForTests();
    }
}
