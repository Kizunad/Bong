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
    /** 功法列表滚动视口固定高度（px）；约 8 行（每行 20 + gap 2 + 上下 padding 3）。 */
    private static final int LIST_VIEWPORT_HEIGHT = 180;

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

        techniqueList = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        techniqueList.surface(Surface.flat(0xFF111111).and(Surface.outline(0xFF303030)));
        techniqueList.padding(Insets.of(3));
        techniqueList.gap(2);
        // 功法可能很多（玄阶/黄阶各路招式），固定视口高度 + 纵向滚动条，
        // 否则列表随内容无限拉长、溢出屏幕底部（无法滚动查看）。
        var techniqueScroll = Containers.verticalScroll(
            Sizing.fixed(176), Sizing.fixed(LIST_VIEWPORT_HEIGHT), techniqueList);
        techniqueScroll.scrollbarThiccness(3);
        listColumn.child(techniqueScroll);
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
        statusLine = Components.label(Text.literal("拖功法到 1-9 槽绑定；绑定后可拖出槽外解绑，或右键清空。"));
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
        hoverTechniqueId = "";
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
        statusLine.text(Text.literal(selectedText + bindText + "\n拖到 1-9 绑定 / 拖出槽外解绑 / 右键清空。"));
    }

    public boolean bindSelectedTechniqueToSlot(int slot) {
        return bindTechniqueToSlot(selectedTechniqueId, slot);
    }

    /**
     * 把指定 id 的功法绑到 1-9 槽（拖拽落槽走这条，不依赖「当前选中」状态）。
     * 锁定功法（经脉断/封闭、未激活、config 必填缺失）拒绝绑定并在状态栏给原因。
     */
    public boolean bindTechniqueToSlot(String techniqueId, int slot) {
        TechniquesListPanel.Technique technique = findTechnique(techniqueId);
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

    /** 选中指定功法（拖拽起手时调用，使高亮/详情卡与两步点击一致）。 */
    public void selectTechnique(String techniqueId) {
        if (techniqueId == null || techniqueId.isBlank()) return;
        if (findVisibleTechnique(techniqueId) == null) return;
        selectedTechniqueId = techniqueId;
        hoverTechniqueId = "";
        configPanelManager.onSelectedTechniqueChanged(selectedTechniqueId);
        refreshSelection();
    }

    /** 命中测试：屏幕坐标落在哪一行功法（拖拽起手用）；落在滚动条/空白/视口外返回 null。 */
    public TechniquesListPanel.Technique techniqueAtScreen(double mouseX, double mouseY) {
        for (TechniqueRowComponent row : techniqueRows) {
            io.wispforest.owo.ui.core.Component c = row.component();
            if (rowHit(mouseX, mouseY, c.x(), c.y(), c.width(), c.height())) {
                return row.technique();
            }
        }
        return null;
    }

    /** 锁定原因（空串=可绑定），供拖拽 ghost / 槽位高亮判定禁止态。 */
    public String lockReasonFor(TechniquesListPanel.Technique technique) {
        return lockReason(technique);
    }

    /** 按 id 取功法（全量 snapshot，不受搜索过滤影响）；从已绑槽拖出时用来还原 Technique。 */
    public TechniquesListPanel.Technique techniqueById(String techniqueId) {
        return findTechnique(techniqueId);
    }

    /** 半开区间矩形命中：{@code [x, x+w) × [y, y+h)}，与 hotbar 槽位命中保持同一约定。 */
    static boolean rowHit(double px, double py, int x, int y, int w, int h) {
        return px >= x && px < x + w && py >= y && py < y + h;
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
