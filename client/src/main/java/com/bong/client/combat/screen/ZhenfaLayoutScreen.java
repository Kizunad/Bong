package com.bong.client.combat.screen;

import com.bong.client.combat.ZhenfaLayoutIntent;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.SliderComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/** 阵法布置 UI；XML 持有布局，Java 只绑定语义状态和 typed intent。 */
public final class ZhenfaLayoutScreen extends OwoXmlScreenHost<FlowLayout> {
    private static final String DEFAULT_KIND = "trap";
    private static final String DEFAULT_TRIGGER = "proximity";
    private static final double DEFAULT_QI = 0.1;

    private final BlockPos targetPos;
    private final String kind;
    private final long itemInstanceId;
    private final String targetFace;
    private final UiIntentSink<ZhenfaLayoutIntent> intentSink;
    private String trigger = DEFAULT_TRIGGER;
    private double qiInvest = DEFAULT_QI;
    private LabelComponent summaryLabel;
    private SliderComponent qiSlider;
    private String feedbackText = "";

    /** 预览入口使用显式 no-op sink，不会成为生产打开路径。 */
    public static ZhenfaLayoutScreen preview() {
        return new ZhenfaLayoutScreen(
            new BlockPos(0, 64, 0), DEFAULT_KIND, 0L, null,
            intent -> UiIntentResult.accepted());
    }

    public ZhenfaLayoutScreen(
        BlockPos targetPos, String kind, long itemInstanceId, String targetFace,
        UiIntentSink<ZhenfaLayoutIntent> intentSink
    ) {
        super(Text.literal("阵法布置"), FlowLayout.class, "zhenfa-layout");
        this.targetPos = targetPos == null ? new BlockPos(0, 64, 0) : targetPos;
        this.kind = kind == null || kind.isBlank() ? DEFAULT_KIND : kind.strip();
        this.itemInstanceId = Math.max(0L, itemInstanceId);
        this.targetFace = targetFace == null || targetFace.isBlank() ? null : targetFace.strip();
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    @Override
    public boolean shouldPause() {
        return true;
    }

    /** XML 负责静态布局；这里只绑定 slider、按钮和 typed action。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        summaryLabel = label("zhenfa-summary");
        qiSlider = component(SliderComponent.class, "zhenfa-qi-slider")
            .value(qiInvest)
            .message(value -> Text.literal("真元 " + percent(parseSliderValue(value)) + "%"));
        component(ButtonComponent.class, "zhenfa-trigger-proximity")
            .active(!usesFixedTrapTrigger())
            .onPress(button -> selectTrigger("proximity", button));
        component(ButtonComponent.class, "zhenfa-trigger-contact")
            .active(!usesFixedTrapTrigger())
            .onPress(button -> selectTrigger("contact", button));
        component(ButtonComponent.class, "zhenfa-trigger-timed")
            .active(!usesFixedTrapTrigger())
            .onPress(button -> selectTrigger("timed", button));
        component(ButtonComponent.class, "zhenfa-place").onPress(button -> dispatch());
        label("zhenfa-feedback").text(Text.literal(feedbackText));
        refreshSummary();
    }

    @Override
    protected void onHostOpened(com.bong.client.ui.contract.UiScreenScope scope) {
        if (qiSlider == null) return;
        var subscription = qiSlider.onChanged().subscribe(value -> {
            qiInvest = normalizeQi(value);
            refreshSummary();
        });
        // slider 回调属于屏幕局部资源，随 host 生命周期取消，避免关闭后的迟到回调。
        scope.addCleanup(subscription::cancel);
    }

    private void selectTrigger(String next, ButtonComponent source) {
        if (usesFixedTrapTrigger()) return;
        trigger = next;
        source.setMessage(Text.literal("已选 " + next));
        refreshSummary();
    }

    void dispatch() {
        UiIntentResult result = intentSink.dispatch(placementIntent());
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            feedbackText = "";
            closeIfCurrentScreen();
            return;
        }
        feedbackText = "阵法未落定: " + result.reason();
        if (hostReadyForTests()) label("zhenfa-feedback").text(Text.literal(feedbackText));
    }

    private ZhenfaLayoutIntent.Place placementIntent() {
        return new ZhenfaLayoutIntent.Place(
            targetPos.getX(), targetPos.getY(), targetPos.getZ(), kind, "common_stone", qiInvest,
            usesFixedTrapTrigger() ? null : trigger, itemInstanceId > 0L ? itemInstanceId : null, targetFace);
    }

    private boolean usesFixedTrapTrigger() {
        return switch (kind) {
            case "warning_trap", "blast_trap", "slow_trap", "beast_trap", "trip_wire", "decoy_stake" -> true;
            default -> false;
        };
    }

    private void refreshSummary() {
        if (summaryLabel != null) {
            summaryLabel.text(Text.literal("触发类型: " + (usesFixedTrapTrigger() ? "固定" : trigger)
                + "    真元: " + percent(qiInvest) + "%"));
        }
    }

    private void closeIfCurrentScreen() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.currentScreen == this) client.setScreen(null);
    }

    private static double normalizeQi(double value) {
        if (!Double.isFinite(value)) return DEFAULT_QI;
        return Math.max(0.0, Math.min(1.0, value));
    }

    private static double parseSliderValue(String value) {
        try {
            return normalizeQi(Double.parseDouble(value));
        } catch (RuntimeException ignored) {
            return DEFAULT_QI;
        }
    }

    private static long percent(double value) {
        return Math.round(normalizeQi(value) * 100.0);
    }

    ZhenfaLayoutIntent.Place placementIntentForTests() {
        return placementIntent();
    }

    String feedbackTextForTests() {
        return feedbackText;
    }

}
