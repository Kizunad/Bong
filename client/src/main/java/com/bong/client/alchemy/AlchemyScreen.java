package com.bong.client.alchemy;

import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.DragState;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.MinecraftClient;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.*;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;

import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.intent.UiIntentResult;

import java.util.function.Consumer;

/** plan-alchemy-v1 §3.3 — 紧凑炼丹炉界面（600×340，UI scale 3 下完整可见）。 */
public final class AlchemyScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("炼丹炉");
    private static final int FURNACE_SLOTS = 4;
    private static final int ICON_SIZE = 128;
    private static final String RECIPE_SCROLL_PREFIX = "recipe_scroll_";

    // Panel target: fits 1080p · UI scale 3 (640×360) and 2K · UI scale 3 (853×480)
    private static final int PANEL_W = 600;
    private static final int PANEL_H = 340;
    private static final int LEFT_W = 150;
    private static final int MID_W = 220;
    private static final int RIGHT_W = 212; // backpack 5×7 占 196 + padding
    private static final int BODY_H = 244;
    private static final int BOTTOM_H = 60;

    private LabelComponent recipeTitle;
    private LabelComponent recipeSubtitle;
    private LabelComponent recipeBody;
    private LabelComponent alchemySkillLabel;
    private LabelComponent furnaceStatusLabel;
    private LabelComponent furnaceInfoLabel;
    private LabelComponent progressLabel;
    private LabelComponent tempValueLabel;
    private LabelComponent qiValueLabel;
    private FlowLayout progressFill;
    private FlowLayout tempKnob;
    private FlowLayout qiFill;
    private FlowLayout interventionsBox;
    private FlowLayout scrollDropZone;
    private FlowLayout outcomeRow;
    private FlowLayout contaminationBox;
    private FlowLayout historyBox;
    private LabelComponent weightLabel;
    private FlowLayout weightFill;

    private final GridSlotComponent[] furnaceSlots = new GridSlotComponent[FURNACE_SLOTS];
    private final InventoryItem[] furnaceItems = new InventoryItem[FURNACE_SLOTS];

    private BackpackGridPanel backpack;
    private final DragState dragState = new DragState();
    private final BlockPos furnacePos;
    private AlchemySessionPresentationPlanner.Presentation sessionPresentation;
    private Consumer<AlchemySessionPresentationPlanner.Presentation> sessionPresentationObserver;
    private final AlchemyScreenController controller;
    private final DefaultUiScreenScope scope = new DefaultUiScreenScope();
    private boolean removed;

    private int dupFlashTicks = 0;

    public AlchemyScreen() {
        this(new BlockPos(0, 64, 0));
    }

    public AlchemyScreen(BlockPos furnacePos) {
        super(TITLE);
        this.furnacePos = furnacePos;
        this.controller = AlchemyScreenController.production(this::applyViewModel, AlchemyScreen::executeOnClientThread);
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    public void removed() {
        removed = true;
        if (scope != null && !scope.isClosed()) {
            scope.close();
        }
        controller.onClose();
        super.removed();
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.fixed(PANEL_W), Sizing.fixed(PANEL_H));
        panel.surface(Surface.flat(0xFF0D0D15).and(Surface.outline(0xFF4A4050)));
        panel.padding(Insets.of(6));
        panel.gap(4);

        panel.child(buildHeader());

        FlowLayout columns = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(BODY_H));
        columns.gap(4);
        columns.child(buildScrollColumn());
        columns.child(buildFurnaceColumn());
        columns.child(buildBackpackColumn());
        panel.child(columns);

        panel.child(buildBottomStrip());

        root.child(panel);
        backpack.populateFromModel(controller.viewModel().inventory());
        refreshView(controller.viewModel());
        removed = false;
        if (!scope.isOpen()) {
            scope.onOpen();
            controller.onOpen(scope);
        }
    }

    private static void executeOnClientThread(Runnable action) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null) {
            client.execute(action);
        } else {
            action.run();
        }
    }

    private void refreshBackpack(com.bong.client.inventory.model.InventoryModel model) {
        if (backpack == null) return;
        backpack.populateFromModel(model);
    }

    private void applyViewModel(AlchemyScreenViewModel model) {
        if (removed) return;
        refreshView(model);
    }

    private void refreshView(AlchemyScreenViewModel model) {
        refreshBackpack(model.inventory());
        refreshAlchemySkillText(model);
        refreshRecipeText(model);
        refreshFurnaceText(model);
        refreshSessionText(model);
        refreshWeight(model);
        refreshOutcomes(model);
        refreshHistory(model);
        refreshContamination(model);
    }

    private FlowLayout buildHeader() {
        FlowLayout h = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        h.verticalAlignment(VerticalAlignment.CENTER);
        h.child(Components.label(Text.literal("§f§l炼丹炉")));
        h.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
        alchemySkillLabel = Components.label(Text.literal("炼丹 Lv.0 · 本次火候容差 +0%"));
        alchemySkillLabel.color(Color.ofArgb(0xFFE0B060));
        h.child(alchemySkillLabel);
        h.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
        h.child(Components.label(Text.literal("§7I 起炉 · F 注真元 · T 结算")));
        return h;
    }

    // ================== LEFT · 配方手札 ==================
    private FlowLayout buildScrollColumn() {
        FlowLayout col = Containers.verticalFlow(Sizing.fixed(LEFT_W), Sizing.fill(100));
        col.surface(Surface.flat(0xFF2A2318).and(Surface.outline(0xFF6A5030)));
        col.padding(Insets.of(4));
        col.gap(2);
        col.horizontalAlignment(HorizontalAlignment.CENTER);

        FlowLayout titleRow = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        titleRow.gap(4);
        titleRow.verticalAlignment(VerticalAlignment.CENTER);
        titleRow.horizontalAlignment(HorizontalAlignment.CENTER);
        titleRow.child(makeButton("§6\u25C0", () -> turnPage(-1)));
        recipeTitle = Components.label(Text.literal(""));
        recipeTitle.color(Color.ofArgb(0xFFE8D8A8));
        titleRow.child(recipeTitle);
        titleRow.child(makeButton("§6\u25B6", () -> turnPage(1)));
        col.child(titleRow);

        recipeSubtitle = Components.label(Text.literal(""));
        recipeSubtitle.color(Color.ofArgb(0xFFA89878));
        col.child(recipeSubtitle);

        FlowLayout dash = Containers.horizontalFlow(Sizing.fill(95), Sizing.fixed(1));
        dash.surface(Surface.flat(0xFF8A6A40));
        col.child(dash);

        FlowLayout bodyWrap = Containers.verticalFlow(Sizing.fill(100), Sizing.fixed(140));
        bodyWrap.padding(Insets.of(2));
        recipeBody = Components.label(Text.literal(""));
        recipeBody.color(Color.ofArgb(0xFFE8D8A8));
        recipeBody.horizontalSizing(Sizing.fill(100));
        recipeBody.maxWidth(LEFT_W - 12);
        bodyWrap.child(recipeBody);
        col.child(bodyWrap);

        scrollDropZone = Containers.verticalFlow(Sizing.fill(100), Sizing.fixed(36));
        scrollDropZone.surface(Surface.flat(0xFF2A2018).and(Surface.outline(0xFF8A6A40)));
        scrollDropZone.horizontalAlignment(HorizontalAlignment.CENTER);
        scrollDropZone.verticalAlignment(VerticalAlignment.CENTER);
        scrollDropZone.gap(1);
        scrollDropZone.child(Components.label(Text.literal("§6§l学习新方")));
        scrollDropZone.child(Components.label(Text.literal("§7拖入【丹方残卷】")));
        col.child(scrollDropZone);

        // plan-alchemy-v1 §1.3 试药史(LifeRecord.alchemy_attempts)— 显示最近 3 条
        historyBox = Containers.verticalFlow(Sizing.fill(100), Sizing.fixed(38));
        historyBox.surface(Surface.flat(0xFF1A1410).and(Surface.outline(0xFF6A5030)));
        historyBox.padding(Insets.of(2));
        historyBox.gap(1);
        col.child(historyBox);

        refreshRecipeText();
        refreshHistory();
        return col;
    }

    // ================== CENTER · 炉体 ==================
    private FlowLayout buildFurnaceColumn() {
        FlowLayout col = Containers.verticalFlow(Sizing.fixed(MID_W), Sizing.fill(100));
        col.surface(Surface.flat(0xFF14141F).and(Surface.outline(0xFFC04040)));
        col.padding(Insets.of(4));
        col.gap(3);

        col.child(Components.label(Text.literal("§c§l炉体")));

        col.child(buildSlotsRow());

        furnaceStatusLabel = Components.label(Text.literal(""));
        furnaceStatusLabel.color(Color.ofArgb(0xFFFFCC40));
        col.child(furnaceStatusLabel);

        col.child(buildProgressBar());
        col.child(buildTempRow());
        col.child(buildQiRow());

        col.child(buildInterventionsAndInfo());

        refreshFurnaceText();
        refreshSessionText();
        return col;
    }

    private FlowLayout buildSlotsRow() {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.gap(0);
        row.verticalAlignment(VerticalAlignment.CENTER);
        for (int i = 0; i < FURNACE_SLOTS; i++) {
            GridSlotComponent slot = new GridSlotComponent(0, i);
            furnaceSlots[i] = slot;
            row.child(slot);
        }
        return row;
    }

    private FlowLayout buildProgressBar() {
        FlowLayout wrap = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        wrap.gap(1);
        progressLabel = Components.label(Text.literal(""));
        progressLabel.color(Color.ofArgb(0xFFCCCCCC));
        wrap.child(progressLabel);

        FlowLayout track = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(8));
        track.surface(Surface.flat(0xFF001020).and(Surface.outline(0xFF666666)));
        progressFill = Containers.horizontalFlow(Sizing.fill(60), Sizing.fill(100));
        progressFill.surface(Surface.flat(0xFFC04040));
        track.child(progressFill);
        wrap.child(track);
        return wrap;
    }

    private FlowLayout buildTempRow() {
        FlowLayout wrap = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        wrap.gap(1);
        FlowLayout hdr = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        hdr.child(Components.label(Text.literal("§f温 \u2191\u2193")));
        hdr.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
        tempValueLabel = Components.label(Text.literal(""));
        tempValueLabel.color(Color.ofArgb(0xFFFFCC40));
        hdr.child(tempValueLabel);
        wrap.child(hdr);

        FlowLayout track = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(6));
        track.surface(Surface.flat(0xFF333333));
        FlowLayout pre = Containers.horizontalFlow(Sizing.fill(48), Sizing.fill(100));
        track.child(pre);
        FlowLayout band = Containers.horizontalFlow(Sizing.fill(12), Sizing.fill(100));
        band.surface(Surface.flat(0x4D40FF80));
        track.child(band);
        tempKnob = Containers.horizontalFlow(Sizing.fixed(3), Sizing.fill(100));
        tempKnob.surface(Surface.flat(0xFFFFCC40));
        track.child(tempKnob);
        wrap.child(track);
        return wrap;
    }

    private FlowLayout buildQiRow() {
        FlowLayout wrap = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        wrap.gap(1);
        FlowLayout hdr = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        hdr.child(Components.label(Text.literal("§b§lF 注真元")));
        hdr.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
        qiValueLabel = Components.label(Text.literal(""));
        qiValueLabel.color(Color.ofArgb(0xFFAAAAAA));
        hdr.child(qiValueLabel);
        wrap.child(hdr);

        FlowLayout qiTrack = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(6));
        qiTrack.surface(Surface.flat(0xFF001020).and(Surface.outline(0xFF666666)));
        qiFill = Containers.horizontalFlow(Sizing.fill(61), Sizing.fill(100));
        qiFill.surface(Surface.flat(0xFF40D0D0));
        qiTrack.child(qiFill);
        wrap.child(qiTrack);
        return wrap;
    }

    private FlowLayout buildInterventionsAndInfo() {
        FlowLayout col = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        col.gap(1);

        interventionsBox = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        interventionsBox.gap(1);
        interventionsBox.child(Components.label(Text.literal("§7干预")));
        col.child(interventionsBox);

        furnaceInfoLabel = Components.label(Text.literal(""));
        col.child(furnaceInfoLabel);
        return col;
    }

    // ================== RIGHT · 玩家背包 ==================
    private FlowLayout buildBackpackColumn() {
        FlowLayout col = Containers.verticalFlow(Sizing.fixed(RIGHT_W), Sizing.fill(100));
        col.surface(Surface.flat(0xFF14141F).and(Surface.outline(0xFF808080)));
        col.padding(Insets.of(4));
        col.gap(3);

        col.child(Components.label(Text.literal("§f§l背包")));

        // F13：容器 id 绑定真实 main_pack（5×7，与此处固定 5×7 面板尺寸一致），
        // 不再用与任何真实数据都不匹配的占位 id（"alchemy_mock"）。
        backpack = new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7);
        col.child(backpack.container());

        weightLabel = Components.label(Text.literal(""));
        weightLabel.color(Color.ofArgb(0xFF888888));
        col.child(weightLabel);

        FlowLayout wtrack = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(4));
        wtrack.surface(Surface.flat(0xFF001020).and(Surface.outline(0xFF444444)));
        weightFill = Containers.horizontalFlow(Sizing.fill(25), Sizing.fill(100));
        weightFill.surface(Surface.flat(0xFF60A060));
        wtrack.child(weightFill);
        col.child(wtrack);

        refreshWeight();
        return col;
    }

    // ================== BOTTOM STRIP ==================
    private FlowLayout buildBottomStrip() {
        FlowLayout strip = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(BOTTOM_H));
        strip.surface(Surface.flat(0xFF14141F).and(Surface.outline(0xFFFFCC40)));
        strip.padding(Insets.of(4));
        strip.gap(4);
        strip.verticalAlignment(VerticalAlignment.CENTER);

        outcomeRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        outcomeRow.gap(3);
        outcomeRow.verticalAlignment(VerticalAlignment.CENTER);
        strip.child(outcomeRow);

        contaminationBox = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        contaminationBox.padding(Insets.of(2, 2, 4, 4));
        contaminationBox.surface(Surface.flat(0xFF1A0A1A).and(Surface.outline(0xFFC040FF)));
        contaminationBox.gap(1);
        strip.child(contaminationBox);

        refreshOutcomes();
        refreshContamination();
        return strip;
    }

    private FlowLayout makeButton(String text, Runnable onClick) {
        var lbl = Components.label(Text.literal(text));
        lbl.color(Color.ofArgb(0xFFCCCCCC));
        lbl.cursorStyle(CursorStyle.HAND);
        FlowLayout wrap = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        wrap.padding(Insets.of(1, 1, 3, 3));
        wrap.surface(Surface.flat(0xFF2A2A2A).and(Surface.outline(0xFF555555)));
        wrap.cursorStyle(CursorStyle.HAND);
        wrap.child(lbl);
        wrap.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { onClick.run(); return true; }
            return false;
        });
        return wrap;
    }

    private void turnPage(int delta) {
        // C→S：server 维护 current_index；adapter 保留未连接时的本地 fallback。
        controller.intentSink().dispatch(new AlchemyIntent.TurnPage(delta));
        controller.refreshFromSource();
    }

    // ================== REFRESH from Stores ==================
    private void refreshRecipeText() {
        refreshRecipeText(controller.viewModel());
    }

    private void refreshRecipeText(AlchemyScreenViewModel model) {
        if (recipeTitle == null) return;
        var snap = model.recipes();
        var cur = snap.current();
        if (cur == null) {
            recipeTitle.text(Text.literal("§8（未悟方子）"));
            recipeSubtitle.text(Text.literal(""));
            recipeBody.text(Text.literal("§8拖入丹方残卷以悟方。"));
            return;
        }
        String title = "§6§l" + cur.displayName();
        if (dupFlashTicks > 0) title += " §c已悟";
        recipeTitle.text(Text.literal(title));
        String sub = "§o" + cur.author() + " · " + snap.learned().size() + "/" + cur.maxKnown();
        recipeSubtitle.text(Text.literal(sub));
        recipeBody.text(Text.literal(cur.bodyText()));
    }

    private void refreshAlchemySkillText() {
        refreshAlchemySkillText(controller.viewModel());
    }

    private void refreshAlchemySkillText(AlchemyScreenViewModel model) {
        if (alchemySkillLabel == null) return;
        SkillSetSnapshot skills = model.skills();
        alchemySkillLabel.text(Text.literal(formatAlchemySkillHeader(
            skills.get(SkillId.ALCHEMY)
        )));
    }

    static String formatAlchemySkillHeader(SkillSetSnapshot.Entry entry) {
        if (entry == null) entry = SkillSetSnapshot.Entry.zero();
        int effectiveLv = entry.effectiveLv();
        int bonusPercent = (int) Math.round((alchemyToleranceScale(effectiveLv) - 1.0) * 100.0);
        if (entry.lv() > entry.cap()) {
            return "炼丹 Lv." + entry.lv() + " (压制→" + effectiveLv + ") · 本次火候容差 +" + bonusPercent + "%";
        }
        return "炼丹 Lv." + effectiveLv + " · 本次火候容差 +" + bonusPercent + "%";
    }

    private static double alchemyToleranceScale(int effectiveLv) {
        if (effectiveLv <= 0) return 1.0;
        if (effectiveLv <= 1) return 1.05;
        if (effectiveLv <= 3) return 1.05 + (effectiveLv - 1) * 0.05;
        if (effectiveLv <= 5) return 1.15 + (effectiveLv - 3) * 0.05;
        if (effectiveLv <= 7) return 1.25 + (effectiveLv - 5) * 0.05;
        if (effectiveLv <= 10) return 1.35 + (effectiveLv - 7) * 0.05;
        return 1.50;
    }

    private void refreshFurnaceText() {
        refreshFurnaceText(controller.viewModel());
    }

    private void refreshFurnaceText(AlchemyScreenViewModel model) {
        if (furnaceInfoLabel == null) return;
        var f = model.furnace();
        BlockPos shownPos = f.pos() != null ? f.pos() : furnacePos;
        furnaceInfoLabel.text(Text.literal(String.format(
            "§7%s · t%d 完整 %.0f/%.0f · %s",
            posLabel(shownPos), f.tier(), f.integrity(), f.integrityMax(), f.ownerName())));
    }

    private static String posLabel(BlockPos pos) {
        return pos.getX() + "," + pos.getY() + "," + pos.getZ();
    }

    private void refreshSessionText() {
        refreshSessionText(controller.viewModel());
    }

    private void refreshSessionText(AlchemyScreenViewModel model) {
        var furnace = model.furnace();
        var session = model.session();
        sessionPresentation = AlchemySessionPresentationPlanner.describe(
            furnace,
            session
        );
        if (sessionPresentationObserver != null) {
            sessionPresentationObserver.accept(sessionPresentation);
        }
        if (removed || furnaceStatusLabel == null) return;

        refreshStageFlash(sessionPresentation);
        furnaceStatusLabel.text(Text.literal(sessionPresentation.statusText()));
        progressLabel.text(Text.literal(sessionPresentation.progressText()));
        tempValueLabel.text(Text.literal(sessionPresentation.temperatureText()));
        qiValueLabel.text(Text.literal(sessionPresentation.qiText()));

        interventionsBox.<FlowLayout>configure(layout -> {
            layout.clearChildren();
            for (String line : sessionPresentation.detailLines()) {
                layout.child(Components.label(Text.literal(line)));
            }
        });
    }

    /**
     * plan-alchemy-v1 §1.3 中途投料 — 当 elapsed_ticks ∈ [at_tick, at_tick+window] 内
     * 且 stage 未完成时,把对应 slot 高亮成 VALID(脉冲提示玩家"该投这一槽了")。
     */
    private void refreshStageFlash(AlchemySessionPresentationPlanner.Presentation presentation) {
        if (furnaceSlots[0] == null) return;
        // 默认清掉 stage 闪烁(保留 drag 高亮 — drag 高亮是 update 时设的)
        for (int i = 0; i < FURNACE_SLOTS; i++) {
            if (furnaceSlots[i] != null && furnaceItems[i] == null) {
                furnaceSlots[i].setHighlightState(GridSlotComponent.HighlightState.NONE);
            }
        }
        for (int slot : presentation.flashingStageSlots()) {
            if (slot >= 0 && slot < FURNACE_SLOTS && furnaceItems[slot] == null) {
                furnaceSlots[slot].setHighlightState(GridSlotComponent.HighlightState.VALID);
            }
        }
    }

    private void refreshWeight() {
        refreshWeight(controller.viewModel());
    }

    private void refreshWeight(AlchemyScreenViewModel model) {
        if (weightLabel == null) return;
        var m = model.inventoryMeta();
        weightLabel.text(Text.literal(String.format(
            "§7重量 %.1f/%.1f", m.weightCurrent(), m.weightMax())));
        int pct = m.weightMax() > 0 ? Math.round(100f * m.weightCurrent() / m.weightMax()) : 0;
        pct = Math.max(1, Math.min(100, pct));
        weightFill.horizontalSizing(Sizing.fill(pct));
    }

    private void refreshOutcomes() {
        refreshOutcomes(controller.viewModel());
    }

    private void refreshOutcomes(AlchemyScreenViewModel model) {
        if (outcomeRow == null) return;
        var o = model.outcome();
        outcomeRow.<FlowLayout>configure(layout -> {
            layout.clearChildren();
            layout.child(outcomeCard("perf", o.perfectPct(), 0xFF0A2A0A, 0xFF40FF80));
            layout.child(outcomeCard("good", o.goodPct(), 0xFF1A2A18, 0xFFA0FF60));
            layout.child(outcomeCard("flaw", o.flawedPct(), 0xFF2A220A, 0xFFFFCC40));
            layout.child(outcomeCard("wast", o.wastePct(), 0xFF1A1A1A, 0xFF888888));
            layout.child(outcomeCard("boom", o.explodePct(), 0xFF2A0A0A, 0xFFFF4040));
        });
    }

    private FlowLayout outcomeCard(String name, float pct, int bg, int border) {
        FlowLayout c = Containers.verticalFlow(Sizing.fixed(36), Sizing.fixed(36));
        c.surface(Surface.flat(bg).and(Surface.outline(border)));
        c.horizontalAlignment(HorizontalAlignment.CENTER);
        c.verticalAlignment(VerticalAlignment.CENTER);
        c.padding(Insets.of(1));
        c.gap(0);
        LabelComponent n = Components.label(Text.literal("§l" + name));
        n.color(Color.ofArgb(border));
        c.child(n);
        LabelComponent v = Components.label(Text.literal(String.format("%.0f%%", pct)));
        v.color(Color.ofArgb(border));
        c.child(v);
        return c;
    }

    private void refreshHistory() {
        refreshHistory(controller.viewModel());
    }

    private void refreshHistory(AlchemyScreenViewModel model) {
        if (historyBox == null) return;
        var entries = model.history();
        historyBox.<FlowLayout>configure(layout -> {
            layout.clearChildren();
            layout.child(Components.label(Text.literal("§7§l试药史")));
            if (entries.isEmpty()) {
                layout.child(Components.label(Text.literal("§8（无记录）")));
                return;
            }
            int from = Math.max(0, entries.size() - 3);
            for (int i = entries.size() - 1; i >= from; i--) {
                var e = entries.get(i);
                String color = switch (e.bucket()) {
                    case "perfect" -> "§a";
                    case "good" -> "§2";
                    case "flawed" -> "§e";
                    case "waste" -> "§7";
                    case "explode" -> "§c";
                    default -> "§f";
                };
                String tag = e.flawedPath() ? " §o残" : "";
                String label = color + e.bucket() + tag + " §7" + e.recipeId();
                if (label.length() > 28) label = label.substring(0, 28);
                layout.child(Components.label(Text.literal(label)));
            }
        });
    }

    private void refreshContamination() {
        refreshContamination(controller.viewModel());
    }

    private void refreshContamination(AlchemyScreenViewModel model) {
        if (contaminationBox == null) return;
        var c = model.contamination();
        contaminationBox.<FlowLayout>configure(layout -> {
            layout.clearChildren();
            layout.child(Components.label(Text.literal("§d§l丹毒")));
            layout.child(contaminationRow("Mel", c.mellowCurrent(), c.mellowMax(), c.mellowOk(), 0xFFC09040));
            layout.child(contaminationRow("Vio", c.violentCurrent(), c.violentMax(), c.violentOk(), 0xFFE05050));
        });
    }

    private FlowLayout contaminationRow(String name, float cur, float max, boolean ok, int fillColor) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.gap(2);
        row.verticalAlignment(VerticalAlignment.CENTER);
        row.child(Components.label(Text.literal("§f" + name)));
        FlowLayout track = Containers.horizontalFlow(Sizing.fixed(80), Sizing.fixed(5));
        track.surface(Surface.flat(0xFF001020).and(Surface.outline(0xFF666666)));
        int pct = max > 0 ? Math.round(100f * cur / max) : 0;
        pct = Math.max(1, Math.min(100, pct));
        FlowLayout fill = Containers.horizontalFlow(Sizing.fill(pct), Sizing.fill(100));
        fill.surface(Surface.flat(fillColor));
        track.child(fill);
        row.child(track);
        row.child(Components.label(Text.literal(String.format("§7%.1f/%.1f", cur, max))));
        row.child(Components.label(Text.literal(ok ? "§a\u2713" : "§c\u2717")));
        return row;
    }

    // ============= 拖拽 =============

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (button != 0) return super.mouseClicked(mouseX, mouseY, button);

        if (backpack.containsPoint(mouseX, mouseY)) {
            var pos = backpack.screenToGrid(mouseX, mouseY);
            if (pos != null) {
                InventoryItem item = backpack.itemAt(pos.row(), pos.col());
                if (item != null) {
                    var anchor = backpack.anchorOf(item);
                    if (anchor != null) {
                        backpack.remove(item);
                        dragState.pickup(item, anchor.row(), anchor.col());
                        return true;
                    }
                }
            }
        }

        int fIdx = furnaceSlotAt(mouseX, mouseY);
        if (fIdx >= 0 && furnaceItems[fIdx] != null) {
            InventoryItem item = furnaceItems[fIdx];
            furnaceItems[fIdx] = null;
            furnaceSlots[fIdx].clearItem();
            dragState.pickup(item, -1, -fIdx - 1);
            return true;
        }

        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseDragged(double mouseX, double mouseY, int button, double dX, double dY) {
        if (dragState.isDragging()) {
            dragState.updateMouse(mouseX, mouseY);
            updateHighlights(mouseX, mouseY);
            return true;
        }
        return super.mouseDragged(mouseX, mouseY, button, dX, dY);
    }

    @Override
    public boolean mouseReleased(double mouseX, double mouseY, int button) {
        if (button == 0 && dragState.isDragging()) {
            attemptDrop(mouseX, mouseY);
            return true;
        }
        return super.mouseReleased(mouseX, mouseY, button);
    }

    private boolean pointInDropZone(double mx, double my) {
        if (scrollDropZone == null) return false;
        int x = scrollDropZone.x(), y = scrollDropZone.y();
        int w = scrollDropZone.width(), h = scrollDropZone.height();
        return mx >= x && mx < x + w && my >= y && my < y + h;
    }

    private void attemptDrop(double mx, double my) {
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) { dragState.cancel(); clearHighlights(); return; }

        if (pointInDropZone(mx, my) && dragged.itemId().startsWith(RECIPE_SCROLL_PREFIX)) {
            String id = dragged.itemId().substring(RECIPE_SCROLL_PREFIX.length());
            UiIntentResult result = controller.intentSink().dispatch(new AlchemyIntent.LearnRecipe(id));
            if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
                controller.refreshFromSource();
                dragState.drop();
                clearHighlights();
                refreshRecipeText();
                return;
            } else {
                var c = dragState.cancel();
                if (c != null && c.item() != null) {
                    int r = c.sourceRow(), co = c.sourceCol();
                    if (r >= 0 && co >= 0 && backpack.canPlace(c.item(), r, co)) {
                        backpack.place(c.item(), r, co);
                    } else {
                        var free = backpack.findFreeSpace(c.item());
                        if (free != null) backpack.place(c.item(), free.row(), free.col());
                    }
                }
                dupFlashTicks = 20;
                refreshRecipeText();
                clearHighlights();
                return;
            }
        }

        int fIdx = furnaceSlotAt(mx, my);
        if (fIdx >= 0 && dragged.gridWidth() == 1 && dragged.gridHeight() == 1) {
            if (furnaceItems[fIdx] == null) {
                furnaceItems[fIdx] = dragged;
                furnaceSlots[fIdx].setItem(dragged, true);
                int count = feedCountForSlot(activeRecipeId(), fIdx, dragged);
                try {
                    controller.intentSink().dispatch(new AlchemyIntent.FeedSlot(
                        furnacePos, fIdx, dragged.itemId(), count
                    ));
                } catch (RuntimeException ignore) { }
                dragState.drop();
                clearHighlights();
                return;
            }
        }

        if (backpack.containsPoint(mx, my)) {
            var pos = backpack.screenToGrid(mx, my);
            if (pos != null && backpack.canPlace(dragged, pos.row(), pos.col())) {
                backpack.place(dragged, pos.row(), pos.col());
                dragState.drop();
                clearHighlights();
                return;
            }
        }

        dragState.cancel();
        var free = backpack.findFreeSpace(dragged);
        if (free != null) backpack.place(dragged, free.row(), free.col());
        clearHighlights();
    }

    private int furnaceSlotAt(double sx, double sy) {
        int cs = GridSlotComponent.CELL_SIZE;
        for (int i = 0; i < FURNACE_SLOTS; i++) {
            GridSlotComponent s = furnaceSlots[i];
            if (s != null && sx >= s.x() && sx < s.x() + cs && sy >= s.y() && sy < s.y() + cs) return i;
        }
        return -1;
    }

    private String activeRecipeId() {
        var model = controller.viewModel();
        if (model.furnace().hasSession() && model.session().isActive()) return model.session().recipeId();
        var current = model.recipes().current();
        return current == null ? "" : current.id();
    }

    static int feedCountForSlot(String recipeId, int slotIdx, InventoryItem item) {
        if (item == null) return 1;
        int required = requiredMaterialCount(recipeId, slotIdx, item.itemId());
        int target = required > 0 ? required : 1;
        return Math.max(1, Math.min(item.stackCount(), target));
    }

    static String canonicalRecipeId(String recipeId) {
        if (recipeId == null) return "";
        return switch (recipeId) {
            case "kaimai_pill", "kai_mai_pill" -> "kai_mai_pill_v0";
            case "huiyuan_pill", "hui_yuan_pill" -> "hui_yuan_pill_v0";
            case "duming_san", "du_ming_san" -> "du_ming_san_v0";
            default -> recipeId;
        };
    }

    private static int requiredMaterialCount(String recipeId, int slotIdx, String material) {
        String normalized = canonicalRecipeId(recipeId);
        return switch (normalized) {
            case "kai_mai_pill_v0" -> slotIdx == 0
                ? switch (material) {
                    case "ci_she_hao" -> 3;
                    case "ling_shui" -> 1;
                    default -> 0;
                }
                : 0;
            case "hui_yuan_pill_v0" -> slotIdx == 0
                ? switch (material) {
                    case "hui_yuan_zhi" -> 2;
                    case "ling_shui" -> 1;
                    default -> 0;
                }
                : 0;
            case "du_ming_san_v0" -> switch (slotIdx) {
                case 0 -> "chi_sui_cao".equals(material) ? 4 : 0;
                case 1 -> "hui_yuan_zhi".equals(material) ? 1 : 0;
                case 2 -> "shao_hou_man".equals(material) ? 1 : 0;
                default -> 0;
            };
            default -> 0;
        };
    }

    private void updateHighlights(double mx, double my) {
        clearHighlights();
        InventoryItem dragged = dragState.draggedItem();
        if (dragged == null) return;

        if (backpack.containsPoint(mx, my)) {
            var pos = backpack.screenToGrid(mx, my);
            if (pos != null) {
                boolean valid = backpack.canPlace(dragged, pos.row(), pos.col());
                backpack.highlightArea(pos.row(), pos.col(), dragged.gridWidth(), dragged.gridHeight(),
                    valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
            }
        }
        int fIdx = furnaceSlotAt(mx, my);
        if (fIdx >= 0) {
            boolean valid = dragged.gridWidth() == 1 && dragged.gridHeight() == 1
                && furnaceItems[fIdx] == null;
            furnaceSlots[fIdx].setHighlightState(
                valid ? GridSlotComponent.HighlightState.VALID : GridSlotComponent.HighlightState.INVALID);
        }
    }

    private void clearHighlights() {
        backpack.clearHighlights();
        for (GridSlotComponent s : furnaceSlots)
            if (s != null) s.setHighlightState(GridSlotComponent.HighlightState.NONE);
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        super.render(context, mouseX, mouseY, delta);

        if (dupFlashTicks > 0) {
            dupFlashTicks--;
            if (dupFlashTicks == 0) refreshRecipeText();
        }

        if (dragState.isDragging() && dragState.draggedItem() != null) {
            InventoryItem item = dragState.draggedItem();
            int cs = GridSlotComponent.CELL_SIZE;
            int gw = item.gridWidth() * cs, gh = item.gridHeight() * cs;
            int fitSize = Math.min(gw, gh);
            int fx = mouseX - fitSize / 2, fy = mouseY - fitSize / 2;
            Identifier tex = GridSlotComponent.textureIdForItem(item);
            var m = context.getMatrices();
            m.push();
            m.translate(0, 0, 200);
            RenderSystem.enableBlend();
            RenderSystem.defaultBlendFunc();
            RenderSystem.setShaderColor(1f, 1f, 1f, 0.75f);
            m.push();
            m.translate(fx, fy, 0);
            m.scale((float) fitSize / ICON_SIZE, (float) fitSize / ICON_SIZE, 1f);
            context.drawTexture(tex, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);
            m.pop();
            RenderSystem.setShaderColor(1f, 1f, 1f, 1f);
            RenderSystem.disableBlend();
            m.pop();
        }
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    // ============= 键盘输入 =============
    // plan-alchemy-v1 §3.3: F 注真元 / ↑↓ 调温

    void attachSessionListenerForTests() {
        if (removed) return;
        if (!scope.isOpen()) {
            scope.onOpen();
            controller.onOpen(scope);
        }
        refreshSessionText();
    }

    void detachSessionListenerForTests() {
        removed = true;
        if (scope != null && !scope.isClosed()) scope.close();
        controller.onClose();
    }

    AlchemySessionPresentationPlanner.Presentation sessionPresentationForTests() {
        return sessionPresentation;
    }

    void setSessionPresentationObserverForTests(
        Consumer<AlchemySessionPresentationPlanner.Presentation> observer
    ) {
        sessionPresentationObserver = observer;
    }

    boolean takeBackForTests() {
        // 该 seam 专门验证 sender 编码，不模拟 Screen host 生命周期；生产按键仍走 guarded controller。
        return AlchemyClientIntentSink.production().dispatch(
            new AlchemyIntent.TakeBack(furnacePos, 0)
        ).kind() == UiIntentResult.Kind.LOCAL_ACCEPTED;
    }

    private static final double QI_INJECT_PER_TAP = 1.0;
    private static final double TEMP_ADJUST_STEP = 0.02;

    private boolean requestTakeBack() {
        for (int i = 0; i < FURNACE_SLOTS; i++) {
            if (furnaceItems[i] != null) {
                return controller.intentSink().dispatch(new AlchemyIntent.TakeBack(furnacePos, i)).kind()
                    == UiIntentResult.Kind.LOCAL_ACCEPTED;
            }
        }
        return controller.intentSink().dispatch(new AlchemyIntent.TakeBack(furnacePos, 0)).kind()
            == UiIntentResult.Kind.LOCAL_ACCEPTED;
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        // GLFW key codes: F=70, UP=265, DOWN=264
        if (keyCode == 70) {
            controller.intentSink().dispatch(new AlchemyIntent.InjectQi(furnacePos, QI_INJECT_PER_TAP));
            return true;
        }
        if (keyCode == 73) {
            var current = controller.viewModel().recipes().current();
            if (current != null) {
                controller.intentSink().dispatch(new AlchemyIntent.Ignite(
                    furnacePos, canonicalRecipeId(current.id())
                ));
            }
            return true;
        }
        if (keyCode == 84) {
            return requestTakeBack();
        }
        if (keyCode == 265 || keyCode == 264) {
            var s = controller.viewModel().session();
            double cur = s.tempCurrent();
            double next = keyCode == 265
                ? Math.min(1.0, cur + TEMP_ADJUST_STEP)
                : Math.max(0.0, cur - TEMP_ADJUST_STEP);
            controller.intentSink().dispatch(new AlchemyIntent.AdjustTemp(furnacePos, next));
            return true;
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }
}
