package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.alchemy.state.ContaminationWarningStore;
import com.bong.client.alchemy.state.InventoryMetaStore;
import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.state.StoreUiStateSource;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Consumer;

/** 将炼丹相关 Store 聚合成一个屏幕级 source；没有 listener 的旧 Store 明确按 open 时读取。 */
public final class AlchemyUiStateSource implements UiStateSource<AlchemyScreenViewModel> {
    private final UiStateSource<RecipeScrollStore.Snapshot> recipes;
    private final UiStateSource<AlchemyFurnaceStore.Snapshot> furnace;
    private final UiStateSource<AlchemySessionStore.Snapshot> session;
    private final UiStateSource<com.bong.client.inventory.model.InventoryModel> inventory;
    private final UiStateSource<com.bong.client.skill.SkillSetSnapshot> skills;
    private final UiStateSource<InventoryMetaStore.Snapshot> inventoryMeta;
    private final UiStateSource<AlchemyOutcomeForecastStore.Snapshot> outcome;
    private final UiStateSource<List<AlchemyAttemptHistoryStore.Entry>> history;
    private final UiStateSource<ContaminationWarningStore.Snapshot> contamination;
    private final AtomicLong revision = new AtomicLong();

    private AlchemyUiStateSource(
        UiStateSource<RecipeScrollStore.Snapshot> recipes,
        UiStateSource<AlchemyFurnaceStore.Snapshot> furnace,
        UiStateSource<AlchemySessionStore.Snapshot> session,
        UiStateSource<com.bong.client.inventory.model.InventoryModel> inventory,
        UiStateSource<com.bong.client.skill.SkillSetSnapshot> skills,
        UiStateSource<InventoryMetaStore.Snapshot> inventoryMeta,
        UiStateSource<AlchemyOutcomeForecastStore.Snapshot> outcome,
        UiStateSource<List<AlchemyAttemptHistoryStore.Entry>> history,
        UiStateSource<ContaminationWarningStore.Snapshot> contamination
    ) {
        this.recipes = Objects.requireNonNull(recipes, "recipes source must not be null");
        this.furnace = Objects.requireNonNull(furnace, "furnace source must not be null");
        this.session = Objects.requireNonNull(session, "session source must not be null");
        this.inventory = Objects.requireNonNull(inventory, "inventory source must not be null");
        this.skills = Objects.requireNonNull(skills, "skills source must not be null");
        this.inventoryMeta = Objects.requireNonNull(inventoryMeta, "inventory meta source must not be null");
        this.outcome = Objects.requireNonNull(outcome, "outcome source must not be null");
        this.history = Objects.requireNonNull(history, "history source must not be null");
        this.contamination = Objects.requireNonNull(contamination, "contamination source must not be null");
    }

    public static AlchemyUiStateSource production() {
        return new AlchemyUiStateSource(
            StoreUiStateSource.pullOnOpen(RecipeScrollStore::snapshot),
            StoreUiStateSource.pullOnOpen(AlchemyFurnaceStore::snapshot),
            StoreUiStateSource.push(
                AlchemySessionStore::snapshot,
                listener -> {
                    Consumer<AlchemySessionStore.Snapshot> adapter = listener::accept;
                    AlchemySessionStore.addListener(adapter);
                    return UiSubscriptions.once(() -> AlchemySessionStore.removeListener(adapter));
                }
            ),
            StoreUiStateSource.push(
                InventoryStateStore::snapshot,
                listener -> {
                    Consumer<com.bong.client.inventory.model.InventoryModel> adapter = listener::accept;
                    InventoryStateStore.addListener(adapter);
                    return UiSubscriptions.once(() -> InventoryStateStore.removeListener(adapter));
                }
            ),
            StoreUiStateSource.push(
                SkillSetStore::snapshot,
                listener -> {
                    Consumer<com.bong.client.skill.SkillSetSnapshot> adapter = listener::accept;
                    SkillSetStore.addListener(adapter);
                    return UiSubscriptions.once(() -> SkillSetStore.removeListener(adapter));
                }
            ),
            StoreUiStateSource.pullOnOpen(InventoryMetaStore::snapshot),
            StoreUiStateSource.pullOnOpen(AlchemyOutcomeForecastStore::snapshot),
            StoreUiStateSource.pullOnOpen(AlchemyAttemptHistoryStore::snapshot),
            StoreUiStateSource.pullOnOpen(ContaminationWarningStore::snapshot)
        );
    }

    @Override
    public AlchemyScreenViewModel snapshot() {
        return read(AlchemyScreenViewModel.Change.INITIAL, revision.get());
    }

    @Override
    public UiSubscription subscribe(Consumer<? super AlchemyScreenViewModel> listener) {
        Objects.requireNonNull(listener, "listener must not be null");
        List<UiSubscription> subscriptions = new ArrayList<>();
        try {
            subscriptions.add(recipes.subscribe(ignored -> publish(AlchemyScreenViewModel.Change.RECIPES, listener)));
            subscriptions.add(furnace.subscribe(ignored -> publish(AlchemyScreenViewModel.Change.FURNACE, listener)));
            subscriptions.add(session.subscribe(ignored -> publish(AlchemyScreenViewModel.Change.SESSION, listener)));
            subscriptions.add(inventory.subscribe(ignored -> publish(AlchemyScreenViewModel.Change.INVENTORY, listener)));
            subscriptions.add(skills.subscribe(ignored -> publish(AlchemyScreenViewModel.Change.SKILLS, listener)));
            return UiSubscriptions.combine(subscriptions.toArray(UiSubscription[]::new));
        } catch (Throwable failure) {
            closePartial(subscriptions, failure);
            throw new AssertionError("unreachable");
        }
    }

    private void publish(
        AlchemyScreenViewModel.Change change,
        Consumer<? super AlchemyScreenViewModel> listener
    ) {
        try {
            listener.accept(read(change, revision.incrementAndGet()));
        } catch (RuntimeException ignored) {
            // UI listener 异常不能中断 Store 的 authoritative 更新。
        }
    }

    private AlchemyScreenViewModel read(AlchemyScreenViewModel.Change change, long currentRevision) {
        return new AlchemyScreenViewModel(
            currentRevision,
            change,
            recipes.snapshot(),
            furnace.snapshot(),
            session.snapshot(),
            inventory.snapshot(),
            skills.snapshot(),
            inventoryMeta.snapshot(),
            outcome.snapshot(),
            history.snapshot(),
            contamination.snapshot()
        );
    }

    private static void closePartial(List<UiSubscription> subscriptions, Throwable registrationFailure) {
        try {
            UiSubscriptions.combine(subscriptions.toArray(UiSubscription[]::new)).close();
        } catch (Throwable closeFailure) {
            if (registrationFailure != closeFailure) {
                registrationFailure.addSuppressed(closeFailure);
            }
        }
        AlchemyUiStateSource.<RuntimeException>throwUnchecked(registrationFailure);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
