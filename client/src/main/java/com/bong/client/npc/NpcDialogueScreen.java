package com.bong.client.npc;

import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
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
 * NPC 对话主屏——owo-lib 重写（plan-npc-combat-gear-v1 P2.1）。
 *
 * <p>布局：headerRow + greeting + option rows + reputation bar。
 */
public final class NpcDialogueScreen extends BaseOwoScreen<FlowLayout> {
    static final int PANEL_WIDTH = 280;
    static final int PANEL_PADDING = 12;

    // ── 颜色表 ──────────────────────────────────────────────────────────────
    static final int COLOR_NPC_NAME = 0xE8D8A8;
    static final int COLOR_REALM_BADGE = 0x8FE6B8;
    static final int COLOR_ARCHETYPE_TAG = 0xA89070;
    static final int COLOR_DIALOGUE = 0xD0D0D0;
    static final int COLOR_OPTION_TEXT = 0xE0D8C8;
    static final int COLOR_OPTION_ARROW = 0xC8A86E;
    static final int COLOR_LEAVE_TEXT = 0xA0A0A0;
    static final int COLOR_LEAVE_ARROW = 0x888888;
    static final int COLOR_HOSTILE = 0xE05A47;
    static final int COLOR_DIVIDER = 0x404040;
    static final int COLOR_OPTION_HOVER = 0xFFFFFF;

    private final NpcMetadata metadata;

    public NpcDialogueScreen(NpcMetadata metadata) {
        super(Text.literal(metadata == null ? "NPC" : metadata.displayName()));
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
        mainPanel.horizontalAlignment(HorizontalAlignment.LEFT);

        // ── header row (spec: archetype[4px]name[8px]realm) ──
        FlowLayout headerRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        headerRow.child(coloredLabel("[" + archetypeLabel(metadata.archetype()) + "]", COLOR_ARCHETYPE_TAG));
        headerRow.child(Containers.verticalFlow(Sizing.fixed(4), Sizing.fixed(1)));  // 4px spacer
        headerRow.child(coloredLabel(metadata.displayName(), COLOR_NPC_NAME));
        headerRow.child(Containers.verticalFlow(Sizing.fixed(8), Sizing.fixed(1)));  // 8px spacer
        headerRow.child(coloredLabel(realmBadge(metadata.realm()), COLOR_REALM_BADGE));
        mainPanel.child(headerRow);

        // ── divider ──
        mainPanel.child(dividerLabel());

        // ── greeting text ──
        String greeting = metadata.hostile() ? "此人对你充满敌意。" : metadata.greetingText();
        int greetingColor = metadata.hostile() ? COLOR_HOSTILE : COLOR_DIALOGUE;
        LabelComponent greetingLabel = Components.label(Text.literal(greeting))
            .color(Color.ofArgb(0xFF000000 | greetingColor))
            .maxWidth(PANEL_WIDTH - PANEL_PADDING * 2);
        mainPanel.child(greetingLabel);

        // ── divider ──
        mainPanel.child(dividerLabel());

        // ── option rows ──
        if (metadata.tradeCandidate()) {
            mainPanel.child(optionRow("看看你有什么好东西", COLOR_OPTION_ARROW, COLOR_OPTION_TEXT, () -> {
                ClientRequestSender.sendNpcDialogueChoice(metadata.entityId(), "trade");
                MinecraftClient.getInstance().setScreen(new NpcTradeScreen(metadata));
            }));
        }

        mainPanel.child(optionRow("让我打量一下你", COLOR_OPTION_ARROW, COLOR_OPTION_TEXT, () -> {
            ClientRequestSender.sendNpcDialogueChoice(metadata.entityId(), "inspect");
            ClientRequestSender.sendNpcInspectRequest(metadata.entityId());
            MinecraftClient.getInstance().setScreen(new NpcInspectScreen(metadata));
        }));

        mainPanel.child(optionRow("告辞", COLOR_LEAVE_ARROW, COLOR_LEAVE_TEXT, () -> {
            ClientRequestSender.sendNpcDialogueChoice(metadata.entityId(), "leave");
            close();
        }));

        // ── divider ──
        mainPanel.child(dividerLabel());

        // ── reputation bar ──
        String repLabel = "信誉: " + NpcReputationIndicator.labelFor(metadata.reputationToPlayer());
        int repColor = NpcReputationIndicator.colorFor(metadata.reputationToPlayer());
        mainPanel.child(coloredLabel(repLabel, repColor & 0x00FFFFFF));

        rootComponent.child(mainPanel);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    // ── test-only describe ──────────────────────────────────────────────────

    /** Pure-function layout description for unit tests. */
    public static RenderContent describe(NpcMetadata metadata) {
        if (metadata == null) {
            return new RenderContent(true, List.of("null metadata"));
        }
        List<String> lines = new ArrayList<>();
        lines.add("[" + archetypeLabel(metadata.archetype()) + "] "
            + metadata.displayName() + " " + realmBadge(metadata.realm()));
        if (metadata.hostile()) {
            lines.add("此人对你充满敌意。");
        } else {
            lines.add(metadata.greetingText());
        }
        if (metadata.tradeCandidate()) {
            lines.add("option: 看看你有什么好东西");
        }
        lines.add("option: 让我打量一下你");
        lines.add("option: 告辞");
        lines.add("rep: " + NpcReputationIndicator.labelFor(metadata.reputationToPlayer()));
        return new RenderContent(false, lines);
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    private FlowLayout optionRow(String text, int arrowColor, int textColor, Runnable onClick) {
        FlowLayout row = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        row.gap(4);
        row.padding(Insets.vertical(2));
        LabelComponent arrow = coloredLabel("▸", arrowColor);
        LabelComponent label = coloredLabel(text, textColor);
        row.child(arrow);
        row.child(label);
        row.mouseEnter().subscribe(() -> {
            label.color(Color.ofArgb(0xFF000000 | COLOR_OPTION_HOVER));
        });
        row.mouseLeave().subscribe(() -> {
            label.color(Color.ofArgb(0xFF000000 | textColor));
        });
        row.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                onClick.run();
                return true;
            }
            return false;
        });
        return row;
    }

    static LabelComponent coloredLabel(String text, int rgb) {
        return Components.label(Text.literal(text)).color(Color.ofArgb(0xFF000000 | rgb));
    }

    static LabelComponent dividerLabel() {
        return coloredLabel("────────────────────────", COLOR_DIVIDER);
    }

    static String archetypeLabel(String archetype) {
        return switch (archetype == null ? "" : archetype) {
            case "rogue" -> "散修";
            case "commoner" -> "凡人";
            case "disciple" -> "宗门弟子";
            case "beast" -> "异兽";
            case "zombie" -> "游尸";
            case "guardian_relic" -> "遗迹守卫";
            case "daoxiang" -> "道伥";
            case "zhinian" -> "执念";
            case "fuya" -> "负压畸变体";
            case "skull_fiend" -> "骨煞";
            default -> "未知";
        };
    }

    static String realmBadge(String realm) {
        return "· " + (realm == null || realm.isBlank() ? "未知" : realm);
    }

    public record RenderContent(boolean placeholder, List<String> lines) {
        public RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
