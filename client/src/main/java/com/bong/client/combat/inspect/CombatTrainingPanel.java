package com.bong.client.combat.inspect;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillRowComponent;
import com.bong.client.skill.SkillSetStore;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/** Main InspectScreen tab for combat-skill binding and technique details. */
public final class CombatTrainingPanel {
    private final FlowLayout root;
    private final FlowLayout techniqueList;
    private final TechniqueDetailCard detailCard;
    private final MeridianMiniView meridianMiniView;
    private final LabelComponent statusLine;
    private final List<FlowLayout> techniqueRows = new ArrayList<>();
    private final Consumer<List<TechniquesListPanel.Technique>> techniquesListener;
    private String selectedTechniqueId = "";

    public CombatTrainingPanel() {
        root = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        root.gap(4);
        root.padding(Insets.of(2));

        FlowLayout top = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        top.gap(4);
        top.verticalAlignment(VerticalAlignment.TOP);

        techniqueList = Containers.verticalFlow(Sizing.fixed(176), Sizing.content());
        techniqueList.surface(Surface.flat(0xFF111111).and(Surface.outline(0xFF303030)));
        techniqueList.padding(Insets.of(3));
        techniqueList.gap(2);
        top.child(techniqueList);

        detailCard = new TechniqueDetailCard();
        top.child(detailCard.component());
        root.child(top);

        FlowLayout bottom = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        bottom.gap(4);
        meridianMiniView = new MeridianMiniView();
        bottom.child(meridianMiniView);
        statusLine = Components.label(Text.literal("拖功法到左侧 1-9 槽；右键左侧技能槽清空。"));
        statusLine.color(Color.ofArgb(0xFFAAAAAA));
        FlowLayout statusBox = Containers.verticalFlow(Sizing.fixed(176), Sizing.fixed(58));
        statusBox.surface(Surface.flat(0xFF151412).and(Surface.outline(0xFF504838)));
        statusBox.padding(Insets.of(4));
        statusBox.child(statusLine);
        bottom.child(statusBox);
        root.child(bottom);

        rebuildTechniques(TechniquesListPanel.snapshot());
        techniquesListener = next -> {
            MinecraftClient client = MinecraftClient.getInstance();
            if (client != null) client.execute(() -> rebuildTechniques(next));
            else rebuildTechniques(next);
        };
        TechniquesListPanel.addListener(techniquesListener);
    }

    public FlowLayout component() {
        return root;
    }

    public void close() {
        TechniquesListPanel.removeListener(techniquesListener);
    }

    public String selectedTechniqueId() {
        return selectedTechniqueId;
    }

    public TechniquesListPanel.Technique selectedTechnique() {
        return findTechnique(selectedTechniqueId);
    }

    public void refreshFromStores() {
        refreshSelection();
        refreshStatus();
    }

    private void rebuildTechniques(List<TechniquesListPanel.Technique> snapshot) {
        techniqueList.clearChildren();
        techniqueRows.clear();
        techniqueList.child(sectionLabel("功法"));
        if (snapshot == null || snapshot.isEmpty()) {
            LabelComponent empty = Components.label(Text.literal("等待 techniques_snapshot…"));
            empty.color(Color.ofArgb(0xFF777777));
            techniqueList.child(empty);
        } else {
            for (TechniquesListPanel.Technique technique : snapshot) {
                FlowLayout row = techniqueRow(technique);
                techniqueRows.add(row);
                techniqueList.child(row);
            }
            if (findTechnique(selectedTechniqueId) == null) {
                selectedTechniqueId = snapshot.get(0).id();
            }
        }

        techniqueList.child(sectionLabel("技艺"));
        for (SkillId id : new SkillId[] { SkillId.HERBALISM, SkillId.ALCHEMY, SkillId.FORGING }) {
            SkillRowComponent row = new SkillRowComponent(id);
            row.update(SkillSetStore.snapshot().get(id), System.currentTimeMillis());
            techniqueList.child(row.component());
        }
        refreshSelection();
    }

    private FlowLayout techniqueRow(TechniquesListPanel.Technique technique) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(20));
        row.surface(Surface.flat(0xFF181818));
        row.padding(Insets.of(2, 2, 4, 4));
        row.gap(4);
        row.verticalAlignment(VerticalAlignment.CENTER);
        row.cursorStyle(CursorStyle.HAND);

        LabelComponent grade = Components.label(Text.literal(technique.grade().label()));
        grade.color(Color.ofArgb(technique.grade().color()));
        grade.horizontalSizing(Sizing.fixed(34));
        row.child(grade);
        LabelComponent name = Components.label(Text.literal(technique.displayName()));
        name.color(Color.ofArgb(0xFFE0E0E0));
        name.horizontalSizing(Sizing.fixed(76));
        row.child(name);
        LabelComponent proficiency = Components.label(Text.literal(Math.round(technique.proficiency() * 100.0f) + "%"));
        proficiency.color(Color.ofArgb(0xFF88CCAA));
        row.child(proficiency);
        row.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) {
                selectedTechniqueId = technique.id();
                refreshSelection();
                return true;
            }
            return false;
        });
        return row;
    }

    private LabelComponent sectionLabel(String text) {
        LabelComponent label = Components.label(Text.literal("§8─ §7" + text + " §8─"));
        label.horizontalSizing(Sizing.fill(100));
        return label;
    }

    private void refreshSelection() {
        TechniquesListPanel.Technique selected = selectedTechnique();
        List<TechniquesListPanel.Technique> snapshot = TechniquesListPanel.snapshot();
        for (int i = 0; i < techniqueRows.size() && i < snapshot.size(); i++) {
            boolean selectedRow = snapshot.get(i).id().equals(selectedTechniqueId);
            techniqueRows.get(i).surface(selectedRow
                ? Surface.flat(0xFF20221A).and(Surface.outline(0xFFE0B060))
                : Surface.flat(0xFF181818));
        }
        detailCard.refresh(selected);
        meridianMiniView.refresh(selected);
        refreshStatus();
    }

    private void refreshStatus() {
        TechniquesListPanel.Technique selected = selectedTechnique();
        String selectedText = selected == null ? "未选择功法" : "已选: " + selected.displayName();
        int boundSlot = selected == null ? -1 : SkillBarStore.findSkill(selected.id());
        String bindText = boundSlot >= 0 ? " · 槽 " + (boundSlot + 1) : " · 未绑定";
        statusLine.text(Text.literal(selectedText + bindText + "\n拖到左侧 1-9；右键技能槽清空。"));
    }

    public boolean bindSelectedTechniqueToSlot(int slot) {
        TechniquesListPanel.Technique technique = selectedTechnique();
        if (technique == null || slot < 0 || slot >= 9) return false;
        SkillBarStore.updateSlot(slot, SkillBarEntry.skill(
            technique.id(),
            technique.displayName(),
            technique.castTicks() * 50,
            technique.cooldownTicks() * 50,
            ""
        ));
        ClientRequestSender.sendSkillBarBindSkill(slot, technique.id());
        refreshSelection();
        return true;
    }

    public boolean clearSkillSlot(int slot) {
        if (slot < 0 || slot >= 9) return false;
        SkillBarStore.updateSlot(slot, null);
        ClientRequestSender.sendSkillBarBindClear(slot);
        refreshSelection();
        return true;
    }

    private TechniquesListPanel.Technique findTechnique(String id) {
        if (id == null || id.isBlank()) return null;
        for (TechniquesListPanel.Technique technique : TechniquesListPanel.snapshot()) {
            if (id.equals(technique.id())) return technique;
        }
        return null;
    }
}
