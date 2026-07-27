package com.bong.client.lifecycle;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiStore;
import com.bong.client.coffin.TutorialCoffinPosStore;
import com.bong.client.combat.baomai.v4.CrackReadingHudStateStore;
import com.bong.client.combat.baomai.v4.ResonanceLockHudStateStore;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.combat.store.HalfStepRechallengeStore;
import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.dying_elder.DyingElderEncounterStore;
import com.bong.client.fauna.HallucinationLayerStore;
import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.gathering.GatheringSessionViewModel;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.CoffinStateStore;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.inventory.state.RemainsStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.npc.NpcLodSnapshot;
import com.bong.client.npc.NpcLodStore;
import com.bong.client.npc.NpcMetadata;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.npc.NpcMoodState;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.state.NarrationState;
import com.bong.client.state.RealmCollapseHudState;
import com.bong.client.state.RealmCollapseHudStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import com.bong.client.tiandao.TiandaoPresenceState;
import com.bong.client.tiandao.TiandaoPresenceStore;
import com.bong.client.tsy.TsyBossHealthState;
import com.bong.client.tsy.TsyBossHealthStore;
import com.bong.client.tsy.TsyDeathVfxState;
import com.bong.client.tsy.TsyDeathVfxStore;
import com.bong.client.visual.VoidErosionVisualStore;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.MethodSource;

import java.util.List;
import java.util.Map;
import java.util.function.BooleanSupplier;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SessionScopedStoreRegistryProductionAdapterTest {
    private static final String SCREEN_XML =
        "<owo-ui><components><flow-layout direction=\"vertical\"/></components></owo-ui>";

    @BeforeEach
    void setUp() {
        resetTestOnlyState();
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        resetTestOnlyState();
    }

    @ParameterizedTest(name = "{0}")
    @MethodSource("productionAdapters")
    void eachRegisteredHandleClearsOnlyItsDeclaredStore(ProductionAdapterCase adapter) {
        List<SessionStoreHandle> handles = SessionScopedStoreRegistry.registeredHandlesForTests();
        assertEquals(25, handles.size(), "P1 必须对生产 REGISTERED 的全部 25 个 handle 逐项验真");
        SessionStoreHandle handle = handles.get(adapter.index());
        assertSame(
            adapter.storeType(),
            handle.storeType(),
            "测试 case 必须按生产声明顺序取得对应 Class handle，不能重建 cleaner 映射"
        );

        adapter.seed().run();
        Map<Class<?>, StoreWitness> witnesses = storeWitnesses();
        witnesses.entrySet().stream()
            .filter(entry -> entry.getKey() != adapter.storeType())
            .forEach(entry -> entry.getValue().seed().run());
        assertFalse(adapter.isCleared().getAsBoolean(), "测试前必须建立目标 Store 的旧 session 状态：" + adapter);
        witnesses.entrySet().stream()
            .filter(entry -> entry.getKey() != adapter.storeType())
            .forEach(entry -> assertFalse(
                entry.getValue().isCleared().getAsBoolean(),
                "测试前必须建立非目标 Store 的旁观状态：" + entry.getKey().getSimpleName()
            ));

        handle.clearOnDisconnect();

        assertTrue(adapter.isCleared().getAsBoolean(), "声明 handle 必须清掉其自身 Store：" + adapter);
        witnesses.entrySet().stream()
            .filter(entry -> entry.getKey() != adapter.storeType())
            .forEach(entry -> assertFalse(
                entry.getValue().isCleared().getAsBoolean(),
                "单 handle 只能清声明 Store，不得误清旁观 Store："
                    + adapter + " -> " + entry.getKey().getSimpleName()
            ));
    }

    private static Map<Class<?>, StoreWitness> storeWitnesses() {
        return productionAdapters().collect(java.util.stream.Collectors.toUnmodifiableMap(
            ProductionAdapterCase::storeType,
            adapter -> new StoreWitness(adapter.seed(), adapter.isCleared())
        ));
    }

    private static Stream<ProductionAdapterCase> productionAdapters() {
        return Stream.of(
            adapter(0, RealmCollapseHudStateStore.class,
                () -> RealmCollapseHudStateStore.replace(RealmCollapseHudState.create("old", "旧局", 1_000L, 100)),
                () -> RealmCollapseHudStateStore.snapshot().isEmpty()),
            adapter(1, NpcMetadataStore.class,
                () -> NpcMetadataStore.upsert(metadata(101, "旧客")),
                () -> NpcMetadataStore.get(101) == null),
            adapter(2, NpcLodStore.class,
                () -> NpcLodStore.upsert(new NpcLodSnapshot(102, "rogue", "引气", 0.8f, 1.0, 64.0, 2.0)),
                () -> NpcLodStore.get(102) == null),
            adapter(3, NpcMoodStore.class,
                () -> NpcMoodStore.upsert(new NpcMoodState(103, "hostile", 0.9, "凝脉", "旧局", 1_000L)),
                () -> NpcMoodStore.get(103) == null),
            adapter(4, TsyBossHealthStore.class,
                () -> TsyBossHealthStore.replace(new TsyBossHealthState(true, "旧守灵", "通灵", 0.66, 2, 4, 1_000L)),
                () -> TsyBossHealthState.empty().equals(TsyBossHealthStore.snapshot())),
            adapter(5, TsyDeathVfxStore.class,
                () -> TsyDeathVfxStore.trigger(1_000L),
                () -> TsyDeathVfxState.empty().equals(TsyDeathVfxStore.snapshot())),
            adapter(6, CoffinStateStore.class,
                () -> CoffinStateStore.replace(new CoffinStateStore.State(true, 0.7, "jade")),
                () -> CoffinStateStore.OUT.equals(CoffinStateStore.snapshot())),
            adapter(7, GatheringSessionStore.class,
                () -> GatheringSessionStore.replace(gathering("old-gather", 1_000L)),
                () -> GatheringSessionStore.snapshot().isEmpty()),
            adapter(8, CrackReadingHudStateStore.class,
                () -> CrackReadingHudStateStore.accept(108L, List.of(
                    new CrackReadingHudStateStore.MeridianEntry("Lung", "MicroTear", true, false)), true, 1_000L),
                () -> CrackReadingHudStateStore.snapshot() == CrackReadingHudStateStore.State.EMPTY),
            adapter(9, ResonanceLockHudStateStore.class,
                () -> ResonanceLockHudStateStore.onLockStarted("offline:old", 10L, 30L),
                () -> ResonanceLockHudStateStore.snapshot() == ResonanceLockHudStateStore.State.UNLOCKED),
            adapter(10, VoidErosionVisualStore.class,
                () -> {
                    VoidErosionVisualStore.replace("offline:old-a", 4, 420.0, true, 0.4f, true);
                    VoidErosionVisualStore.replace("offline:old-b", 2, 90.0, false, 0.7f, false);
                },
                () -> VoidErosionVisualStore.allSnapshots().isEmpty()),
            adapter(11, HallucinationLayerStore.class,
                () -> HallucinationLayerStore.activate(200),
                () -> !HallucinationLayerStore.isActive() && HallucinationLayerStore.getRemainingTicks() == 0),
            adapter(12, DyingElderEncounterStore.class,
                () -> DyingElderEncounterStore.activate("旧域", 112, 1_000L),
                () -> !DyingElderEncounterStore.isActive() && DyingElderEncounterStore.getElderEntityId() == 0),
            adapter(13, TiandaoPresenceStore.class,
                () -> TiandaoPresenceStore.replace(activeTiandaoPresence()),
                () -> !TiandaoPresenceStore.snapshot().active()),
            adapter(14, BongHudStateStore.class,
                () -> BongHudStateStore.replace(BongHudStateSnapshot.create(
                    ZoneState.create("old", "旧域", 0.08, 6, 1_000L),
                    NarrationState.empty(),
                    VisualEffectState.none())),
                () -> BongHudStateStore.snapshot().isEmpty()),
            adapter(15, SearchHudStateStore.class,
                () -> SearchHudStateStore.markStarted("旧石匣", 100),
                () -> SearchHudState.idle().equals(SearchHudStateStore.snapshot())),
            adapter(16, AgentUiStore.class,
                () -> AgentUiStore.setActive(screen("old-active")),
                () -> AgentUiStore.getActive() == null),
            adapter(17, HalfStepRechallengeStore.class,
                () -> HalfStepRechallengeStore.replace(
                    new HalfStepRechallengeStore.State(true, "old-char", 300L, 200L, 1_000L)),
                () -> HalfStepRechallengeStore.State.NONE.equals(HalfStepRechallengeStore.snapshot())),
            adapter(18, TutorialCoffinPosStore.class,
                () -> TutorialCoffinPosStore.set(new BlockPos(1, 64, 1)),
                () -> TutorialCoffinPosStore.snapshot().isEmpty()),
            adapter(19, RemainsStore.class,
                () -> RemainsStore.putOrReplace(remains("old-remains", 1.0)),
                () -> RemainsStore.snapshot().isEmpty()),
            adapter(20, DroppedItemStore.class,
                () -> DroppedItemStore.putOrReplace(drop(1L, "old-item", 1.0)),
                () -> DroppedItemStore.snapshot().isEmpty()),
            adapter(21, CraftStore.class,
                () -> CraftStore.replaceRecipes(List.of(recipe("old-recipe"))),
                () -> CraftStore.recipes().isEmpty()),
            adapter(22, IdentityPanelStateStore.class,
                () -> IdentityPanelStateStore.replace(identity(1, "旧身份")),
                () -> IdentityPanelState.empty().equals(IdentityPanelStateStore.snapshot())),
            adapter(23, FalseSkinHudStateStore.class,
                () -> FalseSkinHudStateStore.replace(falseSkin("old-player")),
                () -> FalseSkinHudStateStore.State.NONE.equals(FalseSkinHudStateStore.snapshot())),
            adapter(24, DuguV2HudStateStore.class,
                () -> DuguV2HudStateStore.replace(dugu("旧局中毒", true)),
                () -> DuguV2HudStateStore.State.NONE.equals(DuguV2HudStateStore.snapshot()))
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

    private record StoreWitness(Runnable seed, BooleanSupplier isCleared) {
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
