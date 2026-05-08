package com.bong.client.craft;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * plan-craft-v1 §3 — 客户端 craft 状态总仓。
 *
 * <p>三类数据：</p>
 * <ul>
 *   <li>{@link #recipes()} — 全配方表（`craft_recipe_list` 推一次后保存；
 *       `recipe_unlocked` 单条更新 unlocked 字段）</li>
 *   <li>{@link #sessionState()} — 当前进行中 session（`craft_session_state` 实时推）</li>
 *   <li>{@link #lastOutcome()} — 最近一次出炉结果（`craft_outcome`，UI 用于弹 toast）</li>
 * </ul>
 *
 * <p>三个状态各自独立 listener channel，避免无关 UI 重绘。</p>
 */
public final class CraftStore {
    private static final List<CraftRecipe> EMPTY_RECIPES = Collections.emptyList();

    private static volatile List<CraftRecipe> recipes = EMPTY_RECIPES;
    /** id → 同 recipe 引用（重复查找加速）。每次 replaceRecipes / 单 recipe 更新时重建。 */
    private static volatile Map<String, CraftRecipe> recipeById = Collections.emptyMap();

    private static volatile CraftSessionStateView session = CraftSessionStateView.IDLE;
    private static volatile CraftOutcomeEvent lastOutcome;
    private static volatile RecipeUnlockedEvent lastUnlocked;

    private static final List<Consumer<List<CraftRecipe>>> recipeListeners =
        new CopyOnWriteArrayList<>();
    private static final List<Consumer<CraftSessionStateView>> sessionListeners =
        new CopyOnWriteArrayList<>();
    private static final List<Consumer<CraftOutcomeEvent>> outcomeListeners =
        new CopyOnWriteArrayList<>();
    private static final List<Consumer<RecipeUnlockedEvent>> unlockListeners =
        new CopyOnWriteArrayList<>();

    private CraftStore() {}

    public static List<CraftRecipe> recipes() { return recipes; }
    public static Optional<CraftRecipe> recipe(String id) {
        return Optional.ofNullable(recipeById.get(id));
    }
    public static CraftSessionStateView sessionState() { return session; }
    public static Optional<CraftOutcomeEvent> lastOutcome() {
        return Optional.ofNullable(lastOutcome);
    }
    public static Optional<RecipeUnlockedEvent> lastUnlocked() {
        return Optional.ofNullable(lastUnlocked);
    }

    /** 全表替换（玩家上线一次性推或重新登录刷新）。 */
    public static void replaceRecipes(List<CraftRecipe> next) {
        Objects.requireNonNull(next, "recipes");
        List<CraftRecipe> snapshot = List.copyOf(next);
        Map<String, CraftRecipe> map = new HashMap<>(snapshot.size());
        for (CraftRecipe recipe : snapshot) {
            map.put(recipe.id(), recipe);
        }
        recipes = snapshot;
        recipeById = Collections.unmodifiableMap(map);
        for (Consumer<List<CraftRecipe>> l : recipeListeners) l.accept(snapshot);
    }

    /** 单条 recipe 解锁状态切换（按 id 查找；找不到则 noop）。 */
    public static void markRecipeUnlocked(String id) {
        Objects.requireNonNull(id, "id");
        if (!recipeById.containsKey(id)) return;
        List<CraftRecipe> updated = new ArrayList<>(recipes.size());
        boolean changed = false;
        for (CraftRecipe r : recipes) {
            if (r.id().equals(id) && !r.unlocked()) {
                updated.add(r.withUnlocked(true));
                changed = true;
            } else {
                updated.add(r);
            }
        }
        if (!changed) return;
        replaceRecipes(updated);
    }

    public static void replaceSession(CraftSessionStateView next) {
        Objects.requireNonNull(next, "session");
        if (next.equals(session)) return;
        session = next;
        for (Consumer<CraftSessionStateView> l : sessionListeners) l.accept(next);
    }

    public static void recordOutcome(CraftOutcomeEvent event) {
        Objects.requireNonNull(event, "event");
        lastOutcome = event;
        for (Consumer<CraftOutcomeEvent> l : outcomeListeners) l.accept(event);
    }

    public static void recordUnlock(RecipeUnlockedEvent event) {
        Objects.requireNonNull(event, "event");
        lastUnlocked = event;
        markRecipeUnlocked(event.recipeId());
        for (Consumer<RecipeUnlockedEvent> l : unlockListeners) l.accept(event);
    }

    public static void clear() {
        recipes = EMPTY_RECIPES;
        recipeById = Collections.emptyMap();
        session = CraftSessionStateView.IDLE;
        lastOutcome = null;
        lastUnlocked = null;
        for (Consumer<List<CraftRecipe>> l : recipeListeners) l.accept(EMPTY_RECIPES);
        for (Consumer<CraftSessionStateView> l : sessionListeners) l.accept(CraftSessionStateView.IDLE);
    }

    /**
     * 测试隔离用：清掉所有 listener。生产代码不应调用——listener 由 owner（Screen / Hud）
     * 显式 add/remove 自管。
     */
    public static void clearAllListenersForTests() {
        recipeListeners.clear();
        sessionListeners.clear();
        outcomeListeners.clear();
        unlockListeners.clear();
    }

    public static void addRecipeListener(Consumer<List<CraftRecipe>> listener) {
        recipeListeners.add(listener);
    }
    public static void removeRecipeListener(Consumer<List<CraftRecipe>> listener) {
        recipeListeners.remove(listener);
    }
    public static void addSessionListener(Consumer<CraftSessionStateView> listener) {
        sessionListeners.add(listener);
    }
    public static void removeSessionListener(Consumer<CraftSessionStateView> listener) {
        sessionListeners.remove(listener);
    }
    public static void addOutcomeListener(Consumer<CraftOutcomeEvent> listener) {
        outcomeListeners.add(listener);
    }
    public static void removeOutcomeListener(Consumer<CraftOutcomeEvent> listener) {
        outcomeListeners.remove(listener);
    }
    public static void addUnlockListener(Consumer<RecipeUnlockedEvent> listener) {
        unlockListeners.add(listener);
    }
    public static void removeUnlockListener(Consumer<RecipeUnlockedEvent> listener) {
        unlockListeners.remove(listener);
    }

    /** 出炉结果事件（成功 / 失败）。 */
    public record CraftOutcomeEvent(
        Kind kind,
        String recipeId,
        String outputTemplate,
        int outputCount,
        long completedAtTick,
        String failureReason,
        int materialReturned,
        double qiRefunded
    ) {
        public enum Kind { COMPLETED, FAILED }
        public CraftOutcomeEvent {
            Objects.requireNonNull(kind, "kind");
            Objects.requireNonNull(recipeId, "recipeId");
        }

        public static CraftOutcomeEvent completed(
            String recipeId, String outputTemplate, int outputCount, long completedAtTick
        ) {
            return new CraftOutcomeEvent(Kind.COMPLETED, recipeId,
                outputTemplate == null ? "" : outputTemplate,
                outputCount, completedAtTick, "", 0, 0.0);
        }

        public static CraftOutcomeEvent failed(
            String recipeId, String reasonWire, int materialReturned, double qiRefunded
        ) {
            return new CraftOutcomeEvent(Kind.FAILED, recipeId,
                "", 0, 0L, reasonWire == null ? "" : reasonWire,
                materialReturned, qiRefunded);
        }
    }

    /** 解锁通知事件（残卷 / 师承 / 顿悟）。 */
    public record RecipeUnlockedEvent(
        String recipeId,
        UnlockSource source,
        long unlockedAtTick
    ) {
        public RecipeUnlockedEvent {
            Objects.requireNonNull(recipeId, "recipeId");
            Objects.requireNonNull(source, "source");
        }

        public sealed interface UnlockSource permits Scroll, Mentor, Insight {}
        public record Scroll(String itemTemplate) implements UnlockSource {}
        public record Mentor(String npcArchetype) implements UnlockSource {}
        public record Insight(String trigger) implements UnlockSource {}
    }

    /** 测试辅助：grouped by category，保留 grouped_for_ui 分组顺序。 */
    public static Map<CraftCategory, List<CraftRecipe>> recipesGroupedByCategory() {
        Map<CraftCategory, List<CraftRecipe>> grouped = new LinkedHashMap<>();
        for (CraftRecipe r : recipes) {
            grouped.computeIfAbsent(r.category(), k -> new ArrayList<>()).add(r);
        }
        return Collections.unmodifiableMap(grouped);
    }
}
