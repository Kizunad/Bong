package com.bong.client.inventory.component;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.function.Consumer;

/**
 * plan-craft-v1 §2 — 通用手搓 inventory tab UI。
 *
 * <p>布局：左 RecipeListPanel（按 6 类分组 + ✅/🔒）+ 右 RecipeDetailPanel（材料 ✓/✗ +
 * 耗时 / 产出 / requirements）+ 底 CurrentTaskBar（进度条 + [开始]/[取消]）。</p>
 *
 * <p>事件流：</p>
 * <ol>
 *   <li>构造时订阅 CraftStore 三个 listener（recipes / session / outcome）</li>
 *   <li>用户点击 list 项 → setSelected(recipe) → 重绘 detailPanel</li>
 *   <li>用户点击 [开始手搓] → ClientRequestSender.sendCraftStart(recipe.id)</li>
 *   <li>用户点击 [取消任务] → ClientRequestSender.sendCraftCancel()</li>
 *   <li>server 推 CraftSessionState/CraftOutcome → CraftStore listener → CurrentTaskBar.refresh()</li>
 * </ol>
 */
public final class CraftTabPanel {
    private static final int COLOR_TEXT_PRIMARY = 0xFFE8DDC4;
    private static final int COLOR_TEXT_DIM = 0xFF888070;
    private static final int COLOR_LOCKED = 0xFF666666;
    private static final int COLOR_SUFFICIENT = 0xFF7AB45A;
    private static final int COLOR_INSUFFICIENT = 0xFFD05A4A;
    private static final int COLOR_PANEL_BG = 0xC0151715;
    private static final int COLOR_PANEL_BORDER = 0xFF4A5C46;
    private static final int COLOR_BAR_FULL = 0xFFC0A050;
    private static final int COLOR_BAR_EMPTY = 0xFF222222;
    private static final int LIST_WIDTH = 180;

    private final FlowLayout root;
    private final FlowLayout listColumn;
    private final FlowLayout detailColumn;
    private final FlowLayout currentTaskColumn;
    private final ButtonComponent startButton;
    private final ButtonComponent cancelButton;
    private final LabelComponent progressLabel;

    /** review fix (CodeRabbit/Codex P2): 仅存 id；每次 rebuild 都从 CraftStore.recipe(id) 拿当前快照，
     * 解锁后右侧详情会跟随刷新（旧实现持有 CraftRecipe 引用 → unlock 后右栏永远 🔒 + Start 灰）。 */
    private String selectedId;
    private final Consumer<List<CraftRecipe>> recipeListener = recipes -> rebuildAll();
    private final Consumer<CraftSessionStateView> sessionListener = state -> refreshTaskBar();
    private final Consumer<CraftStore.CraftOutcomeEvent> outcomeListener = event -> refreshTaskBar();
    /** review fix (Claude / plan §1 P2 acceptance "材料缺料红字")：inventory 变更时刷右栏材料颜色。 */
    private final Consumer<InventoryModel> inventoryListener = inv -> rebuildDetail();

    public CraftTabPanel() {
        root = Containers.verticalFlow(Sizing.fill(100), Sizing.fill(100));
        root.padding(Insets.of(2));
        root.gap(4);

        FlowLayout topRow = Containers.horizontalFlow(Sizing.fill(100), Sizing.fill(82));
        topRow.gap(4);
        listColumn = Containers.verticalFlow(Sizing.fixed(LIST_WIDTH), Sizing.fill(100));
        listColumn.surface(Surface.flat(COLOR_PANEL_BG).and(Surface.outline(COLOR_PANEL_BORDER)));
        listColumn.padding(Insets.of(3));
        listColumn.gap(2);
        detailColumn = Containers.verticalFlow(Sizing.fill(100), Sizing.fill(100));
        detailColumn.surface(Surface.flat(COLOR_PANEL_BG).and(Surface.outline(COLOR_PANEL_BORDER)));
        detailColumn.padding(Insets.of(4));
        detailColumn.gap(3);
        topRow.child(listColumn);
        topRow.child(detailColumn);
        root.child(topRow);

        currentTaskColumn = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        currentTaskColumn.surface(Surface.flat(COLOR_PANEL_BG).and(Surface.outline(COLOR_PANEL_BORDER)));
        currentTaskColumn.padding(Insets.of(4));
        currentTaskColumn.gap(2);
        progressLabel = label("", COLOR_TEXT_PRIMARY);
        currentTaskColumn.child(progressLabel);

        FlowLayout buttonsRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        buttonsRow.gap(4);
        startButton = Components.button(Text.literal("开始手搓"), btn -> onStartClicked());
        startButton.sizing(Sizing.fixed(80), Sizing.fixed(18));
        cancelButton = Components.button(Text.literal("取消任务"), btn -> onCancelClicked());
        cancelButton.sizing(Sizing.fixed(80), Sizing.fixed(18));
        buttonsRow.child(startButton);
        buttonsRow.child(cancelButton);
        currentTaskColumn.child(buttonsRow);
        root.child(currentTaskColumn);

        rebuildAll();
        attachListeners();
    }

    public FlowLayout root() { return root; }

    /** 刷新整个 tab（list + detail + task bar）。 */
    public void rebuildAll() {
        rebuildList();
        rebuildDetail();
        refreshTaskBar();
    }

    /** Screen 关闭时调用，避免 listener 累积内存泄漏。 */
    public void dispose() {
        CraftStore.removeRecipeListener(recipeListener);
        CraftStore.removeSessionListener(sessionListener);
        CraftStore.removeOutcomeListener(outcomeListener);
        InventoryStateStore.removeListener(inventoryListener);
    }

    private void attachListeners() {
        CraftStore.addRecipeListener(recipeListener);
        CraftStore.addSessionListener(sessionListener);
        CraftStore.addOutcomeListener(outcomeListener);
        InventoryStateStore.addListener(inventoryListener);
    }

    /** review fix: 始终从 CraftStore 拿当前快照而不是持引用，避免 unlock 后陷在旧对象。 */
    private CraftRecipe currentSelected() {
        if (selectedId == null) return null;
        return CraftStore.recipe(selectedId).orElse(null);
    }

    // ─── 左 list ────────────────────────────────────────────────

    private void rebuildList() {
        listColumn.clearChildren();
        listColumn.child(label("配方列表", COLOR_TEXT_PRIMARY));
        listColumn.child(label("──────────", COLOR_TEXT_DIM));

        Map<CraftCategory, List<CraftRecipe>> grouped = new LinkedHashMap<>();
        for (CraftRecipe r : CraftStore.recipes()) {
            grouped.computeIfAbsent(r.category(), k -> new ArrayList<>()).add(r);
        }
        if (grouped.isEmpty()) {
            listColumn.child(label("（暂无配方）", COLOR_TEXT_DIM));
            return;
        }
        for (Map.Entry<CraftCategory, List<CraftRecipe>> entry : grouped.entrySet()) {
            CraftCategory category = entry.getKey();
            listColumn.child(label("▼ " + category.displayName(), COLOR_TEXT_PRIMARY));
            for (CraftRecipe recipe : entry.getValue()) {
                listColumn.child(buildListRow(recipe));
            }
        }
    }

    private FlowLayout buildListRow(CraftRecipe recipe) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.padding(Insets.of(1, 1, 4, 4));
        row.verticalAlignment(VerticalAlignment.CENTER);
        boolean isSelected = recipe.id().equals(selectedId);
        if (isSelected) {
            row.surface(Surface.flat(0x40FFFFFF));
        }
        String marker = recipe.unlocked() ? "✅" : "🔒";
        int color = recipe.unlocked() ? COLOR_TEXT_PRIMARY : COLOR_LOCKED;
        LabelComponent labelText = label(marker + " " + recipe.displayName(), color);
        row.child(labelText);
        final String rowId = recipe.id();
        row.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                setSelected(rowId);
                return true;
            }
            return false;
        });
        return row;
    }

    private void setSelected(String recipeId) {
        if (Objects.equals(this.selectedId, recipeId)) return;
        this.selectedId = recipeId;
        rebuildList();
        rebuildDetail();
        refreshTaskBar();
    }

    // ─── 右 detail ──────────────────────────────────────────────

    private void rebuildDetail() {
        detailColumn.clearChildren();
        CraftRecipe recipe = currentSelected();
        if (recipe == null) {
            detailColumn.child(label("← 选一个配方查看详情", COLOR_TEXT_DIM));
            return;
        }
        detailColumn.child(label("选中：" + recipe.displayName(), COLOR_TEXT_PRIMARY));
        detailColumn.child(label("──────────────────", COLOR_TEXT_DIM));
        detailColumn.child(label("类别：" + recipe.category().displayName(), COLOR_TEXT_DIM));
        if (!recipe.unlocked()) {
            detailColumn.child(label("🔒 未解锁（残卷 / 师承 / 顿悟）", COLOR_LOCKED));
        }

        // review fix (Claude / plan §1 P2 acceptance "材料检查实时高亮（缺料红字）"):
        // 用 InventoryStateStore 当前快照对每条材料 have/need 比对：足绿、缺红。
        // qi 用 InventoryModel.qiCurrent() vs recipe.qiCost() 对比。
        InventoryModel inv = InventoryStateStore.snapshot();
        detailColumn.child(label("材料：", COLOR_TEXT_DIM));
        for (CraftRecipe.MaterialEntry mat : recipe.materials()) {
            int have = countTemplateInInventory(inv, mat.templateId());
            boolean ok = have >= mat.count();
            int color = ok ? COLOR_SUFFICIENT : COLOR_INSUFFICIENT;
            String mark = ok ? "✓" : "✗";
            detailColumn.child(label(
                String.format("  %s %s ×%d  [已有 %d]", mark, mat.templateId(), mat.count(), have),
                color));
        }
        if (recipe.qiCost() > 0) {
            double qiCur = inv.qiCurrent();
            boolean qiOk = qiCur >= recipe.qiCost();
            int color = qiOk ? COLOR_SUFFICIENT : COLOR_INSUFFICIENT;
            String mark = qiOk ? "✓" : "✗";
            detailColumn.child(label(
                String.format("  %s 自身真元 ×%.0f  [当前 %.0f]", mark, recipe.qiCost(), qiCur),
                color));
        }

        // requirements
        for (String reqLine : recipe.requirements().humanLines()) {
            detailColumn.child(label("门槛：" + reqLine, COLOR_TEXT_DIM));
        }

        long timeSec = (recipe.timeTicks() + 19L) / 20L;
        detailColumn.child(label(
            String.format("耗时：%d 秒（in-game）", timeSec),
            COLOR_TEXT_DIM));
        detailColumn.child(label(
            "产出：" + recipe.outputTemplate() + " ×" + recipe.outputCount(),
            COLOR_TEXT_PRIMARY));
    }

    /** 在 inventory 所有 grid + equipped + hotbar 里聚合 templateId 的 stack count。 */
    private static int countTemplateInInventory(InventoryModel inv, String templateId) {
        if (inv == null || templateId == null || templateId.isEmpty()) return 0;
        long total = 0;
        for (InventoryModel.GridEntry entry : inv.gridItems()) {
            InventoryItem item = entry.item();
            if (item != null && templateId.equals(item.itemId())) {
                total += item.stackCount();
            }
        }
        for (InventoryItem item : inv.equipped().values()) {
            if (item != null && templateId.equals(item.itemId())) {
                total += item.stackCount();
            }
        }
        for (InventoryItem item : inv.hotbar()) {
            if (item != null && templateId.equals(item.itemId())) {
                total += item.stackCount();
            }
        }
        // clamp 到 int 上限以兼容材料 count 字段
        return total > Integer.MAX_VALUE ? Integer.MAX_VALUE : (int) total;
    }

    // ─── 底 current task bar ──────────────────────────────────────

    private void refreshTaskBar() {
        CraftSessionStateView state = CraftStore.sessionState();
        if (state.active()) {
            String recipeId = state.recipeId().orElse("");
            CraftRecipe activeRecipe = CraftStore.recipe(recipeId).orElse(null);
            String name = activeRecipe != null ? activeRecipe.displayName() : recipeId;
            int pct = (int) Math.round(state.progress() * 100);
            String bar = renderBar(state.progress(), 12);
            progressLabel.text(Text.literal(String.format(
                "进行中：%s  [%s]  %d%%  剩 %ds",
                name, bar, pct, state.remainingSeconds())));
            startButton.active(false);
            cancelButton.active(true);
            startButton.tooltip(Text.literal("已有任务在跑，先取消才能起新任务"));
        } else {
            CraftStore.lastOutcome().ifPresentOrElse(outcome -> {
                String text = switch (outcome.kind()) {
                    case COMPLETED -> "✓ 出炉：" + outcome.outputTemplate() + " ×" + outcome.outputCount();
                    case FAILED -> "✗ " + (outcome.failureReason().equals("player_cancelled")
                        ? buildCancelText(outcome)
                        : "失败：" + outcome.failureReason());
                };
                progressLabel.text(Text.literal(text));
            }, () -> progressLabel.text(Text.literal("当前任务：（无）")));
            CraftRecipe sel = currentSelected();
            startButton.active(sel != null && sel.unlocked());
            cancelButton.active(false);
            startButton.tooltip(sel == null
                ? Text.literal("先在左列选一个配方")
                : (sel.unlocked()
                    ? Text.literal("起手搓 " + sel.displayName())
                    : Text.literal("配方未解锁")));
        }
    }

    /** review fix (CodeRabbit): 根据 outcome.qiRefunded() 而不是硬编码"真元不退"。 */
    private static String buildCancelText(CraftStore.CraftOutcomeEvent outcome) {
        if (outcome.qiRefunded() > 0) {
            return String.format(
                "取消（返还材料 ×%d，退还真元 %.0f）",
                outcome.materialReturned(), outcome.qiRefunded());
        }
        return "取消（返还材料 ×" + outcome.materialReturned() + "，真元不退）";
    }

    private void onStartClicked() {
        CraftRecipe sel = currentSelected();
        if (sel == null || !sel.unlocked()) return;
        CraftSessionStateView state = CraftStore.sessionState();
        if (state.active()) return;
        ClientRequestSender.sendCraftStart(sel.id());
    }

    private void onCancelClicked() {
        CraftSessionStateView state = CraftStore.sessionState();
        if (!state.active()) return;
        ClientRequestSender.sendCraftCancel();
    }

    // ─── 工具方法 ────────────────────────────────────────────────

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        return label;
    }

    /** 进度条文本渲染（▓ filled / ░ empty）。 */
    private static String renderBar(float progress, int width) {
        if (width <= 0) return "";
        int filled = Math.max(0, Math.min(width, Math.round(progress * width)));
        StringBuilder sb = new StringBuilder(width);
        for (int i = 0; i < width; i++) {
            sb.append(i < filled ? '▓' : '░');
        }
        return sb.toString();
    }

    @SuppressWarnings("unused")
    private static void touchUnusedColors() {
        // review fix: 保留 BAR 颜色常量供未来 owo-lib 进度条 component 升级使用；
        // SUFFICIENT/INSUFFICIENT 已在 rebuildDetail 接入材料高亮，无需在此手动 reference。
        int _f = COLOR_BAR_FULL;
        int _e = COLOR_BAR_EMPTY;
    }
}
