package com.bong.client.combat.screen;

import com.bong.client.combat.ForgeCarrierIntent;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.SliderComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.Objects;

/**
 * 暗器注入界面（plan §U5 / §1 ForgeWeaponCarrier）。
 *
 * <p>组件树与布局由本地 owo XML 持有；Java 只绑定选中状态、slider 变化和
 * 类型化意图。生产网络 sink 由 {@link com.bong.client.combat.ForgeCarrierScreenBootstrap}
 * 在组合根组装，避免界面直接依赖 sender。</p>
 */
public final class ForgeCarrierScreen extends OwoXmlScreenHost<FlowLayout> {
    private static final String DAGGER = "dagger";
    private static final String NEEDLE = "needle";

    private String selectedItem;
    private double qiInvest;
    private final UiIntentSink<ForgeCarrierIntent> intentSink;
    private LabelComponent selectionLabel;
    private LabelComponent qiLabel;
    private LabelComponent feedbackLabel;
    private SliderComponent qiSlider;
    private String feedbackText = "";

    public ForgeCarrierScreen(
        String selectedItem,
        double qiInvest,
        UiIntentSink<ForgeCarrierIntent> intentSink
    ) {
        super(Text.literal("暗器制作"), FlowLayout.class, "forge-carrier");
        this.selectedItem = normalizeItem(selectedItem);
        this.qiInvest = normalizeQiInvest(qiInvest);
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    @Override
    public boolean shouldPause() {
        return true;
    }

    /** XML 负责静态布局；这里只接入动态文本、slider 和 typed button callback。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        label("forge-carrier-title");
        selectionLabel = label("forge-carrier-selection");
        qiLabel = label("forge-carrier-qi-label");
        feedbackLabel = label("forge-carrier-feedback");

        ButtonComponent daggerButton = component(ButtonComponent.class, "forge-carrier-dagger");
        daggerButton.onPress(button -> selectItem(DAGGER, button));
        ButtonComponent needleButton = component(ButtonComponent.class, "forge-carrier-needle");
        needleButton.onPress(button -> selectItem(NEEDLE, button));

        qiSlider = component(SliderComponent.class, "forge-carrier-qi-slider")
            .value(qiInvest)
            .message(value -> Text.literal("真元 " + percent(parseSliderValue(value)) + "%"));
        component(ButtonComponent.class, "forge-carrier-submit")
            .onPress(button -> dispatch());
        refreshLabels();
    }

    @Override
    protected void onHostOpened(UiScreenScope scope) {
        if (qiSlider == null) {
            return;
        }
        SliderComponent.OnChanged listener = value -> {
            qiInvest = normalizeQiInvest(value);
            refreshLabels();
        };
        var subscription = qiSlider.onChanged().subscribe(listener);
        // slider 的回调属于屏幕局部资源，随 host 生命周期取消，避免关闭后的迟到回调。
        scope.addCleanup(subscription::cancel);
    }

    void dispatch() {
        UiIntentResult result = intentSink.dispatch(new ForgeCarrierIntent.Begin(selectedItem, qiInvest));
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            feedbackText = "";
            if (feedbackLabel != null) {
                feedbackLabel.text(Text.literal(feedbackText));
            }
            closeIfCurrentScreen();
            return;
        }
        feedbackText = "注入未提交: " + result.reason();
        if (feedbackLabel != null) {
            feedbackLabel.text(Text.literal(feedbackText));
        }
    }

    private void selectItem(String item, ButtonComponent source) {
        selectedItem = normalizeItem(item);
        String label = selectedItem.equals(DAGGER) ? "飞刀" : "飞针";
        source.setMessage(Text.literal("已选: " + label));
        refreshLabels();
    }

    private void refreshLabels() {
        if (selectionLabel != null) {
            selectionLabel.text(Text.literal("当前暗器: " + itemLabel(selectedItem)));
        }
        if (qiLabel != null) {
            qiLabel.text(Text.literal("注入真元比例: " + percent(qiInvest) + "%"));
        }
        if (feedbackLabel != null) {
            feedbackLabel.text(Text.literal(feedbackText));
        }
    }

    private void closeIfCurrentScreen() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.currentScreen == this) {
            client.setScreen(null);
        }
    }

    private static String normalizeItem(String value) {
        return NEEDLE.equals(value) ? NEEDLE : DAGGER;
    }

    private static double normalizeQiInvest(double value) {
        if (!Double.isFinite(value)) {
            return 0.5;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    private static double parseSliderValue(String value) {
        try {
            return normalizeQiInvest(Double.parseDouble(value));
        } catch (RuntimeException ignored) {
            return 0.0;
        }
    }

    private static long percent(double ratio) {
        return Math.round(normalizeQiInvest(ratio) * 100.0);
    }

    private static String itemLabel(String item) {
        return NEEDLE.equals(item) ? "飞针" : "飞刀";
    }

    String selectedItemForTests() {
        return selectedItem;
    }

    double qiInvestForTests() {
        return qiInvest;
    }

    String feedbackTextForTests() {
        return feedbackText;
    }

    void selectItemForTests(String item) {
        selectedItem = normalizeItem(item);
        refreshLabels();
    }

    void setQiInvestForTests(double value) {
        qiInvest = normalizeQiInvest(value);
        refreshLabels();
    }
}
