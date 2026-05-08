package com.bong.client.combat.inspect;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SkillConfigStore;
import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/** Main InspectScreen tab for technique binding and details. */
public final class TechniquesTabPanel {
    private final FlowLayout root;
    private final FlowLayout techniqueList;
    private final TechniqueSearchBar searchBar;
    private final TechniqueDetailCard detailCard;
    private final SkillConfigPanelManager configPanelManager;
    private final MeridianMiniSilhouette meridianMiniView;
    private final LabelComponent statusLine;
    private final List<TechniqueRowComponent> techniqueRows = new ArrayList<>();
    private final Consumer<List<TechniquesListPanel.Technique>> techniquesListener;
    private final Consumer<List<MeridianChannel>> meridianHighlightSink;
    private List<TechniquesListPanel.Technique> snapshot = List.of();
    private List<TechniquesListPanel.Technique> visibleTechniques = List.of();
    private String selectedTechniqueId = "";
    private String hoverTechniqueId = "";

    public TechniquesTabPanel() {
        this(channels -> {});
    }

    public TechniquesTabPanel(Consumer<List<MeridianChannel>> meridianHighlightSink) {
        this.meridianHighlightSink = meridianHighlightSink == null ? channels -> {} : meridianHighlightSink;
        root = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        root.gap(4);
        root.padding(Insets.of(2));

        FlowLayout top = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        top.gap(4);
        top.verticalAlignment(VerticalAlignment.TOP);

        FlowLayout listColumn = Containers.verticalFlow(Sizing.fixed(176), Sizing.content());
        listColumn.gap(3);
        searchBar = new TechniqueSearchBar(query -> {
            refreshVisibleTechniques();
            refreshSelection();
        });
        listColumn.child(searchBar.component());

        techniqueList = Containers.verticalFlow(Sizing.fixed(176), Sizing.content());
        techniqueList.surface(Surface.flat(0xFF111111).and(Surface.outline(0xFF303030)));
        techniqueList.padding(Insets.of(3));
        techniqueList.gap(2);
        listColumn.child(techniqueList);
        top.child(listColumn);

        FlowLayout configLayer = Containers.verticalFlow(Sizing.fixed(1), Sizing.fixed(1));
        configPanelManager = new SkillConfigPanelManager(configLayer, this::refreshSelection);
        detailCard = new TechniqueDetailCard(technique -> {
            MinecraftClient client = MinecraftClient.getInstance();
            int screenWidth = client == null ? 480 : client.getWindow().getScaledWidth();
            int screenHeight = client == null ? 320 : client.getWindow().getScaledHeight();
            configPanelManager.open(technique, 190, 18, screenWidth, screenHeight);
        });
        top.child(detailCard.component());
        root.child(top);

        FlowLayout bottom = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        bottom.gap(4);
        meridianMiniView = new MeridianMiniSilhouette();
        bottom.child(meridianMiniView);
        statusLine = Components.label(Text.literal("拖功法到左侧 1-9 槽；右键左侧技能槽清空。"));
        statusLine.color(Color.ofArgb(0xFFAAAAAA));
        FlowLayout statusBox = Containers.verticalFlow(Sizing.fixed(176), Sizing.fixed(58));
        statusBox.surface(Surface.flat(0xFF151412).and(Surface.outline(0xFF504838)));
        statusBox.padding(Insets.of(4));
        statusBox.child(statusLine);
        bottom.child(statusBox);
        root.child(bottom);
        root.child(configLayer);

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
        configPanelManager.dispose();
        meridianHighlightSink.accept(List.of());
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

    private void rebuildTechniques(List<TechniquesListPanel.Technique> next) {
        snapshot = next == null ? List.of() : List.copyOf(next);
        refreshVisibleTechniques();
        refreshSelection();
    }

    private void refreshVisibleTechniques() {
        String previousSelection = selectedTechniqueId;
        visibleTechniques = TechniquesListPanel.filter(snapshot, searchBar.query());
        techniqueList.clearChildren();
        techniqueRows.clear();
        if (visibleTechniques.isEmpty()) {
            String emptyText = snapshot.isEmpty() ? "等待 techniques_snapshot…" : "未找到功法";
            LabelComponent empty = Components.label(Text.literal(emptyText));
            empty.color(Color.ofArgb(0xFF777777));
            techniqueList.child(empty);
            selectedTechniqueId = "";
        } else {
            for (TechniquesListPanel.Technique technique : visibleTechniques) {
                TechniqueRowComponent row = new TechniqueRowComponent(
                    technique,
                    selected -> {
                        selectedTechniqueId = selected.id();
                        hoverTechniqueId = "";
                        configPanelManager.onSelectedTechniqueChanged(selectedTechniqueId);
                        refreshSelection();
                    },
                    hovered -> {
                        hoverTechniqueId = hovered.id();
                        refreshSelection();
                    },
                    () -> {
                        hoverTechniqueId = "";
                        refreshSelection();
                    }
                );
                techniqueRows.add(row);
                techniqueList.child(row.component());
            }
            if (findVisibleTechnique(selectedTechniqueId) == null) {
                selectedTechniqueId = visibleTechniques.get(0).id();
            }
        }
        if (!previousSelection.equals(selectedTechniqueId)) {
            configPanelManager.onSelectedTechniqueChanged(selectedTechniqueId);
        }
    }

    private void refreshSelection() {
        TechniquesListPanel.Technique selected = selectedTechnique();
        for (TechniqueRowComponent row : techniqueRows) {
            boolean selectedRow = row.technique().id().equals(selectedTechniqueId);
            row.refresh(selectedRow, lockReason(row.technique()));
        }
        detailCard.refresh(selected);
        TechniquesListPanel.Technique highlighted = highlightedTechnique();
        meridianMiniView.refresh(highlighted);
        meridianHighlightSink.accept(TechniquesListPanel.requiredChannels(highlighted));
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
        String lockReason = lockReason(technique);
        if (!lockReason.isBlank()) {
            statusLine.text(Text.literal("不可绑定: " + lockReason));
            return false;
        }
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
        for (TechniquesListPanel.Technique technique : snapshot) {
            if (id.equals(technique.id())) return technique;
        }
        return null;
    }

    private TechniquesListPanel.Technique findVisibleTechnique(String id) {
        if (id == null || id.isBlank()) return null;
        for (TechniquesListPanel.Technique technique : visibleTechniques) {
            if (id.equals(technique.id())) return technique;
        }
        return null;
    }

    private TechniquesListPanel.Technique highlightedTechnique() {
        TechniquesListPanel.Technique hovered = findVisibleTechnique(hoverTechniqueId);
        return hovered == null ? selectedTechnique() : hovered;
    }

    private String lockReason(TechniquesListPanel.Technique technique) {
        if (technique == null) return "";
        String configReason = SkillConfigSchemaRegistry.missingRequiredReason(
            technique.id(),
            SkillConfigStore.configFor(technique.id())
        );
        if (!configReason.isBlank()) return configReason;
        if (!technique.active()) return "尚未激活";
        MeridianBody body = MeridianStateStore.snapshot();
        if (body == null) return "";
        for (TechniquesListPanel.RequiredMeridian required : technique.requiredMeridians()) {
            MeridianChannel channel = TechniquesListPanel.channelFromWire(required.channel()).orElse(null);
            if (channel == null) continue;
            ChannelState state = body.channel(channel);
            if (state == null) continue;
            if (state.damage() == ChannelState.DamageLevel.SEVERED) {
                return channel.displayName() + "已断";
            }
            if (state.blocked()) {
                return channel.displayName() + "已封闭";
            }
            double health = state.capacity() <= 0 ? 0.0 : state.effectiveFlow() / state.capacity();
            if (health < required.minHealth()) {
                return channel.displayName() + "健康不足";
            }
        }
        return "";
    }
}
