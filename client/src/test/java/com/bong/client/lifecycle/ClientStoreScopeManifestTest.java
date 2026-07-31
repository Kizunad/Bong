package com.bong.client.lifecycle;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ClientStoreScopeManifestTest {
    private static final String LIFECYCLE_INTERFACE =
        "com.bong.client.lifecycle.SessionScopedStore";
    private static final List<String> P2_REGISTERED_STORES = List.of(
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
        "com.bong.client.visual.VoidErosionVisualStore",
        "com.bong.client.visual.realm_vision.PerceptionEdgeStateStore",
        "com.bong.client.visual.realm_vision.RealmVisionStateStore",
        "com.bong.client.yidao.YidaoHudStateStore",
        "com.bong.client.yidao.YidaoNpcAiStateStore"
    );

    @Test
    void everyProductionStoreHasExactlyOneExplicitScope() throws IOException {
        Set<String> discovered = discoverProductionStores();
        Set<String> session = ClientStoreScopeManifest.sessionScopedStores();
        Set<String> persistent = ClientStoreScopeManifest.persistentConfigStores();
        Set<String> constant = ClientStoreScopeManifest.constantStores();

        assertDisjoint(session, persistent, "session-scoped", "persistent-config");
        assertDisjoint(session, constant, "session-scoped", "constant");
        assertDisjoint(persistent, constant, "persistent-config", "constant");

        assertEquals(
            discovered,
            new TreeSet<>(ClientStoreScopeManifest.allClassifiedStores()),
            "每个 production *Store.java 必须恰好落入 manifest 的一种 scope；新增 Store 时必须显式判定"
                + " session-scoped / persistent-config / constant，且 lifecycle interface 本身不能污染业务发现集"
        );
    }

    @Test
    void p0BaselineKeepsTheVerifiedThreeWayClassification() {
        assertEquals(
            106,
            ClientStoreScopeManifest.sessionScopedStores().size(),
            "P0 验真基线应有 106 个 session-scoped Store；数量变化时必须连同逐 FQCN source 对拍一起显式复核"
        );
        assertEquals(
            108,
            ClientStoreScopeManifest.allClassifiedStores().size(),
            "P0 验真基线应有 108 个业务 *Store.java（106 session + 1 persistent + 1 constant）"
        );
    }

    @Test
    void connectionStatusStoreRemainsTokenManagedOutsideTheGlobalRegistry() {
        assertEquals(
            Set.of("com.bong.client.ui.ClientConnectionStatusStore"),
            ClientStoreScopeManifest.externallyManagedSessionStores(),
            "连接状态 Store 必须由 handler token 精确失活，不能被无参全局 registry 清理"
        );
        assertFalse(
            ClientStoreScopeManifest.registryManagedSessionStores().contains(
                "com.bong.client.ui.ClientConnectionStatusStore"
            ),
            "ClientConnectionStatusStore 必须在 registry clear 之前由 invalidateSession(handler, now) 管理"
        );
        assertTrue(
            ClientStoreScopeManifest.sessionScopedStores().containsAll(
                ClientStoreScopeManifest.externallyManagedSessionStores()
            ),
            "externally managed 列表只能是 session-scoped Store 的子集"
        );
    }

    @Test
    void persistentPreferenceAndConstantLookupStayOutOfSessionScope() {
        assertEquals(
            Set.of("com.bong.client.hud.HudLayoutPreferenceStore"),
            ClientStoreScopeManifest.persistentConfigStores(),
            "HUD 布局是本地用户偏好，断线必须保留，不能误归 session-scoped"
        );
        assertEquals(
            Set.of("com.bong.client.combat.ArmorProfileStore"),
            ClientStoreScopeManifest.constantStores(),
            "护甲 profile 是固定查表，断线必须保留，不能误归 session-scoped"
        );
        assertFalse(
            ClientStoreScopeManifest.sessionScopedStores().contains(
                "com.bong.client.hud.HudLayoutPreferenceStore"
            ),
            "persistent-config Store 不得同时进入 session-scoped"
        );
        assertFalse(
            ClientStoreScopeManifest.sessionScopedStores().contains(
                "com.bong.client.combat.ArmorProfileStore"
            ),
            "constant Store 不得同时进入 session-scoped"
        );
    }

    @Test
    void p2RegistryMatchesTheOrderedCumulativeMigrationSetAndScope() {
        List<String> registeredFqcns = SessionScopedStoreRegistry.registeredFqcnsForTests();
        assertEquals(
            P2_REGISTERED_STORES,
            registeredFqcns,
            "P2 registry 必须严格按 manifest registry-managed session Store 的既定相对顺序累计登记 105 个 adapter；"
                + "漏项、错绑 Class 或顺序漂移都需要逐 Store 行为复核"
        );

        Set<String> registered = new HashSet<>(registeredFqcns);
        assertEquals(
            registered.size(),
            registeredFqcns.size(),
            "registry 不得重复登记同一 FQCN，否则断线会重复清理同一 Store"
        );
        assertTrue(
            ClientStoreScopeManifest.registryManagedSessionStores().containsAll(registered),
            "P2 registry 只允许登记 manifest 中由全局 registry 管理的 session Store；实际越界="
                + difference(registered, ClientStoreScopeManifest.registryManagedSessionStores())
        );
        assertFalse(
            registered.contains("com.bong.client.ui.ClientConnectionStatusStore"),
            "ClientConnectionStatusStore 必须继续由 invalidateSession(handler, now) 管理，不能进入无参 registry"
        );
        assertFalse(
            registered.contains("com.bong.client.hud.HudLayoutPreferenceStore"),
            "HudLayoutPreferenceStore 是跨 session 的本地偏好，不能进入 registry"
        );
        assertFalse(
            registered.contains("com.bong.client.combat.ArmorProfileStore"),
            "ArmorProfileStore 是固定查表，不能进入 registry"
        );
    }

    private static Set<String> discoverProductionStores() throws IOException {
        Path javaRoot = ClientSourceTree.clientRoot().resolve("src/main/java");
        Path clientPackage = javaRoot.resolve("com/bong/client");
        TreeSet<String> discovered = new TreeSet<>();
        try (Stream<Path> paths = Files.walk(clientPackage)) {
            paths.filter(Files::isRegularFile)
                .filter(path -> path.getFileName().toString().endsWith("Store.java"))
                .map(path -> toFqcn(javaRoot.relativize(path)))
                .filter(fqcn -> !LIFECYCLE_INTERFACE.equals(fqcn))
                .forEach(discovered::add);
        }
        return discovered;
    }

    private static String toFqcn(Path relativeJavaPath) {
        int nameCount = relativeJavaPath.getNameCount();
        String fileName = relativeJavaPath.getName(nameCount - 1).toString();
        String simpleName = fileName.substring(0, fileName.length() - ".java".length());
        StringBuilder fqcn = new StringBuilder();
        for (int index = 0; index < nameCount - 1; index++) {
            if (index > 0) {
                fqcn.append('.');
            }
            fqcn.append(relativeJavaPath.getName(index));
        }
        return fqcn.append('.').append(simpleName).toString();
    }

    private static void assertDisjoint(
        Set<String> left,
        Set<String> right,
        String leftLabel,
        String rightLabel
    ) {
        Set<String> overlap = new TreeSet<>(left);
        overlap.retainAll(right);
        assertTrue(
            overlap.isEmpty(),
            "Store scope 必须互斥；" + leftLabel + " 与 " + rightLabel + " 重叠=" + overlap
        );
    }

    private static Set<String> difference(Set<String> left, Set<String> right) {
        Set<String> difference = new TreeSet<>(left);
        difference.removeAll(right);
        return difference;
    }
}
