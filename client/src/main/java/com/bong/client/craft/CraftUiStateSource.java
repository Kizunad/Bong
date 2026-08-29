package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.state.StoreUiStateSource;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Consumer;

/** 把 Craft、背包和技艺 Store 合成为屏幕可消费的单一状态源。 */
public final class CraftUiStateSource implements UiStateSource<CraftScreenViewModel> {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-craft-ui-state");

    private final UiStateSource<List<CraftRecipe>> recipes;
    private final UiStateSource<CraftSessionStateView> session;
    private final UiStateSource<Optional<CraftOutcomeView>> outcome;
    private final UiStateSource<InventoryModel> inventory;
    private final UiStateSource<SkillSetSnapshot> skills;
    private final AtomicLong revision = new AtomicLong();

    private CraftUiStateSource(
        UiStateSource<List<CraftRecipe>> recipes,
        UiStateSource<CraftSessionStateView> session,
        UiStateSource<Optional<CraftOutcomeView>> outcome,
        UiStateSource<InventoryModel> inventory,
        UiStateSource<SkillSetSnapshot> skills
    ) {
        this.recipes = Objects.requireNonNull(recipes, "recipes source must not be null");
        this.session = Objects.requireNonNull(session, "session source must not be null");
        this.outcome = Objects.requireNonNull(outcome, "outcome source must not be null");
        this.inventory = Objects.requireNonNull(inventory, "inventory source must not be null");
        this.skills = Objects.requireNonNull(skills, "skills source must not be null");
    }

    public static CraftUiStateSource production() {
        return new CraftUiStateSource(
            StoreUiStateSource.push(
                CraftStore::recipes,
                listener -> registerRecipeListener(listener)
            ),
            StoreUiStateSource.push(
                CraftStore::sessionState,
                listener -> registerSessionListener(listener)
            ),
            StoreUiStateSource.push(
                () -> CraftStore.lastOutcome().map(CraftOutcomeView::from),
                listener -> registerOutcomeListener(listener)
            ),
            StoreUiStateSource.push(
                InventoryStateStore::snapshot,
                listener -> registerInventoryListener(listener)
            ),
            StoreUiStateSource.push(
                SkillSetStore::snapshot,
                listener -> registerSkillListener(listener)
            )
        );
    }

    @Override
    public CraftScreenViewModel snapshot() {
        return read(CraftScreenViewModel.Change.INITIAL, revision.get());
    }

    @Override
    public UiSubscription subscribe(Consumer<? super CraftScreenViewModel> listener) {
        Objects.requireNonNull(listener, "listener must not be null");
        List<UiSubscription> subscriptions = new ArrayList<>();
        try {
            subscriptions.add(recipes.subscribe(ignored -> publish(CraftScreenViewModel.Change.RECIPES, listener)));
            subscriptions.add(session.subscribe(ignored -> publish(CraftScreenViewModel.Change.SESSION, listener)));
            subscriptions.add(outcome.subscribe(ignored -> publish(CraftScreenViewModel.Change.OUTCOME, listener)));
            subscriptions.add(inventory.subscribe(ignored -> publish(CraftScreenViewModel.Change.INVENTORY, listener)));
            subscriptions.add(skills.subscribe(ignored -> publish(CraftScreenViewModel.Change.SKILLS, listener)));
            return UiSubscriptions.combine(subscriptions.toArray(UiSubscription[]::new));
        } catch (Throwable registrationFailure) {
            closePartialSubscriptions(subscriptions, registrationFailure);
            throw new AssertionError("unreachable");
        }
    }

    private void publish(
        CraftScreenViewModel.Change change,
        Consumer<? super CraftScreenViewModel> listener
    ) {
        CraftScreenViewModel next = read(change, revision.incrementAndGet());
        try {
            listener.accept(next);
        } catch (RuntimeException failure) {
            // UI 消费者失败不能反向中断 Store 的 authoritative 状态写入或后续 listener。
            LOGGER.error("Craft UI state listener failed at revision {}", next.revision(), failure);
        }
    }

    private CraftScreenViewModel read(CraftScreenViewModel.Change change, long currentRevision) {
        return new CraftScreenViewModel(
            currentRevision,
            change,
            recipes.snapshot(),
            inventory.snapshot(),
            skills.snapshot(),
            session.snapshot(),
            outcome.snapshot()
        );
    }

    private static UiSubscription registerRecipeListener(Consumer<? super List<CraftRecipe>> listener) {
        Consumer<List<CraftRecipe>> adapter = listener::accept;
        CraftStore.addRecipeListener(adapter);
        return UiSubscriptions.once(() -> CraftStore.removeRecipeListener(adapter));
    }

    private static UiSubscription registerSessionListener(Consumer<? super CraftSessionStateView> listener) {
        Consumer<CraftSessionStateView> adapter = listener::accept;
        CraftStore.addSessionListener(adapter);
        return UiSubscriptions.once(() -> CraftStore.removeSessionListener(adapter));
    }

    private static UiSubscription registerOutcomeListener(
        Consumer<? super Optional<CraftOutcomeView>> listener
    ) {
        Consumer<CraftStore.CraftOutcomeEvent> adapter = event ->
            listener.accept(Optional.of(CraftOutcomeView.from(event)));
        CraftStore.addOutcomeListener(adapter);
        return UiSubscriptions.once(() -> CraftStore.removeOutcomeListener(adapter));
    }

    private static UiSubscription registerInventoryListener(Consumer<? super InventoryModel> listener) {
        Consumer<InventoryModel> adapter = listener::accept;
        InventoryStateStore.addListener(adapter);
        return UiSubscriptions.once(() -> InventoryStateStore.removeListener(adapter));
    }

    private static UiSubscription registerSkillListener(Consumer<? super SkillSetSnapshot> listener) {
        Consumer<SkillSetSnapshot> adapter = listener::accept;
        SkillSetStore.addListener(adapter);
        return UiSubscriptions.once(() -> SkillSetStore.removeListener(adapter));
    }

    private static void closePartialSubscriptions(
        List<UiSubscription> subscriptions,
        Throwable registrationFailure
    ) {
        try {
            UiSubscriptions.combine(subscriptions.toArray(UiSubscription[]::new)).close();
        } catch (Throwable closeFailure) {
            if (registrationFailure != closeFailure) {
                registrationFailure.addSuppressed(closeFailure);
            }
        }
        CraftUiStateSource.<RuntimeException>throwUnchecked(registrationFailure);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
