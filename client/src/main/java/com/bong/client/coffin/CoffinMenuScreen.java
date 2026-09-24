package com.bong.client.coffin;

import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/**
 * 延寿棺 G 菜单（plan-coffin-tiers-v1 P3）。
 *
 * <p>玩家瞄准延寿棺 marker 实体并按 G 键（或右键）时弹出，提供：
 * <ul>
 *   <li>[入眠] — 发送 {@code coffin_enter}，进入卧棺状态；</li>
 *   <li>[回收] — 发送 {@code coffin_menu_reclaim}，拆棺返还合成材料；</li>
 * </ul>
 * 组件树由本地 owo XML 模板描述，Java 只负责绑定两个类型化意图。
 * 菜单不暂停游戏（{@code shouldPause=false}）。</p>
 */
public final class CoffinMenuScreen extends OwoXmlScreenHost<FlowLayout> {
    private final BlockPos coffinPos;
    private final UiIntentSink<CoffinMenuIntent> intentSink;

    public CoffinMenuScreen(BlockPos coffinPos) {
        this(coffinPos, CoffinMenuClientIntentSink.production());
    }

    CoffinMenuScreen(BlockPos coffinPos, UiIntentSink<CoffinMenuIntent> intentSink) {
        super(Text.literal("延寿棺"), FlowLayout.class, "coffin-menu");
        this.coffinPos = Objects.requireNonNull(coffinPos, "coffinPos must not be null");
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    /** XML 负责面板和按钮布局；这里只登记 typed action callback。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        label("coffin-title").text(Text.literal("◇ 延 寿 棺 ◇"));
        component(ButtonComponent.class, "coffin-enter")
            .onPress(button -> dispatch(new CoffinMenuIntent.Enter(coffinPos)));
        component(ButtonComponent.class, "coffin-reclaim")
            .onPress(button -> dispatch(new CoffinMenuIntent.Reclaim(coffinPos)));
    }

    /** 保留给生产交互链测试的动作入口，实际按钮也只通过同一 typed sink。 */
    private void onEnter() {
        dispatch(new CoffinMenuIntent.Enter(coffinPos));
    }

    /** XML [回收] 按钮对应的生产动作，供交互链测试复用。 */
    void onReclaim() {
        dispatch(new CoffinMenuIntent.Reclaim(coffinPos));
    }

    private void dispatch(CoffinMenuIntent intent) {
        UiIntentResult result = intentSink.dispatch(intent);
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            closeIfCurrentScreen();
        }
    }

    private void closeIfCurrentScreen() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }
}
