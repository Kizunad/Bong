package com.bong.client.craft;

import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

/** plan-craft-ux-v1 — 左栏配方搜索、分类与收藏排序。 */
public final class CraftRecipeFilter {
    private CraftRecipeFilter() {}

    public static List<CraftRecipe> filter(
        List<CraftRecipe> recipes,
        CraftCategory category,
        String query,
        Set<String> favorites
    ) {
        String normalizedQuery = normalize(query);
        Set<String> favoriteIds = favorites == null ? Set.of() : new LinkedHashSet<>(favorites);
        return recipes.stream()
            .filter(recipe -> category == null || recipe.category() == category)
            .filter(recipe -> matches(recipe, normalizedQuery))
            .sorted(Comparator
                .comparing((CraftRecipe recipe) -> !favoriteIds.contains(recipe.id()))
                .thenComparing(recipe -> !recipe.unlocked())
                .thenComparing(CraftRecipe::displayName)
                .thenComparing(CraftRecipe::id))
            .toList();
    }

    public static boolean matches(CraftRecipe recipe, String query) {
        String q = normalize(query);
        if (q.isEmpty()) {
            return true;
        }
        if (recipe == null) {
            return false;
        }
        if (!recipe.unlocked() && !"???".contains(q)) {
            return false;
        }
        if (contains(recipe.displayName(), q)
            || contains(recipe.id(), q)
            || contains(recipe.outputTemplate(), q)
            || contains(recipe.category().displayName(), q)) {
            return true;
        }
        for (CraftRecipe.MaterialEntry material : recipe.materials()) {
            if (contains(material.templateId(), q)) {
                return true;
            }
        }
        return false;
    }

    public static String displayName(CraftRecipe recipe) {
        if (recipe == null) {
            return "";
        }
        return recipe.unlocked() ? recipe.displayName() : "???";
    }

    public static String unlockHint(CraftRecipe recipe) {
        if (recipe == null || recipe.unlocked()) {
            return "";
        }
        List<String> requirements = recipe.requirements().humanLines();
        if (!requirements.isEmpty()) {
            return String.join(" / ", requirements);
        }
        return "引气 / 残卷 / 师承";
    }

    private static boolean contains(String value, String query) {
        return normalize(value).contains(query);
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim().toLowerCase(Locale.ROOT);
    }
}
