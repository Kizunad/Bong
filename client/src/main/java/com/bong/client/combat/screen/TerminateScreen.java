package com.bong.client.combat.screen;

import com.bong.client.combat.TerminateClientIntentSink;
import com.bong.client.combat.TerminateIntent;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.death.DeathBackdrop;
import com.bong.client.death.OtherworldVerdict;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;

import java.util.Objects;
import java.util.Locale;
import java.util.stream.Collectors;

/**
 * Final termination overlay (plan §U4). Displays final words + epilogue + a
 * "create new character" button.
 */
public final class TerminateScreen extends OwoXmlScreenHost<FlowLayout> {
    private final TerminateStateStore.State state;
    private final UiIntentSink<TerminateIntent> intentSink;
    private final OtherworldVerdict verdict;
    private ButtonComponent createButton;
    private long pendingSinceMs;

    public TerminateScreen(TerminateStateStore.State state) {
        this(state, TerminateClientIntentSink.production());
    }

    TerminateScreen(
        TerminateStateStore.State state,
        UiIntentSink<TerminateIntent> intentSink
    ) {
        super(Text.literal("\u7ec8\u7ed3"), FlowLayout.class, "terminate");
        this.state = state == null ? TerminateStateStore.State.HIDDEN : state;
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
        verdict = OtherworldVerdict.choose(this.state.summary(), this.state.hashCode());
    }

    @Override
    public boolean shouldPause() { return false; }

    @Override
    public boolean shouldCloseOnEsc() { return false; }

    /** XML 负责布局；这里仅绑定服务端快照和 typed button callback。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        label("terminate-title");
        label("terminate-final-words").text(Text.literal(formatFinalWords(state.finalWords())));
        label("terminate-epilogue").text(Text.literal(state.epilogue()));
        label("terminate-voice").text(Text.literal(verdict.voice()));
        label("terminate-verdict").text(Text.literal(verdict.words()));
        var summary = state.summary();
        label("terminate-summary-left").text(Text.literal(
            "旧名  " + (summary.characterName().isBlank() ? "未留名" : summary.characterName())
                + "\n止境  " + realmName(summary.realm())
                + "\n历死  " + summary.deathCount() + " 次"
                + "\n行年  " + number(summary.yearsLived())));
        label("terminate-summary-right").text(Text.literal(
            "经脉  " + number(summary.meridiansOpen())
                + "\n功法  " + number(summary.techniquesLearned())
                + "\n真元上限  " + number(summary.qiMax())
                + "\n体魄上限  " + number(summary.healthMax())));
        createButton = component(ButtonComponent.class, "terminate-create-character").onPress(button -> createCharacter());
        createButton.setMessage(Text.literal("另启一生").styled(style -> style.withColor(DeathBackdrop.REBIRTH_COLOR)));
        DeathBackdrop.command(createButton, DeathBackdrop.REBIRTH_COLOR);
    }

    @Override
    public void init() {
        super.init();
        if (!hostReadyForTests()) return;
        component(FlowLayout.class, "terminate-panel").horizontalSizing(Sizing.fixed(Math.min(400, width - 32)));
        component(ScrollContainer.class, "terminate-content-scroll").verticalSizing(Sizing.fixed(Math.max(30, height - 166)));
    }

    private void createCharacter() {
        if (pendingSinceMs != 0) return;
        var result = intentSink.dispatch(new TerminateIntent.CreateNewCharacter());
        if (result.kind() == com.bong.client.ui.intent.UiIntentResult.Kind.LOCAL_ACCEPTED) {
            pendingSinceMs = System.currentTimeMillis();
            createButton.active(false);
            label("terminate-feedback").text(Text.literal("已提交，等待入世"));
        } else {
            label("terminate-feedback").text(Text.literal("未能入世，请重试"));
        }
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        DeathBackdrop.render(context, width, height, verdict.accent());
        if (pendingSinceMs != 0 && System.currentTimeMillis() - pendingSinceMs >= 5_000) {
            pendingSinceMs = 0;
            createButton.active(true);
            label("terminate-feedback").text(Text.literal("尚未收到回应，可重试"));
        }
        super.render(context, mouseX, mouseY, delta);
        if (hostReadyForTests()) {
            var title = componentBoundsForPreview("terminate-title");
            DeathBackdrop.title(context, "终焉之言", (int) title.centerX(), title.y());
        }
    }

    private static String number(Number value) {
        if (value == null) return "未留记录";
        double number = value.doubleValue();
        return String.format(Locale.ROOT, number == Math.rint(number) ? "%.0f" : "%.1f", number);
    }

    private static String realmName(String realm) {
        return switch (realm) {
            case "Awaken" -> "醒灵";
            case "Induce" -> "引气";
            case "Condense" -> "凝脉";
            case "Solidify" -> "固元";
            case "Spirit" -> "通灵";
            case "Void" -> "化虚";
            default -> "未留记录";
        };
    }

    static String formatFinalWords(String raw) {
        if (raw == null || raw.isBlank()) {
            return "";
        }
        return raw.lines()
            .filter(line -> !line.isBlank())
            .collect(Collectors.joining("\n"));
    }

    static String formatArchetype(String raw) {
        return raw == null || raw.isBlank() ? "" : "\u5efa\u8bae\u65b0\u89d2\u8272\u539f\u578b: " + raw;
    }

    TerminateStateStore.State stateForTests() { return state; }
}
