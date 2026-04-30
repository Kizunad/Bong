package com.bong.client.insight;

import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
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
import net.minecraft.util.Formatting;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Objects;
import java.util.function.Consumer;
import java.util.function.LongSupplier;

/**
 * 顿悟邀约弹窗——展示 trigger 上下文 + 2-3 个候选 + "心未契机" 拒绝按钮 + 倒计时。
 *
 * <p>关闭方式：
 * <ul>
 *   <li>点击候选卡 → 提交 CHOSEN，关闭。</li>
 *   <li>点击底部"心未契机" → 提交 DECLINED，关闭。</li>
 *   <li>倒计时归零 (tick 检测) → 提交 TIMED_OUT，关闭。</li>
 *   <li>ESC：和"心未契机"等价。</li>
 * </ul>
 */
public final class InsightOfferScreen extends BaseOwoScreen<FlowLayout> {
    static final Text TITLE = Text.literal("◇ 心 有 所 感 ◇");
    static final String HEART_DEMON_TRIGGER_PREFIX = "heart_demon:";

    private static final int CARD_WIDTH = 150;
    private static final int CARD_HEIGHT = 200;
    private static final int CARD_PADDING = 8;
    private static final int CARD_GAP = 8;
    private static final int OUTER_PADDING = 14;
    private static final int CARDS_MAX = 4;

    private static final int COLOR_TITLE = 0xFFE9D9A6;
    private static final int COLOR_TRIGGER = 0xFFCCCCCC;
    private static final int COLOR_META = 0xFFAAAAAA;
    private static final int COLOR_TIMER_OK = 0xFF8FE38F;
    private static final int COLOR_TIMER_LOW = 0xFFE57373;
    private static final int COLOR_FLAVOR = 0xFFE0D4B8;
    private static final int COLOR_EFFECT = 0xFFB7CDE5;
    private static final int COLOR_HINT = 0xFFB0A37C;
    private static final int COLOR_DECLINE = 0xFFAAAAAA;

    private final InsightOfferViewModel offer;
    private final Consumer<InsightDecision> onDecision;
    private final LongSupplier clock;

    private FlowLayout timerLabelHolder;
    private FlowLayout headerHolder;
    private boolean settled;

    public InsightOfferScreen(InsightOfferViewModel offer) {
        this(offer, InsightOfferStore::submit, System::currentTimeMillis);
    }

    InsightOfferScreen(InsightOfferViewModel offer,
                       Consumer<InsightDecision> onDecision,
                       LongSupplier clock) {
        super(TITLE);
        this.offer = Objects.requireNonNull(offer, "offer");
        this.onDecision = Objects.requireNonNull(onDecision, "onDecision");
        this.clock = Objects.requireNonNull(clock, "clock");
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.content(), Sizing.content());
        panel.surface(Surface.DARK_PANEL);
        panel.padding(Insets.of(OUTER_PADDING));
        panel.gap(6);
        panel.horizontalAlignment(HorizontalAlignment.CENTER);

        // ── 顶部：标题 + trigger label ──
        panel.child(coloredLabel(screenTitle(offer), COLOR_TITLE));
        panel.child(coloredLabel("【触发】" + offer.triggerLabel(), COLOR_TRIGGER));

        // ── 元信息 ──
        headerHolder = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        headerHolder.gap(14);
        headerHolder.child(coloredLabel("境界: " + offer.realmLabel(), COLOR_META));
        headerHolder.child(coloredLabel(
            String.format(Locale.ROOT, "心境: %.2f", offer.composure()), COLOR_META));
        headerHolder.child(coloredLabel(metaQuotaLabel(offer), COLOR_META));
        panel.child(headerHolder);

        // ── 倒计时 ──
        timerLabelHolder = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        timerLabelHolder.child(buildTimerLabel(remainingSeconds()));
        panel.child(timerLabelHolder);

        panel.child(divider(CARD_WIDTH * Math.min(offer.choices().size(), CARDS_MAX)
            + CARD_GAP * (Math.min(offer.choices().size(), CARDS_MAX) - 1)));

        // ── 候选卡片 (横排) ──
        FlowLayout cards = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        cards.gap(CARD_GAP);
        for (InsightChoice choice : offer.choices()) {
            cards.child(buildCard(choice));
        }
        panel.child(cards);

        // ── 底部：拒绝按钮 ──
        FlowLayout declineRow = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        declineRow.padding(Insets.top(8));
        declineRow.child(buildDeclineButton());
        panel.child(declineRow);

        root.child(panel);
    }

    private FlowLayout buildCard(InsightChoice choice) {
        FlowLayout card = Containers.verticalFlow(Sizing.fixed(CARD_WIDTH), Sizing.fixed(CARD_HEIGHT));
        card.surface(Surface.PANEL_INSET.and(Surface.outline(choice.category().accentArgb())));
        card.padding(Insets.of(CARD_PADDING));
        card.gap(4);
        card.horizontalAlignment(HorizontalAlignment.LEFT);

        // 类别标签 (彩色)
        card.child(coloredLabel(choice.category().label(), choice.category().accentArgb()));
        // 标题 (大字)
        card.child(Components.label(Text.literal(choice.title())
            .formatted(Formatting.BOLD)).color(Color.ofArgb(COLOR_TITLE)));
        // 数值描述
        card.child(wrapped(choice.effectSummary(), COLOR_EFFECT, CARD_WIDTH - CARD_PADDING * 2));
        // 灰色分隔符
        card.child(divider(CARD_WIDTH - CARD_PADDING * 2));
        // ✦ flavor
        card.child(wrapped("✦ " + choice.flavor(), COLOR_FLAVOR, CARD_WIDTH - CARD_PADDING * 2));
        // 风格提示
        if (!choice.styleHint().isEmpty()) {
            card.child(Components.label(Text.literal(""))); // spacer
            card.child(wrapped("→ " + choice.styleHint(), COLOR_HINT, CARD_WIDTH - CARD_PADDING * 2));
        }

        card.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                settle(InsightDecision.chosen(offer.triggerId(), choice.choiceId()));
                return true;
            }
            return false;
        });

        return card;
    }

    private FlowLayout buildDeclineButton() {
        FlowLayout box = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        box.surface(Surface.PANEL_INSET);
        box.padding(Insets.of(6, 6, 14, 14));
        box.child(Components.label(Text.literal(declineLabel(offer))
            .formatted(Formatting.ITALIC))
            .color(Color.ofArgb(COLOR_DECLINE)));
        box.mouseDown().subscribe((mouseX, mouseY, button) -> {
            if (button == 0) {
                settle(InsightDecision.declined(offer.triggerId()));
                return true;
            }
            return false;
        });
        return box;
    }

    private FlowLayout divider(int width) {
        FlowLayout line = Containers.horizontalFlow(Sizing.fixed(width), Sizing.fixed(1));
        line.surface(Surface.flat(0x44FFFFFF));
        return line;
    }

    private static io.wispforest.owo.ui.component.LabelComponent coloredLabel(String text, int argb) {
        return Components.label(Text.literal(text)).color(Color.ofArgb(argb));
    }

    private io.wispforest.owo.ui.component.LabelComponent wrapped(String text, int argb, int maxPx) {
        return Components.label(Text.literal(text))
            .color(Color.ofArgb(argb))
            .maxWidth(maxPx);
    }

    private io.wispforest.owo.ui.component.LabelComponent buildTimerLabel(long secondsLeft) {
        int color = secondsLeft <= 10 ? COLOR_TIMER_LOW : COLOR_TIMER_OK;
        String text = timerLabel(offer, secondsLeft);
        return Components.label(Text.literal(text)).color(Color.ofArgb(color));
    }

    @Override
    public void tick() {
        super.tick();
        if (settled) {
            return;
        }
        long now = clock.getAsLong();
        if (offer.isExpired(now)) {
            settle(InsightDecision.timedOut(offer.triggerId()));
            return;
        }
        if (timerLabelHolder != null) {
            timerLabelHolder.clearChildren();
            timerLabelHolder.child(buildTimerLabel(Math.max(0L, offer.remainingMillis(now) / 1000L)));
        }
    }

    @Override
    public void close() {
        if (!settled) {
            // 任何未结算的关闭都按 declined 处理 (e.g. ESC)
            settle(InsightDecision.declined(offer.triggerId()));
            return;
        }
        super.close();
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    private void settle(InsightDecision decision) {
        if (settled) {
            return;
        }
        settled = true;
        try {
            onDecision.accept(decision);
        } finally {
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc != null && mc.currentScreen == this) {
                mc.setScreen(null);
            }
        }
    }

    private long remainingSeconds() {
        return Math.max(0L, offer.remainingMillis(clock.getAsLong()) / 1000L);
    }

    /** 测试用：纯函数式渲染描述，便于 assert 关键文本。 */
    public static RenderContent describe(InsightOfferViewModel offer) {
        Objects.requireNonNull(offer, "offer");
        List<String> lines = new ArrayList<>();
        lines.add(screenTitle(offer));
        lines.add("【触发】" + offer.triggerLabel());
        lines.add("境界: " + offer.realmLabel());
        lines.add(String.format(Locale.ROOT, "心境: %.2f", offer.composure()));
        lines.add(metaQuotaLabel(offer));
        lines.add(timerLabel(offer, Math.max(0L, offer.remainingMillis(System.currentTimeMillis()) / 1000L)));
        for (InsightChoice c : offer.choices()) {
            lines.add("[" + c.category().code() + "] " + c.title());
            lines.add("    效果: " + c.effectSummary());
            lines.add("    ✦ " + c.flavor());
            if (!c.styleHint().isEmpty()) {
                lines.add("    → " + c.styleHint());
            }
        }
        lines.add(declineShortLabel(offer));
        return new RenderContent(lines);
    }

    private static boolean isHeartDemon(InsightOfferViewModel offer) {
        return offer.triggerId().startsWith(HEART_DEMON_TRIGGER_PREFIX);
    }

    private static String screenTitle(InsightOfferViewModel offer) {
        return isHeartDemon(offer) ? "◇ 心 魔 劫 ◇" : TITLE.getString();
    }

    private static String metaQuotaLabel(InsightOfferViewModel offer) {
        return isHeartDemon(offer)
            ? "心魔抉择: " + offer.choices().size() + " 项"
            : "剩余顿悟额度: " + offer.quotaRemaining() + "/" + offer.quotaTotal();
    }

    private static String timerLabel(InsightOfferViewModel offer, long secondsLeft) {
        return isHeartDemon(offer)
            ? "心魔倒计时: " + secondsLeft + "s（超时默认执念）"
            : "⏳ " + secondsLeft + "s";
    }

    private static String declineLabel(InsightOfferViewModel offer) {
        return isHeartDemon(offer)
            ? "[ 不作答 ]  交由心魔拖入执念"
            : "[ 心未契机 ]  拒绝, 不消耗额度";
    }

    private static String declineShortLabel(InsightOfferViewModel offer) {
        return isHeartDemon(offer) ? "[ 不作答 ]" : "[ 心未契机 ]";
    }

    InsightOfferViewModel offer() {
        return offer;
    }

    boolean settledForTests() {
        return settled;
    }

    public record RenderContent(List<String> lines) {
        public RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
