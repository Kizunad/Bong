package com.bong.client.npc;

import com.bong.client.state.PlayerStateStore;
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
import java.util.Locale;
import java.util.Map;

/**
 * NPC 查看屏——owo-lib 重写（plan-npc-combat-gear-v1 P2.2）。
 *
 * <p>双列布局：左（140px）=基础信息+qi/hp 状态条，右（190px）=装备+功法列表。
 * 功法可见性按玩家境界分级（醒灵/引气=未知，凝脉=名字，固元+=完整+熟练度）。
 */
public final class NpcInspectScreen extends BaseOwoScreen<FlowLayout> {
    static final int PANEL_WIDTH = 340;
    static final int LEFT_COL_WIDTH = 140;
    static final int RIGHT_COL_WIDTH = 190;
    static final int PANEL_PADDING = 12;
    static final int BAR_SEGMENTS = 10;

    // ── 颜色表 ──────────────────────────────────────────────────────────────
    static final int COLOR_HEADER = 0xE8D8A8;
    static final int COLOR_SECTION_TITLE = 0xC8A86E;
    static final int COLOR_INFO = 0xD0D0D0;
    static final int COLOR_FRIENDLY = 0x5DD17A;
    static final int COLOR_HOSTILE = 0xE05A47;
    static final int COLOR_NEUTRAL = 0xC8C8C8;
    static final int COLOR_QI_HINT = 0x8FE6B8;
    static final int COLOR_QI_BAR = 0x4A9EE0;
    static final int COLOR_HP_BAR = 0xCC4444;
    static final int COLOR_EMPTY = 0x666666;
    static final int COLOR_DIVIDER = 0x404040;
    static final int COLOR_BUTTON = 0xD0D0D0;

    // ── 功法可见性境界门槛 ──────────────────────────────────────────────────
    // 醒灵/引气: "未知" / 凝脉: 名字无熟练度 / 固元+: 完整
    static final int REALM_RANK_UNKNOWN = 1;     // 引气及以下
    static final int REALM_RANK_NAME_ONLY = 2;   // 凝脉
    static final int REALM_RANK_FULL = 3;        // 固元+

    private final NpcMetadata metadata;

    public NpcInspectScreen(NpcMetadata metadata) {
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

        // ── header ──
        String headerText = metadata.displayName() + " · "
            + NpcDialogueScreen.archetypeLabel(metadata.archetype()) + " · " + metadata.realm();
        mainPanel.child(NpcDialogueScreen.coloredLabel(headerText, COLOR_HEADER));
        mainPanel.child(NpcDialogueScreen.dividerLabel());

        // ── content row (left + right columns) ──
        FlowLayout contentRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        contentRow.gap(8);

        // ── left column: basic info + status ──
        FlowLayout leftCol = Containers.verticalFlow(Sizing.fixed(LEFT_COL_WIDTH), Sizing.content());
        leftCol.gap(2);
        leftCol.child(NpcDialogueScreen.coloredLabel("基础信息", COLOR_SECTION_TITLE));
        leftCol.child(infoLine("寿元: " + metadata.ageBand()));
        if (metadata.factionName() != null) {
            String factionText = metadata.factionRank() != null
                ? metadata.factionName() + " / " + metadata.factionRank()
                : metadata.factionName();
            leftCol.child(infoLine("派系: " + factionText));
        } else {
            leftCol.child(infoLine("派系: 无"));
        }
        leftCol.child(attitudeLine(metadata.reputationToPlayer()));
        if (metadata.qiHint() != null) {
            leftCol.child(coloredLine("气息: " + metadata.qiHint(), COLOR_QI_HINT));
        }
        leftCol.child(spacer(8));
        leftCol.child(NpcDialogueScreen.coloredLabel("状态", COLOR_SECTION_TITLE));
        leftCol.child(barLine("真元", metadata.qiRatio(), COLOR_QI_BAR));
        leftCol.child(barLine("生命", metadata.hpRatio(), COLOR_HP_BAR));
        String repLabel = NpcReputationIndicator.labelFor(metadata.reputationToPlayer());
        int repColor = NpcReputationIndicator.colorFor(metadata.reputationToPlayer()) & 0x00FFFFFF;
        leftCol.child(coloredLine("信誉: " + repLabel, repColor));

        contentRow.child(leftCol);

        // ── right column: equipment + techniques ──
        FlowLayout rightCol = Containers.verticalFlow(Sizing.fixed(RIGHT_COL_WIDTH), Sizing.content());
        rightCol.gap(2);
        rightCol.child(NpcDialogueScreen.coloredLabel("装备", COLOR_SECTION_TITLE));
        if (metadata.equipment().isEmpty()) {
            rightCol.child(coloredLine("无", COLOR_EMPTY));
        } else {
            for (Map.Entry<String, NpcEquipSlotData> entry : metadata.equipment().entrySet()) {
                rightCol.child(equipLine(entry.getKey(), entry.getValue()));
            }
        }
        rightCol.child(spacer(8));
        rightCol.child(NpcDialogueScreen.coloredLabel("已知功法", COLOR_SECTION_TITLE));
        int playerRealmRank = playerRealmRank();
        if (metadata.techniques().isEmpty()) {
            rightCol.child(coloredLine("无", COLOR_EMPTY));
        } else if (playerRealmRank <= REALM_RANK_UNKNOWN) {
            rightCol.child(coloredLine("功法: 未知", COLOR_EMPTY));
        } else {
            for (NpcTechniqueData tech : metadata.techniques()) {
                rightCol.child(techLine(tech, playerRealmRank));
            }
        }

        contentRow.child(rightCol);
        mainPanel.child(contentRow);

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

    // ── test-only describe ──────────────────────────────────────────────────

    /** Pure-function layout description for unit tests. */
    public static RenderContent describe(NpcMetadata metadata, int playerRealmRank) {
        if (metadata == null) {
            return new RenderContent(true, List.of("null metadata"));
        }
        List<String> lines = new ArrayList<>();
        lines.add("header: " + metadata.displayName() + " · "
            + NpcDialogueScreen.archetypeLabel(metadata.archetype()) + " · " + metadata.realm());
        lines.add("age: " + metadata.ageBand());
        if (metadata.factionName() != null) {
            lines.add("faction: " + metadata.factionName()
                + (metadata.factionRank() != null ? " / " + metadata.factionRank() : ""));
        } else {
            lines.add("faction: 无");
        }
        lines.add("attitude: " + attitudeLabel(metadata.reputationToPlayer()));
        if (metadata.qiHint() != null) {
            lines.add("qi_hint: " + metadata.qiHint());
        }
        lines.add("qi_bar: " + formatPercent(metadata.qiRatio()));
        lines.add("hp_bar: " + formatPercent(metadata.hpRatio()));

        // equipment
        if (metadata.equipment().isEmpty()) {
            lines.add("equip: 无");
        } else {
            for (Map.Entry<String, NpcEquipSlotData> entry : metadata.equipment().entrySet()) {
                NpcEquipSlotData slot = entry.getValue();
                lines.add("equip:" + slotLabel(entry.getKey()) + ": "
                    + slot.displayName() + " (" + slot.qualityLabel() + ") "
                    + formatPercent(slot.durabilityRatio()));
            }
        }

        // techniques
        if (metadata.techniques().isEmpty()) {
            lines.add("tech: 无");
        } else if (playerRealmRank <= REALM_RANK_UNKNOWN) {
            lines.add("tech: 未知");
        } else {
            for (NpcTechniqueData tech : metadata.techniques()) {
                if (playerRealmRank >= REALM_RANK_FULL) {
                    lines.add("tech: " + tech.displayName() + " "
                        + String.format(Locale.ROOT, "%.2f", tech.proficiency()));
                } else {
                    lines.add("tech: " + tech.displayName());
                }
            }
        }

        return new RenderContent(false, lines);
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    private static LabelComponent infoLine(String text) {
        return NpcDialogueScreen.coloredLabel(text, COLOR_INFO);
    }

    private static LabelComponent coloredLine(String text, int rgb) {
        return NpcDialogueScreen.coloredLabel(text, rgb);
    }

    private static FlowLayout barLine(String label, double ratio, int barColor) {
        FlowLayout row = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        row.gap(2);
        row.child(NpcDialogueScreen.coloredLabel(label + ": ", COLOR_INFO));
        row.child(NpcDialogueScreen.coloredLabel(buildBar(ratio), barColor));
        row.child(NpcDialogueScreen.coloredLabel(" " + formatPercent(ratio), barColor));
        return row;
    }

    private static LabelComponent attitudeLine(int reputation) {
        String label = attitudeLabel(reputation);
        int color = reputation > 50 ? COLOR_FRIENDLY : reputation < -30 ? COLOR_HOSTILE : COLOR_NEUTRAL;
        return coloredLine("态度: " + label, color);
    }

    static String attitudeLabel(int reputation) {
        if (reputation > 50) {
            return "友善";
        }
        if (reputation < -30) {
            return "敌意";
        }
        return "中立";
    }

    private static FlowLayout equipLine(String slotName, NpcEquipSlotData slot) {
        FlowLayout row = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        row.gap(2);
        row.child(NpcDialogueScreen.coloredLabel(slotLabel(slotName) + ": ", COLOR_INFO));
        row.child(NpcDialogueScreen.coloredLabel(slot.displayName(), slot.qualityColor()));
        if (slot.qualityTier() > 0) {
            row.child(NpcDialogueScreen.coloredLabel(" (" + slot.qualityLabel() + ")", slot.qualityColor()));
        }
        row.child(NpcDialogueScreen.coloredLabel(" " + buildBar(slot.durabilityRatio())
            + " " + formatPercent(slot.durabilityRatio()), slot.durabilityColor()));
        return row;
    }

    private static FlowLayout techLine(NpcTechniqueData tech, int playerRealmRank) {
        FlowLayout row = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        row.gap(4);
        row.child(NpcDialogueScreen.coloredLabel(tech.displayName(), COLOR_INFO));
        if (playerRealmRank >= REALM_RANK_FULL) {
            row.child(NpcDialogueScreen.coloredLabel(
                " " + buildBar(tech.proficiency())
                    + " " + String.format(Locale.ROOT, "%.2f", tech.proficiency()),
                tech.proficiencyColor()));
        }
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

    private static FlowLayout spacer(int height) {
        return Containers.verticalFlow(Sizing.fixed(1), Sizing.fixed(height));
    }

    static String buildBar(double ratio) {
        double clamped = Math.max(0.0, Math.min(1.0, ratio));
        int filled = (int) Math.round(clamped * BAR_SEGMENTS);
        StringBuilder sb = new StringBuilder(BAR_SEGMENTS);
        for (int i = 0; i < BAR_SEGMENTS; i++) {
            sb.append(i < filled ? '█' : '░');
        }
        return sb.toString();
    }

    static String formatPercent(double ratio) {
        return Math.round(Math.max(0.0, Math.min(1.0, ratio)) * 100.0) + "%";
    }

    static String slotLabel(String slotName) {
        return switch (slotName == null ? "" : slotName) {
            case "main_hand" -> "主手";
            case "off_hand" -> "副手";
            case "head" -> "头盔";
            case "chest" -> "胸甲";
            case "legs" -> "腿甲";
            case "feet" -> "靴子";
            default -> slotName;
        };
    }

    /**
     * 获取当前玩家境界排名用于功法可见性判断。
     * 醒灵=0, 引气=1, 凝脉=2, 固元=3, 通灵=4, 化虚=5。
     */
    static int playerRealmRank() {
        String realm = PlayerStateStore.snapshot().realm();
        return realmToRank(realm);
    }

    static int realmToRank(String realm) {
        if (realm == null || realm.isBlank()) {
            return 0;
        }
        return switch (realm) {
            case "醒灵" -> 0;
            case "引气" -> 1;
            case "凝脉" -> 2;
            case "固元" -> 3;
            case "通灵" -> 4;
            case "化虚" -> 5;
            default -> 0;
        };
    }

    public record RenderContent(boolean placeholder, List<String> lines) {
        public RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
