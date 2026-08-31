package com.bong.client.combat.screen;

import com.bong.client.combat.DeathIntent;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.death.DeathCinematicRenderer;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
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

    private final DeathStateStore.State state;
    private final UiIntentSink<DeathIntent> intentSink;
    private LabelComponent titleLabel;
    private LabelComponent luckLabel;
    private LabelComponent phaseLabel;
    private LabelComponent countdownLabel;
    private LabelComponent lifespanLabel;
    private LabelComponent finalWordsLabel;
    private LabelComponent feedbackLabel;
    private FlowLayout luckFill;
    private String feedbackText = "";
    private long lastRenderMs;

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

        ButtonComponent reincarnate = component(ButtonComponent.class, "death-reincarnate")
            .onPress(button -> dispatch(new DeathIntent.Reincarnate()));
        ButtonComponent terminate = component(ButtonComponent.class, "death-terminate")
            .onPress(button -> dispatch(new DeathIntent.Terminate()));
        if (!state.hasLifespanPreview()) {
            lifespanLabel.remove();
            lifespanLabel = null;
        }
        // 不可用动作从已挂载树移除，保持旧屏“不可用按钮不出现”的行为。
        if (!state.canReincarnate()) {
            reincarnate.remove();
        }
        if (!state.canTerminate()) {
            terminate.remove();
        }
        refreshBindings(System.currentTimeMillis());
    }

    @Override
    public void tick() {
        super.tick();
        if (!DeathStateStore.snapshot().visible()) {
            close();
        }
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        long now = System.currentTimeMillis();
        lastRenderMs = now;
        context.fill(0, 0, width, height, BG_COLOR);
        refreshBindings(now);
        super.render(context, mouseX, mouseY, delta);
        renderCinematicCommands(
            context,
            DeathCinematicRenderer.buildCommands(state.cinematic(), now, width, height)
        );
    }

    private void refreshBindings(long nowMs) {
        if (titleLabel == null) {
            return;
        }
        titleLabel.text(Text.literal("道陨——" + causeLabel(state.cause())));
        luckLabel.text(Text.literal("重生概率: " + Math.round(state.luckRemaining() * 100) + "%"));
        phaseLabel.text(Text.literal(formatPhaseLine()));
        countdownLabel.text(Text.literal("倒计时: " + (state.remainingMs(nowMs) / 1000) + "s"));
        if (lifespanLabel != null) {
            lifespanLabel.text(Text.literal(formatLifespan()));
        }
        finalWordsLabel.text(Text.literal(formatFinalWords(state.finalWords())));
        feedbackLabel.text(Text.literal(feedbackText));
        luckFill.horizontalSizing(Sizing.fixed(Math.round(state.luckRemaining() * LUCK_BAR_WIDTH)));
        luckFill.surface(Surface.flat(luckFillColor()));
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

    private void dispatch(DeathIntent intent) {
        UiIntentResult result = intentSink.dispatch(intent);
        if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) {
            feedbackText = "操作未提交: " + result.reason();
            if (feedbackLabel != null) {
                feedbackLabel.text(Text.literal(feedbackText));
            }
        }
    }

    private int luckFillColor() {
        return state.luckRemaining() < 0.3f
            ? 0xFFE04040
            : state.luckRemaining() < 0.7f ? LUCK_FILL_COLOR : 0xFF60D060;
    }

    private void renderCinematicCommands(DrawContext context, List<HudRenderCommand> commands) {
        for (HudRenderCommand command : commands) {
            if (command.isScreenTint()) {
                context.fill(0, 0, width, height, command.color());
                continue;
            }
            if (command.isEdgeVignette()) {
                renderEdgeVignette(context, command.color());
                continue;
            }
            if (command.isRect()) {
                context.fill(command.x(), command.y(), command.x() + command.width(), command.y() + command.height(), command.color());
                continue;
            }
            if (command.isText()) {
                context.drawTextWithShadow(this.textRenderer, command.text(), command.x(), command.y(), command.color());
                continue;
            }
            if (command.isScaledText()) {
                var matrices = context.getMatrices();
                matrices.push();
                matrices.translate(command.x(), command.y(), 0);
                float scale = (float) command.textScale();
                matrices.scale(scale, scale, 1.0f);
                context.drawTextWithShadow(this.textRenderer, command.text(), 0, 0, command.color());
                matrices.pop();
            }
        }
    }

    private void renderEdgeVignette(DrawContext context, int color) {
        int edge = Math.max(12, Math.min(width, height) / 8);
        context.fill(0, 0, width, edge, color);
        context.fill(0, height - edge, width, height, color);
        context.fill(0, edge, edge, height - edge, color);
        context.fill(width - edge, edge, width, height - edge, color);
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
