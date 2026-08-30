package com.bong.client.combat.screen;

import com.bong.client.combat.RepairClientIntentSink;
import com.bong.client.combat.RepairIntent;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.Objects;

/** 武器/法宝养护界面；XML 持有布局，Java 只绑定快照和类型化意图。 */
public final class RepairScreen extends OwoXmlScreenHost<FlowLayout> {
    private static final int DURABILITY_BAR_WIDTH = 200;

    private final float durabilityNorm;
    private final String weaponLabel;
    private final long weaponInstanceId;
    private final int stationX;
    private final int stationY;
    private final int stationZ;
    private final UiIntentSink<RepairIntent> intentSink;

    public RepairScreen(
        String weaponLabel,
        float durabilityNorm,
        long weaponInstanceId,
        int stationX,
        int stationY,
        int stationZ
    ) {
        this(
            weaponLabel,
            durabilityNorm,
            weaponInstanceId,
            stationX,
            stationY,
            stationZ,
            RepairClientIntentSink.production()
        );
    }

    RepairScreen(
        String weaponLabel,
        float durabilityNorm,
        long weaponInstanceId,
        int stationX,
        int stationY,
        int stationZ,
        UiIntentSink<RepairIntent> intentSink
    ) {
        super(Text.literal("养护"), FlowLayout.class, "repair");
        this.weaponLabel = weaponLabel == null ? "-" : weaponLabel;
        this.durabilityNorm = Math.max(0f, Math.min(1f, durabilityNorm));
        this.weaponInstanceId = Math.max(0L, weaponInstanceId);
        this.stationX = stationX;
        this.stationY = stationY;
        this.stationZ = stationZ;
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    @Override
    public boolean shouldPause() {
        return true;
    }

    /** XML 负责布局；这里只绑定动态文本、耐久条和两个 typed action。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        LabelComponent titleLabel = label("repair-title");
        titleLabel.text(Text.literal("养护 · " + weaponLabel));

        LabelComponent durabilityLabel = label("repair-durability");
        durabilityLabel.text(Text.literal("耐久: " + Math.round(durabilityNorm * 100) + "%"));

        FlowLayout durabilityFill = component(FlowLayout.class, "repair-durability-fill");
        durabilityFill.horizontalSizing(Sizing.fixed(Math.round(durabilityNorm * DURABILITY_BAR_WIDTH)));
        durabilityFill.surface(Surface.flat(durabilityColor()));

        component(ButtonComponent.class, "repair-steel")
            .onPress(button -> dispatch("refined_steel"));
        component(ButtonComponent.class, "repair-pill")
            .onPress(button -> dispatch("pill"));
    }

    private void dispatch(String material) {
        UiIntentResult result = intentSink.dispatch(new RepairIntent.Commit(
            material,
            weaponInstanceId,
            stationX,
            stationY,
            stationZ
        ));
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            closeIfCurrentScreen();
        }
    }

    private void closeIfCurrentScreen() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.currentScreen == this) {
            client.setScreen(null);
        }
    }

    private int durabilityColor() {
        return durabilityNorm < 0.3f
            ? 0xFFE04040
            : durabilityNorm < 0.7f ? 0xFFE0C040 : 0xFF60D060;
    }

    public float durabilityNormForTests() {
        return durabilityNorm;
    }

    public String weaponLabelForTests() {
        return weaponLabel;
    }

    public long weaponInstanceIdForTests() {
        return weaponInstanceId;
    }

    public int stationXForTests() {
        return stationX;
    }

    public int stationYForTests() {
        return stationY;
    }

    public int stationZForTests() {
        return stationZ;
    }

    public int durabilityBarWidthForTests() {
        return DURABILITY_BAR_WIDTH;
    }
}
