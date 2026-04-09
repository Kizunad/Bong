package com.bong.client;

import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

public class CultivationScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text SCREEN_TITLE = Text.literal("修仙面板");

    private final PlayerStateViewModel viewModel;

    public CultivationScreen() {
        this(PlayerStateViewModel.fromCurrentState());
    }

    CultivationScreen(PlayerStateViewModel viewModel) {
        this.viewModel = Objects.requireNonNull(viewModel, "viewModel");
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout rootComponent) {
        rootComponent.surface(Surface.VANILLA_TRANSLUCENT);
        rootComponent.horizontalAlignment(HorizontalAlignment.CENTER);
        rootComponent.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.content(), Sizing.content());
        panel.padding(Insets.of(12));
        panel.gap(2);
        panel.surface(Surface.DARK_PANEL);
        panel.child(Components.label(SCREEN_TITLE));

        for (String line : contentLines(viewModel)) {
            panel.child(Components.label(Text.literal(line)));
        }

        rootComponent.child(panel);
    }

    static List<String> contentLines(PlayerStateViewModel viewModel) {
        Objects.requireNonNull(viewModel, "viewModel");

        List<String> lines = new ArrayList<>();
        lines.add("境界: " + viewModel.realmLabel());
        if (!viewModel.hasState()) {
            lines.add("状态: " + viewModel.statusText());
        }
        lines.add("真元: " + viewModel.spiritQiBar() + " " + viewModel.spiritQiLabel());
        lines.add("因果 (karma): " + viewModel.karmaLabel());
        lines.add("因果天平: " + viewModel.karmaAxis());
        lines.add("综合实力: " + viewModel.compositePowerLabel());

        List<PlayerStateViewModel.PowerBreakdownRow> powerBreakdown = viewModel.powerBreakdown();
        for (int index = 0; index < powerBreakdown.size(); index++) {
            PlayerStateViewModel.PowerBreakdownRow row = powerBreakdown.get(index);
            String prefix = index == powerBreakdown.size() - 1 ? "└ " : "├ ";
            lines.add(prefix + row.label() + ": " + row.barText() + " " + row.valueLabel());
        }

        lines.add("当前区域: " + viewModel.zoneLabel());
        lines.add("动态 XML UI: " + viewModel.dynamicXmlUiLabel());
        lines.add("界面模式: 只读本地状态");
        return List.copyOf(lines);
    }
}
