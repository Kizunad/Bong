package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.skill.SkillSetSnapshot;

import java.util.List;
import java.util.Objects;
import java.util.Optional;

/** 手搓屏幕唯一可读的不可变状态投影。 */
public record CraftScreenViewModel(
    long revision,
    Change change,
    List<CraftRecipe> recipes,
    InventoryModel inventory,
    SkillSetSnapshot skills,
    CraftSessionStateView session,
    Optional<CraftOutcomeView> latestOutcome
) {
    public CraftScreenViewModel {
        if (revision < 0L) {
            throw new IllegalArgumentException("revision must be >= 0");
        }
        Objects.requireNonNull(change, "change must not be null");
        recipes = List.copyOf(Objects.requireNonNull(recipes, "recipes must not be null"));
        Objects.requireNonNull(inventory, "inventory must not be null");
        Objects.requireNonNull(skills, "skills must not be null");
        Objects.requireNonNull(session, "session must not be null");
        Objects.requireNonNull(latestOutcome, "latestOutcome must not be null");
    }

    public Optional<CraftRecipe> recipe(String recipeId) {
        if (recipeId == null) {
            return Optional.empty();
        }
        return recipes.stream().filter(recipe -> recipe.id().equals(recipeId)).findFirst();
    }

    public enum Change {
        INITIAL,
        RECIPES,
        SESSION,
        OUTCOME,
        INVENTORY,
        SKILLS
    }
}
