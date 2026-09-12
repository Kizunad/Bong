package com.bong.client.combat.screen;

import com.bong.client.combat.DeathIntent;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.death.DeathBackdrop;
import com.bong.client.death.DeathDiceRenderer;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;

import java.util.List;
import java.util.Locale;
import java.util.Objects;
import java.util.stream.Collectors;

/**
 * 死亡全屏覆盖层（plan §U3 / §2.3）。XML 持有布局，Java 只绑定快照、typed
 * action 和无法声明式表达的 cinematic 渲染桥接。
 */
public final class DeathScreen extends OwoXmlScreenHost<FlowLayout> {
    public static final int BG_COLOR = 0xE0000000;
    public static final int TITLE_COLOR = 0xFFFF4040;
    public static final int TEXT_COLOR = 0xFFDDDDDD;
    public static final int LUCK_FILL_COLOR = 0xFFE0C040;
    public static final int LUCK_TRACK_COLOR = 0xFF303030;

    private static final int LUCK_BAR_WIDTH = 220;

    private DeathStateStore.State state;
    private final UiIntentSink<DeathIntent> intentSink;
    private LabelComponent titleLabel;
    private LabelComponent luckLabel;
    private LabelComponent phaseLabel;
    private LabelComponent countdownLabel;
    private LabelComponent lifespanLabel;
    private LabelComponent finalWordsLabel;
    private LabelComponent feedbackLabel;
    private FlowLayout luckFill;
    private ButtonComponent reincarnateButton;
    private ButtonComponent terminateButton;
    private String feedbackText = "";
    private long lastRenderMs;
    private long pendingSinceMs;

    public DeathScreen(DeathStateStore.State state, UiIntentSink<DeathIntent> intentSink) {
        super(Text.literal("死亡"), FlowLayout.class, "death");
        this.state = state == null ? DeathStateStore.State.HIDDEN : state;
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public boolean shouldCloseOnEsc() {
        return false;
    }

    /** XML 负责组件树；这里只登记动态快照和 typed button callback。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        titleLabel = label("death-title");
        luckLabel = label("death-luck");
        phaseLabel = label("death-phase");
        countdownLabel = label("death-countdown");
        lifespanLabel = label("death-lifespan");
        finalWordsLabel = label("death-final-words");
        feedbackLabel = label("death-feedback");
        luckFill = component(FlowLayout.class, "death-luck-fill");

        reincarnateButton = component(ButtonComponent.class, "death-reincarnate")
            .onPress(button -> dispatch(new DeathIntent.Reincarnate()));
        terminateButton = component(ButtonComponent.class, "death-terminate")
            .onPress(button -> dispatch(new DeathIntent.Terminate()));
        DeathBackdrop.command(reincarnateButton, DeathBackdrop.REBIRTH_COLOR);
        DeathBackdrop.command(terminateButton, DeathBackdrop.ENDING_COLOR);
        refreshBindings(System.currentTimeMillis());
    }

    @Override
    public void init() {
        super.init();
        if (!hostReadyForTests()) return;
        component(FlowLayout.class, "death-panel").horizontalSizing(Sizing.fixed(Math.min(400, width - 32)));
        int diceHeight = Math.min(190, Math.max(40, Math.min(height * 4 / 9, height - 180)));
        component(FlowLayout.class, "death-dice").verticalSizing(Sizing.fixed(diceHeight));
        component(ScrollContainer.class, "death-content-scroll").verticalSizing(Sizing.fixed(Math.max(16, height - 164 - diceHeight)));
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        long now = System.currentTimeMillis();
        lastRenderMs = now;
        DeathBackdrop.render(context, width, height, DeathBackdrop.ENDING_COLOR);
        refreshBindings(now);
        super.render(context, mouseX, mouseY, delta);
        if (hostReadyForTests()) {
            var title = componentBoundsForPreview("death-title");
            DeathBackdrop.title(context, "一息未尽", (int) title.centerX(), title.y());
            var dice = componentBoundsForPreview("death-dice");
            DeathDiceRenderer.render(context, state, now, dice.x(), dice.y(), dice.width(), dice.height());
        }
    }

    private void refreshBindings(long nowMs) {
        if (titleLabel == null) {
            return;
        }
        if (DeathStateStore.snapshot().visible()) state = DeathStateStore.snapshot();
        boolean rolling = DeathDiceRenderer.rolling(state);
        if (pendingSinceMs != 0 && nowMs - pendingSinceMs >= 5_000) {
            pendingSinceMs = 0;
            feedbackText = "尚未收到裁决，可重试";
        }
        label("death-cause").text(Text.literal(causeLabel(state.cause())));
        luckLabel.text(Text.literal("再续此身的运数  " + Math.round(state.luckRemaining() * 100) + "%"));
        phaseLabel.text(Text.literal(formatPhaseLine()));
        countdownLabel.text(Text.literal(rolling ? "运数已掷" : "余 " + (state.remainingMs(nowMs) / 1000) + " 息"));
        if (lifespanLabel != null) {
            lifespanLabel.text(Text.literal(formatLifespan()));
        }
        finalWordsLabel.text(Text.literal(formatFinalWords(state.finalWords())));
        feedbackLabel.text(Text.literal(rolling ? "" : feedbackText));
        luckFill.horizontalSizing(Sizing.fixed(Math.round(state.luckRemaining() * LUCK_BAR_WIDTH)));
        luckFill.surface(Surface.flat(luckFillColor()));
        reincarnateButton.active(state.canReincarnate() && pendingSinceMs == 0);
        terminateButton.active(state.canTerminate() && pendingSinceMs == 0);
        reincarnateButton.setMessage(Text.literal(rolling ? "听候落定" : "掷下运数").styled(style -> style.withColor(DeathBackdrop.REBIRTH_COLOR)));
        terminateButton.setMessage(Text.literal(rolling ? "运数已掷" : state.canTerminate() ? "终结此生" : "运数未尽")
            .styled(style -> style.withColor(state.canTerminate() ? DeathBackdrop.ENDING_COLOR : 0x75817A)));
    }

    private String formatPhaseLine() {
        String phase = phaseLabel(state.stage());
        String zone = zoneLabel(state.zoneKind());
        String deathNo = state.deathNumber() > 0 ? " · 第" + state.deathNumber() + "死" : "";
        return phase + deathNo + (zone.isEmpty() ? "" : " · " + zone);
    }

    private String formatLifespan() {
        if (!state.hasLifespanPreview()) {
            return "";
        }
        return String.format(
            Locale.ROOT,
            "寿元 %.1f/%d · 余%.1f · 本死扣%d · 流速×%.1f%s",
            state.yearsLived(), state.lifespanCapByRealm(), state.remainingYears(),
            state.deathPenaltyYears(), state.lifespanTickRateMultiplier(),
            state.windCandle() ? " · 风烛" : ""
        );
    }

    void dispatch(DeathIntent intent) {
        if (pendingSinceMs != 0) return;
        UiIntentResult result = intentSink.dispatch(intent);
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            pendingSinceMs = System.currentTimeMillis();
            feedbackText = "已提交，等待裁决";
            if (feedbackLabel != null) {
                feedbackLabel.text(Text.literal(feedbackText));
            }
            return;
        }
        feedbackText = "操作未提交: " + result.reason();
        if (feedbackLabel != null) {
            feedbackLabel.text(Text.literal(feedbackText));
        }
    }

    private int luckFillColor() {
        return state.luckRemaining() < 0.3f
            ? DeathBackdrop.ENDING_COLOR
            : DeathBackdrop.REBIRTH_COLOR;
    }

    static String formatFinalWords(List<String> words) {
        if (words == null || words.isEmpty()) {
            return "";
        }
        return words.stream()
            .filter(word -> word != null && !word.isBlank())
            .limit(6)
            .map(word -> "「" + word + "」")
            .collect(Collectors.joining("\n"));
    }

    private static String causeLabel(String cause) {
        return switch (cause == null ? "" : cause) {
            case "pk" -> "死于PK";
            case "tribulation" -> "死于天劫";
            case "dao_heart_shatter" -> "道心崩塌";
            case "starvation" -> "饿死";
            case "cultivation:SwarmQiDrain" -> "真元遭啮，气息断绝";
            case "cultivation:DevCommand" -> "气息断绝";
            default -> cause == null || cause.isBlank() ? "未知" : cause;
        };
    }

    private static String phaseLabel(String stage) {
        return switch (stage == null ? "" : stage) {
            case "fortune" -> "运数期";
            case "tribulation" -> "劫数期";
            default -> "重生判定";
        };
    }

    private static String zoneLabel(String zoneKind) {
        return switch (zoneKind == null ? "" : zoneKind) {
            case "death" -> "死域：跳过运数";
            case "negative" -> "负灵域：跳过运数";
            default -> "";
        };
    }

    long lastRenderForTests() {
        return lastRenderMs;
    }

    public DeathStateStore.State stateForTests() {
        return state;
    }

    String feedbackTextForTests() {
        return feedbackText;
    }
}
