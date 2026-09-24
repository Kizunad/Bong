package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.alchemy.state.ContaminationWarningStore;
import com.bong.client.alchemy.state.InventoryMetaStore;
import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.skill.SkillSetSnapshot;

import java.util.List;
import java.util.Objects;

/** 炼丹屏唯一可读的不可变状态投影，隔离 UI 与各个领域 Store。 */
public record AlchemyScreenViewModel(
    long revision,
    Change change,
    RecipeScrollStore.Snapshot recipes,
    AlchemyFurnaceStore.Snapshot furnace,
    AlchemySessionStore.Snapshot session,
    InventoryModel inventory,
    SkillSetSnapshot skills,
    InventoryMetaStore.Snapshot inventoryMeta,
    AlchemyOutcomeForecastStore.Snapshot outcome,
    List<AlchemyAttemptHistoryStore.Entry> history,
    ContaminationWarningStore.Snapshot contamination
) {
    public AlchemyScreenViewModel {
        if (revision < 0L) {
            throw new IllegalArgumentException("revision must be >= 0");
        }
        Objects.requireNonNull(change, "change must not be null");
        Objects.requireNonNull(recipes, "recipes must not be null");
        Objects.requireNonNull(furnace, "furnace must not be null");
        Objects.requireNonNull(session, "session must not be null");
        Objects.requireNonNull(inventory, "inventory must not be null");
        Objects.requireNonNull(skills, "skills must not be null");
        Objects.requireNonNull(inventoryMeta, "inventoryMeta must not be null");
        Objects.requireNonNull(outcome, "outcome must not be null");
        history = List.copyOf(Objects.requireNonNull(history, "history must not be null"));
        Objects.requireNonNull(contamination, "contamination must not be null");
    }

    public enum Change {
        INITIAL,
        RECIPES,
        FURNACE,
        SESSION,
        INVENTORY,
        SKILLS,
        PRESENTATION
    }
}
