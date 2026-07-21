package com.bong.client.animation;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.function.UnaryOperator;

/**
 * plan-fpv-cast-av-v1 P0：FPV 技术路线 A/B/C POC 的运行时切换键位。
 *
 * <p>按一下循环 {@link FpvPocState} OFF → A → B → C → OFF，并在 actionbar 报当前路线，让用户在
 * runClient 里施同一招（如 sword.cleave）时实时对比第一人称手臂/持物遮挡表现（§8 #1 决定性判据）。
 *
 * <p>键位**默认不绑定**（不占用 F1-F9 快捷栏），用户在「选项 → 控制 → category.bong-client」里
 * 绑一个键（建议 F 区或小键盘）。POC 收尾（路线拍板）后本类随 harness 一并移除。
 */
public final class FpvPocControls {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String CYCLE_KEY = "key.bong-client.fpv_poc_cycle";
    private static KeyBinding cycleKey;

    private FpvPocControls() {
    }

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(FpvPocControls::onEndClientTick);
    }

    private static void onEndClientTick(MinecraftClient client) {
        boolean cycled = false;
        while (keyBinding().wasPressed()) {
            FpvPocState.cycle();
            cycled = true;
        }
        if (cycled && client.player != null) {
            FpvPocState route = FpvPocState.current();
            client.player.sendMessage(
                Text.literal("[FPV POC] 路线 = " + route.name() + "  " + describe(route)),
                true /* actionbar */
            );
        }
    }

    private static String describe(FpvPocState route) {
        return switch (route) {
            case OFF -> "出厂现状（THIRD_PERSON_MODEL，第一人称只见持物、无手臂）";
            case A -> "库原生：THIRD_PERSON_MODEL + config 全开（手臂+持物）";
            case B -> "自绘层（NONE，渲染器待补 → 暂等价 vanilla FP 手臂）";
            case C -> "vanilla 注入（VANILLA，骨骼注入待补 → 暂为库 vanilla FP）";
        };
    }

    private static KeyBinding keyBinding() {
        if (cycleKey == null) {
            installCycleKey(KeyBindingHelper::registerKeyBinding);
        }
        return cycleKey;
    }

    static KeyBinding installCycleKey(UnaryOperator<KeyBinding> registrar) {
        // 默认不绑定：F1-F9 留给快捷栏，用户自行在控制里绑键。
        cycleKey = registrar.apply(
            new KeyBinding(CYCLE_KEY, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_UNKNOWN, CATEGORY)
        );
        return cycleKey;
    }

    static void resetControlsForTests() {
        cycleKey = null;
    }
}
