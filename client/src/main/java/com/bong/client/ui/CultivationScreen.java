package com.bong.client.ui;

import com.bong.client.PlayerStateCache;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.StackLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Positioning;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;
import org.jetbrains.annotations.NotNull;

public class CultivationScreen extends BaseOwoScreen<FlowLayout> {
    private static final int PANEL_WIDTH = 232;
    private static final int BAR_WIDTH = 176;
    private static final int BAR_HEIGHT = 8;
    private static final int SECTION_GAP = 6;
    private static final int PANEL_BACKGROUND = 0xD418120C;
    private static final int PANEL_BORDER = 0xAA8F6A2A;
    private static final int TITLE_COLOR = 0xE9D7A5;
    private static final int PRIMARY_TEXT = 0xF3E9D2;
    private static final int MUTED_TEXT = 0xB7A58A;
    private static final int QI_FILL = 0xFF67D8B5;
    private static final int KARMA_FILL = 0xFFC18A56;
    private static final int POWER_FILL = 0xFF8EB8FF;
    private static final int TRACK_FILL = 0x66231811;
    private static final int TRACK_OUTLINE = 0x884C3924;
    private static final int DIVIDER = 0x99563F21;

    private final CultivationScreenModel model;

    public CultivationScreen() {
        this(PlayerStateCache.peek());
    }

    public CultivationScreen(PlayerStateCache.PlayerStateSnapshot snapshot) {
        super(Text.literal("修仙面板"));
        this.model = CultivationScreenModel.from(snapshot);
    }

    @Override
    protected @NotNull OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout rootComponent) {
        rootComponent.surface(Surface.VANILLA_TRANSLUCENT);
        rootComponent.alignment(HorizontalAlignment.CENTER, VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.fixed(PANEL_WIDTH), Sizing.content());
        panel.gap(SECTION_GAP);
        panel.padding(Insets.of(12));
        panel.surface(Surface.flat(PANEL_BACKGROUND).and(Surface.outline(PANEL_BORDER)));

        panel.child(
            Components.label(Text.literal("修 仙 面 板"))
                .shadow(true)
                .color(Color.ofRgb(TITLE_COLOR))
                .horizontalTextAlignment(HorizontalAlignment.CENTER)
                .sizing(Sizing.fill(100), Sizing.content())
        );
        panel.child(buildDivider());
        panel.child(buildStatLine("境界", this.model.realmLabel()));
        panel.child(buildBarSection("真元", this.model.spiritQiText(), this.model.spiritQiRatio(), QI_FILL));
        panel.child(buildKarmaSection());
        panel.child(buildStatLine("综合实力", this.model.compositePowerText()));
        panel.child(buildBreakdownSection());
        panel.child(buildStatLine("当前区域", this.model.zoneText()));
        panel.child(
            Components.label(Text.literal(this.model.footerText()))
                .color(Color.ofRgb(MUTED_TEXT))
                .maxWidth(BAR_WIDTH)
        );

        rootComponent.child(panel);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    private FlowLayout buildStatLine(String label, String value) {
        FlowLayout line = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        line.gap(2);
        line.child(
            Components.label(Text.literal(label + ": " + value))
                .color(Color.ofRgb(PRIMARY_TEXT))
                .maxWidth(BAR_WIDTH)
        );
        return line;
    }

    private FlowLayout buildBarSection(String label, String value, double ratio, int fillColor) {
        FlowLayout section = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        section.gap(3);
        section.child(
            Components.label(Text.literal(label + ": " + value))
                .color(Color.ofRgb(PRIMARY_TEXT))
                .maxWidth(BAR_WIDTH)
        );
        section.child(buildProgressBar(ratio, BAR_HEIGHT, fillColor));
        return section;
    }

    private FlowLayout buildKarmaSection() {
        FlowLayout section = buildBarSection("因果", this.model.karmaText(), this.model.karmaRatio(), KARMA_FILL);
        section.child(
            Components.label(Text.literal("善 ──────────── 恶"))
                .color(Color.ofRgb(MUTED_TEXT))
                .maxWidth(BAR_WIDTH)
        );
        return section;
    }

    private FlowLayout buildBreakdownSection() {
        FlowLayout section = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        section.gap(4);
        section.child(
            Components.label(Text.literal("实力分解"))
                .color(Color.ofRgb(TITLE_COLOR))
        );

        for (CultivationScreenModel.PowerEntry entry : this.model.breakdownEntries()) {
            section.child(buildCompactBarSection(entry.label(), entry.valueText(), entry.ratio()));
        }

        return section;
    }

    private FlowLayout buildCompactBarSection(String label, String value, double ratio) {
        FlowLayout section = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        section.gap(2);
        section.child(
            Components.label(Text.literal("• " + label + ": " + value))
                .color(Color.ofRgb(PRIMARY_TEXT))
                .maxWidth(BAR_WIDTH)
        );
        section.child(buildProgressBar(ratio, 6, POWER_FILL));
        return section;
    }

    private StackLayout buildProgressBar(double ratio, int height, int fillColor) {
        StackLayout bar = Containers.stack(Sizing.fixed(BAR_WIDTH), Sizing.fixed(height));
        bar.child(
            Components.box(Sizing.fixed(BAR_WIDTH), Sizing.fixed(height))
                .fill(true)
                .color(Color.ofArgb(TRACK_FILL))
        );

        int fillWidth = (int) Math.round(BAR_WIDTH * Math.max(0.0, Math.min(1.0, ratio)));
        if (fillWidth > 0) {
            bar.child(
                Components.box(Sizing.fixed(fillWidth), Sizing.fixed(height))
                    .fill(true)
                    .color(Color.ofArgb(fillColor))
                    .positioning(Positioning.absolute(0, 0))
            );
        }

        bar.child(
            Components.box(Sizing.fixed(BAR_WIDTH), Sizing.fixed(height))
                .color(Color.ofArgb(TRACK_OUTLINE))
                .positioning(Positioning.absolute(0, 0))
        );
        return bar;
    }

    private FlowLayout buildDivider() {
        FlowLayout divider = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        divider.child(
            Components.box(Sizing.fixed(BAR_WIDTH), Sizing.fixed(1))
                .fill(true)
                .color(Color.ofArgb(DIVIDER))
        );
        return divider;
    }
}
