package com.bong.client.ui;

import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.util.RealmLabel;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class CultivationScreen extends BaseOwoScreen<FlowLayout> {
    static final Text TITLE = Text.literal("修仙面板");

    private static final int PANEL_PADDING = 10;
    private static final int BAR_SEGMENTS = 10;
    private static final int KARMA_METER_SLOTS = 12;

    private final PlayerStateViewModel playerState;

    public CultivationScreen(PlayerStateViewModel playerState) {
        super(TITLE);
        this.playerState = playerState == null ? PlayerStateViewModel.empty() : playerState;
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
        panel.surface(Surface.DARK_PANEL);
        panel.padding(Insets.of(PANEL_PADDING));
        panel.child(Components.label(TITLE));

        for (String line : describe(playerState).lines()) {
            panel.child(Components.label(Text.literal(line)));
        }

        rootComponent.child(panel);
    }

    PlayerStateViewModel playerState() {
        return playerState;
    }

    static RenderContent describe(PlayerStateViewModel playerState) {
        PlayerStateViewModel safePlayerState = playerState == null ? PlayerStateViewModel.empty() : playerState;
        if (safePlayerState.isEmpty()) {
            return new RenderContent(true, List.of(
                "当前尚未同步修仙数据",
                "请等待 server 下发 player_state。"
            ));
        }

        List<String> lines = new ArrayList<>();
        lines.add("境界: " + RealmLabel.displayName(safePlayerState.realm()));
        lines.add(
            "真元: "
                + buildBar(safePlayerState.spiritQiFillRatio())
                + " "
                + formatQuantity(safePlayerState.spiritQiCurrent())
                + "/"
                + formatQuantity(safePlayerState.spiritQiMax())
        );
        lines.add("因果 (karma): " + formatSigned(safePlayerState.karma()));
        lines.add("善恶刻度: " + buildKarmaMeter(safePlayerState.karma()));
        lines.add("综合实力: " + formatNormalized(safePlayerState.compositePower()));
        lines.add("战斗: " + formatNormalized(safePlayerState.breakdown().combat()));
        lines.add("财富: " + formatNormalized(safePlayerState.breakdown().wealth()));
        lines.add("社交: " + formatNormalized(safePlayerState.breakdown().social()));
        lines.add("领地: " + formatNormalized(safePlayerState.breakdown().territory()));
        PlayerStateViewModel.SocialSnapshot social = safePlayerState.social();
        lines.add("声名: fame " + formatSignedInteger(social.fame()) + " / notoriety " + formatSignedInteger(social.notoriety()));
        lines.add("声名标签: " + formatTags(social.topTags()));
        lines.add("派系挂靠: " + formatFaction(social));
        lines.add("当前区域: " + safePlayerState.zoneLabel());
        lines.add(
            "灵气浓度: "
                + buildBar(safePlayerState.zoneSpiritQiNormalized())
                + " "
                + formatPercent(safePlayerState.zoneSpiritQiNormalized())
        );
        if (safePlayerState.localNegPressure() < 0.0) {
            lines.add("局部灵压: " + formatSigned(safePlayerState.localNegPressure()));
        }

        return new RenderContent(false, lines);
    }

    private static String buildBar(double ratio) {
        double clamped = clamp(ratio, 0.0, 1.0);
        int filledSegments = (int) Math.round(clamped * BAR_SEGMENTS);
        StringBuilder builder = new StringBuilder(BAR_SEGMENTS);
        for (int index = 0; index < BAR_SEGMENTS; index++) {
            builder.append(index < filledSegments ? '█' : '░');
        }
        return builder.toString();
    }

    private static String buildKarmaMeter(double karma) {
        double clamped = clamp(karma, -1.0, 1.0);
        int markerIndex = (int) Math.round((1.0 - ((clamped + 1.0) / 2.0)) * KARMA_METER_SLOTS);
        markerIndex = Math.max(0, Math.min(KARMA_METER_SLOTS - 1, markerIndex));

        StringBuilder builder = new StringBuilder(KARMA_METER_SLOTS + 10);
        builder.append('[');
        for (int index = 0; index < KARMA_METER_SLOTS; index++) {
            builder.append(index == markerIndex ? '●' : '═');
        }
        builder.append("] 善 ←→ 恶");
        return builder.toString();
    }

    private static String formatSigned(double value) {
        return String.format(Locale.ROOT, "%+.2f", value);
    }

    private static String formatSignedInteger(int value) {
        return String.format(Locale.ROOT, "%+d", value);
    }

    private static String formatNormalized(double value) {
        return String.format(Locale.ROOT, "%.2f", value);
    }

    private static String formatPercent(double value) {
        return Math.round(clamp(value, 0.0, 1.0) * 100.0) + "%";
    }

    private static String formatQuantity(double value) {
        double rounded = Math.rint(value);
        if (Math.abs(value - rounded) < 0.0001) {
            return Long.toString((long) rounded);
        }

        return String.format(Locale.ROOT, "%.2f", value);
    }

    private static String formatTags(List<String> tags) {
        if (tags == null || tags.isEmpty()) {
            return "无";
        }
        return String.join(" / ", tags);
    }

    private static String formatFaction(PlayerStateViewModel.SocialSnapshot social) {
        if (social == null || !social.hasFaction()) {
            return "无";
        }
        return social.faction()
            + " rank "
            + social.factionRank()
            + " / loyalty "
            + formatSignedInteger(social.factionLoyalty())
            + " / betrayals "
            + social.factionBetrayalCount();
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }

    static record RenderContent(boolean placeholder, List<String> lines) {
        RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
