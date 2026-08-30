package com.bong.client.combat.juice;

import com.bong.client.ui.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.Locale;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;

/**
 * plan-fpv-cast-av-v1 §P3「可及性」—— juice 强度全局倍率的<b>运行时可调入口</b>。
 *
 * <p>形态沿用仓库既有 keybind 先例：由 R7 全局 registry 注册按键，再由
 * {@link ClientTickEvents#END_CLIENT_TICK} 消费按下事件 → 改配置持有者 {@link JuiceConfig}。
 *
 * <p>按一次循环下一档（0 关闭 → 0.5 → 1.0 默认 → 1.5 → 回 0）。<b>默认不绑键</b>——与既有先例
 * 一致（F1-F9 留给快捷栏），玩家在原版「按键设置 → Bong Combat HUD」里自行绑定。每次切换在
 * 动作栏回显新档位，玩家知道自己调到了哪一档（尤其「关闭」需要明确可见，否则会误以为 juice 坏了）。
 */
public final class JuiceControls {
    private static final String CATEGORY = "category.bong-client.combat";
    private static final String KEY_TRANSLATION = "key.bong-client.juice_multiplier_cycle";

    /**
     * 动作栏回显的翻译 key（en_us / zh_cn 双份）。<b>不写死中文</b>——英文客户端也得看懂
     * （review finding H）。百分比档位走 {@link #FEEDBACK_LEVEL_KEY}，参数是已格式化的整数
     * 百分数字符串（lang 里的 {@code %%} 渲染成字面 {@code %}）；关闭档单独一条 key，因为
     * 「0%」容易被玩家当成显示 bug。
     */
    static final String FEEDBACK_OFF_KEY = "message.bong-client.juice_multiplier.off";
    static final String FEEDBACK_LEVEL_KEY = "message.bong-client.juice_multiplier.level";

    private static KeyBinding cycleKey;
    private static boolean registered;

    private JuiceControls() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        installCycleKey(BongKeybindRegistry.global());
        ClientTickEvents.END_CLIENT_TICK.register(JuiceControls::onEndClientTick);
        registered = true;
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null) {
            return;
        }
        consumeInstalledCyclePresses(
            client.player != null,
            client.currentScreen != null,
            message -> {
                if (client.player != null) {
                    // true = 动作栏瞬态回显（先例 InspectScreenBootstrap / LingtianActionScreen）。
                    client.player.sendMessage(message, true);
                }
            }
        );
    }

    static int consumeInstalledCyclePresses(
        boolean playerPresent, boolean screenOpen, Consumer<Text> feedback
    ) {
        return consumeCyclePresses(
            playerPresent,
            screenOpen,
            () -> cycleKey != null && cycleKey.wasPressed(),
            feedback
        );
    }

    /**
     * 消费本 tick 累积的按键（{@code wasPressed} 是队列语义，必须循环取干净，否则连按丢档）。
     *
     * <p><b>无玩家 / GUI 打开时仍然把队列排空，只是不改配置、不回显</b>：
     * {@code KeyBinding.wasPressed} 是**累积队列**，早退（不读队列）只是把按键攒着——
     * 玩家在输入框里按到的键会在关掉界面后的下一 tick 被补消费，倍率延迟连跳几档
     * （review finding G）。「避免在输入框里打字触发」的正确实现是**丢弃**，不是攒着。
     *
     * @return 本次<b>生效</b>的切档次数（被门控时恒为 0，即便排空了若干按键）
     */
    static int consumeCyclePresses(
        boolean playerPresent, boolean screenOpen, BooleanSupplier wasPressed, Consumer<Text> feedback
    ) {
        boolean gated = !playerPresent || screenOpen;
        int consumed = 0;
        while (wasPressed.getAsBoolean()) {
            if (gated) {
                continue;  // 排空丢弃：不改配置、不回显
            }
            float next = JuiceConfig.cycleJuiceMultiplier();
            if (feedback != null) {
                feedback.accept(describe(next));
            }
            consumed++;
        }
        return consumed;
    }

    /**
     * 动作栏文案：0 明写「关闭」（百分比 0% 容易被当成显示 bug）。
     *
     * <p>百分数用 {@link Locale#ROOT} 格式化成整数字符串再交给翻译层——数字形态不进 lang，
     * 免得两份 lang 各写一套格式化规则漂移。
     */
    static Text describe(float multiplier) {
        return multiplier <= 0f
            ? Text.translatable(FEEDBACK_OFF_KEY)
            : Text.translatable(
                FEEDBACK_LEVEL_KEY, String.format(Locale.ROOT, "%.0f", multiplier * 100f));
    }

    static KeyBinding installCycleKey(BongKeybindRegistry registry) {
        // 默认不绑键：F1-F9 留给快捷栏，玩家可在控制设置中显式重绑。
        return cycleKey = registry.register(new BongKeybindRegistry.BindingSpec(
            new BongKeybindRegistry.BindingOwner("combat.juice_multiplier_cycle"),
            KEY_TRANSLATION,
            InputUtil.Type.KEYSYM,
            InputUtil.UNKNOWN_KEY.getCode(),
            CATEGORY
        ));
    }

    static void resetControlsForTests() {
        cycleKey = null;
        registered = false;
    }
}
