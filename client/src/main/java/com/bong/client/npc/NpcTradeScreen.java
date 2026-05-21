package com.bong.client.npc;

import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

/**
 * NPC 交易屏——owo-lib 重写（plan-npc-combat-gear-v1 P2.3）。
 *
 * <p>双列布局：左=NPC 货物可滚动列表（170px），右=玩家出价+确认按钮（170px）。
 * 骨币定价走枚数（整数），server 端按 shelflife 模块做折算。
 */
public final class NpcTradeScreen extends BaseOwoScreen<FlowLayout> {
    static final int PANEL_WIDTH = 360;
    static final int WARES_COL_WIDTH = 170;
    static final int OFFER_COL_WIDTH = 170;
    static final int PANEL_PADDING = 12;
    static final int WARES_MAX_VISIBLE = 4;
    static final int WARE_ROW_HEIGHT = 22;

    // ── 颜色表 ──────────────────────────────────────────────────────────────
    static final int COLOR_HEADER = 0xE8D8A8;
    static final int COLOR_BALANCE = 0xC8A86E;
    static final int COLOR_DIVIDER = 0x404040;
    static final int COLOR_WARE_NAME = 0xD0D0D0;
    static final int COLOR_WARE_PRICE = 0xB8E6B8;
    static final int COLOR_SELECTED = 0x5DD17A;
    static final int COLOR_ARROW = 0xC8A86E;
    static final int COLOR_PRICE_TOTAL = 0xE0D8C8;
    static final int COLOR_DISCOUNT = 0x5DD17A;
    static final int COLOR_INSUFFICIENT = 0xE05A47;
    static final int COLOR_BUTTON = 0xD0D0D0;
    static final int COLOR_BUTTON_DISABLED = 0x666666;
    static final int COLOR_SECTION_TITLE = 0xC8A86E;

    private final NpcMetadata metadata;
    private int selectedIndex = -1;

    // UI references for dynamic updates
    private FlowLayout offerPanel;
    private FlowLayout waresList;

    public NpcTradeScreen(NpcMetadata metadata) {
        super(Text.literal(metadata == null ? "NPC Trade" : metadata.displayName()));
        this.metadata = metadata;
    }

    NpcMetadata metadata() {
        return metadata;
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout rootComponent) {
        if (metadata == null) {
            close();
            return;
        }

        rootComponent.surface(Surface.VANILLA_TRANSLUCENT);
        rootComponent.horizontalAlignment(HorizontalAlignment.CENTER);
        rootComponent.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout mainPanel = Containers.verticalFlow(Sizing.fixed(PANEL_WIDTH), Sizing.content());
        mainPanel.surface(Surface.DARK_PANEL);
        mainPanel.padding(Insets.of(PANEL_PADDING));
        mainPanel.gap(4);

        // ── header row ──
        FlowLayout headerRow = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        headerRow.child(NpcDialogueScreen.coloredLabel(
            metadata.displayName() + " · 交易", COLOR_HEADER));
        // placeholder balance — real value would need player state
        headerRow.child(NpcDialogueScreen.coloredLabel("骨币: -- 枚", COLOR_BALANCE));
        headerRow.horizontalAlignment(HorizontalAlignment.LEFT);
        mainPanel.child(headerRow);

        // ── divider ──
        mainPanel.child(NpcDialogueScreen.dividerLabel());

        // ── trade body: left (NPC wares) + right (player offer) ──
        FlowLayout tradeBody = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        tradeBody.gap(4);

        // ── left: NPC wares ──
        FlowLayout npcWaresCol = Containers.verticalFlow(Sizing.fixed(WARES_COL_WIDTH), Sizing.content());
        npcWaresCol.gap(2);
        npcWaresCol.child(NpcDialogueScreen.coloredLabel("NPC 货物", COLOR_SECTION_TITLE));

        waresList = Containers.verticalFlow(Sizing.fixed(WARES_COL_WIDTH), Sizing.content());
        waresList.gap(1);
        rebuildWaresList();

        if (metadata.tradeOffers().size() > WARES_MAX_VISIBLE) {
            ScrollContainer<FlowLayout> scroll = Containers.verticalScroll(
                Sizing.fixed(WARES_COL_WIDTH),
                Sizing.fixed(WARE_ROW_HEIGHT * WARES_MAX_VISIBLE),
                waresList
            );
            npcWaresCol.child(scroll);
        } else {
            npcWaresCol.child(waresList);
        }

        tradeBody.child(npcWaresCol);

        // ── separator ──
        FlowLayout separator = Containers.verticalFlow(Sizing.fixed(1), Sizing.fixed(
            WARE_ROW_HEIGHT * Math.min(metadata.tradeOffers().size(), WARES_MAX_VISIBLE) + 40));
        separator.surface(Surface.flat(0xFF000000 | COLOR_DIVIDER));
        tradeBody.child(separator);

        // ── right: player offer ──
        offerPanel = Containers.verticalFlow(Sizing.fixed(OFFER_COL_WIDTH), Sizing.content());
        offerPanel.gap(4);
        offerPanel.child(NpcDialogueScreen.coloredLabel("你的出价", COLOR_SECTION_TITLE));
        rebuildOfferPanel();
        tradeBody.child(offerPanel);

        mainPanel.child(tradeBody);

        // ── divider + buttons ──
        mainPanel.child(NpcDialogueScreen.dividerLabel());
        FlowLayout buttonRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        buttonRow.gap(8);
        buttonRow.child(textButton("返回", () -> {
            MinecraftClient.getInstance().setScreen(new NpcDialogueScreen(metadata));
        }));
        buttonRow.child(textButton("关闭", this::close));
        mainPanel.child(buttonRow);

        rootComponent.child(mainPanel);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    // ── dynamic rebuild ─────────────────────────────────────────────────────

    private void rebuildWaresList() {
        waresList.clearChildren();
        List<NpcTradeOffer> offers = metadata.tradeOffers();
        for (int i = 0; i < offers.size(); i++) {
            waresList.child(wareRow(offers.get(i), i));
        }
    }

    private void rebuildOfferPanel() {
        // Keep only the title (first child) if rebuilding
        while (offerPanel.children().size() > 1) {
            offerPanel.removeChild(offerPanel.children().get(offerPanel.children().size() - 1));
        }

        if (selectedIndex < 0 || selectedIndex >= metadata.tradeOffers().size()) {
            offerPanel.child(NpcDialogueScreen.coloredLabel("请选择物品", COLOR_BUTTON_DISABLED));
            return;
        }

        NpcTradeOffer offer = metadata.tradeOffers().get(selectedIndex);
        offerPanel.child(NpcDialogueScreen.coloredLabel(
            offer.displayName() + " x" + offer.count(), COLOR_SELECTED));
        offerPanel.child(NpcDialogueScreen.coloredLabel(
            "总价: " + offer.priceBoneCoins() + " 骨币", COLOR_PRICE_TOTAL));

        // discount line (reputation > 50)
        if (metadata.reputationToPlayer() > 50) {
            offerPanel.child(NpcDialogueScreen.coloredLabel("信誉折扣: 8折", COLOR_DISCOUNT));
        }

        // confirm button
        FlowLayout confirmBox = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        confirmBox.surface(Surface.DARK_PANEL);
        confirmBox.padding(Insets.of(4, 4, 8, 8));
        LabelComponent confirmLabel = NpcDialogueScreen.coloredLabel("确认交易", COLOR_BUTTON);
        confirmBox.child(confirmLabel);
        confirmBox.mouseEnter().subscribe(() -> confirmLabel.color(Color.ofArgb(0xFFFFFFFF)));
        confirmBox.mouseLeave().subscribe(() ->
            confirmLabel.color(Color.ofArgb(0xFF000000 | COLOR_BUTTON)));
        confirmBox.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                ClientRequestSender.sendNpcTradeRequest(
                    metadata.entityId(), List.of(), offer.templateId());
                close();
                return true;
            }
            return false;
        });
        offerPanel.child(confirmBox);
    }

    private void selectWare(int index) {
        selectedIndex = index;
        rebuildWaresList();
        rebuildOfferPanel();
    }

    // ── test-only describe ──────────────────────────────────────────────────

    /** Pure-function layout description for unit tests. */
    public static RenderContent describe(NpcMetadata metadata) {
        if (metadata == null) {
            return new RenderContent(true, List.of("null metadata"));
        }
        List<String> lines = new ArrayList<>();
        lines.add("header: " + metadata.displayName() + " · 交易");
        lines.add("balance: 骨币 -- 枚");
        for (NpcTradeOffer offer : metadata.tradeOffers()) {
            lines.add("ware: " + offer.displayName() + " x" + offer.count()
                + " = " + offer.priceBoneCoins() + " 骨币");
        }
        if (metadata.tradeOffers().isEmpty()) {
            lines.add("ware: 无可售物品");
        }
        if (metadata.reputationToPlayer() > 50) {
            lines.add("discount: 8折");
        }
        return new RenderContent(false, lines);
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    private FlowLayout wareRow(NpcTradeOffer offer, int index) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fixed(WARES_COL_WIDTH), Sizing.fixed(WARE_ROW_HEIGHT));
        row.gap(4);
        row.padding(Insets.horizontal(4));
        row.verticalAlignment(VerticalAlignment.CENTER);

        if (index == selectedIndex) {
            row.child(NpcDialogueScreen.coloredLabel("▸ ", COLOR_ARROW));
        }

        LabelComponent nameLabel = NpcDialogueScreen.coloredLabel(
            offer.displayName() + " x" + offer.count(),
            index == selectedIndex ? COLOR_SELECTED : COLOR_WARE_NAME);
        row.child(nameLabel);
        row.child(NpcDialogueScreen.coloredLabel(
            " " + offer.priceBoneCoins() + "骨币", COLOR_WARE_PRICE));

        row.mouseEnter().subscribe(() -> row.surface(Surface.flat(0x30FFFFFF)));
        row.mouseLeave().subscribe(() -> row.surface(Surface.BLANK));
        row.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                selectWare(index);
                return true;
            }
            return false;
        });
        return row;
    }

    private FlowLayout textButton(String label, Runnable onClick) {
        FlowLayout box = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        box.surface(Surface.DARK_PANEL);
        box.padding(Insets.of(4, 4, 8, 8));
        LabelComponent btnLabel = NpcDialogueScreen.coloredLabel(label, COLOR_BUTTON);
        box.child(btnLabel);
        box.mouseEnter().subscribe(() -> btnLabel.color(Color.ofArgb(0xFFFFFFFF)));
        box.mouseLeave().subscribe(() -> btnLabel.color(Color.ofArgb(0xFF000000 | COLOR_BUTTON)));
        box.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                onClick.run();
                return true;
            }
            return false;
        });
        return box;
    }

    public record RenderContent(boolean placeholder, List<String> lines) {
        public RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
