package com.bong.client.lifecycle;

import com.bong.client.agentui.AgentUiStore;
import com.bong.client.coffin.TutorialCoffinPosStore;
import com.bong.client.combat.baomai.v4.CrackReadingHudStateStore;
import com.bong.client.combat.baomai.v4.ResonanceLockHudStateStore;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.combat.store.HalfStepRechallengeStore;
import com.bong.client.craft.CraftStore;
import com.bong.client.dying_elder.DyingElderEncounterStore;
import com.bong.client.fauna.HallucinationLayerStore;
import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.CoffinStateStore;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.inventory.state.RemainsStore;
import com.bong.client.npc.NpcLodStore;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.state.RealmCollapseHudStateStore;
import com.bong.client.tiandao.TiandaoPresenceStore;
import com.bong.client.tsy.TsyBossHealthStore;
import com.bong.client.tsy.TsyDeathVfxStore;
import com.bong.client.visual.VoidErosionVisualStore;

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
        SessionStoreHandle.forStore(
            RealmCollapseHudStateStore.class,
            RealmCollapseHudStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(NpcMetadataStore.class, NpcMetadataStore::clearAll),
        SessionStoreHandle.forStore(NpcLodStore.class, NpcLodStore::clearAll),
        SessionStoreHandle.forStore(NpcMoodStore.class, NpcMoodStore::clearAll),
        SessionStoreHandle.forStore(TsyBossHealthStore.class, TsyBossHealthStore::reset),
        SessionStoreHandle.forStore(TsyDeathVfxStore.class, TsyDeathVfxStore::reset),
        SessionStoreHandle.forStore(CoffinStateStore.class, CoffinStateStore::clear),
        SessionStoreHandle.forStore(
            GatheringSessionStore.class,
            GatheringSessionStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(CrackReadingHudStateStore.class, CrackReadingHudStateStore::clear),
        SessionStoreHandle.forStore(ResonanceLockHudStateStore.class, ResonanceLockHudStateStore::clear),
        SessionStoreHandle.forStore(VoidErosionVisualStore.class, VoidErosionVisualStore::reset),
        SessionStoreHandle.forStore(
            HallucinationLayerStore.class,
            HallucinationLayerStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(
            DyingElderEncounterStore.class,
            DyingElderEncounterStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(TiandaoPresenceStore.class, TiandaoPresenceStore::clear),
        SessionStoreHandle.forStore(BongHudStateStore.class, BongHudStateStore::clear),
        SessionStoreHandle.forStore(SearchHudStateStore.class, SearchHudStateStore::clearOnDisconnect),
        SessionStoreHandle.forStore(AgentUiStore.class, AgentUiStore::clear),
        SessionStoreHandle.forStore(HalfStepRechallengeStore.class, HalfStepRechallengeStore::clear),
        SessionStoreHandle.forStore(
            TutorialCoffinPosStore.class,
            TutorialCoffinPosStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(RemainsStore.class, RemainsStore::clearOnDisconnect),
        SessionStoreHandle.forStore(DroppedItemStore.class, DroppedItemStore::clearOnDisconnect),
        SessionStoreHandle.forStore(CraftStore.class, CraftStore::clear),
        SessionStoreHandle.forStore(
            IdentityPanelStateStore.class,
            IdentityPanelStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(
            FalseSkinHudStateStore.class,
            FalseSkinHudStateStore::clearOnDisconnect
        ),
        SessionStoreHandle.forStore(DuguV2HudStateStore.class, DuguV2HudStateStore::clearOnDisconnect)
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
