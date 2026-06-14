package com.bong.client.mineral;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;

import java.util.function.Consumer;

/**
 * plan-interaction-intent-cleanup-v1 P0 — 神识感矿脉「主动触发」键位入口。
 *
 * <p>历史上 {@link ClientRequestSender#sendMineralProbe(int, int, int)} 是个孤岛
 * C2S：server 侧 {@code mineral_probe} → {@code MineralProbeIntent} → resolver →
 * {@code mineral_probe_result} S2C 全链路都齐，但客户端从来没有任何触发点把它接上。
 * 本类把它接到一个专属键位（默认 N，可在原版控制里重绑）：
 *
 * <ul>
 *   <li>玩家看向某方块时按键 → 发一次 {@code mineral_probe(x, y, z)}（按键边沿单发，
 *       不做被动每 tick 轮询，杜绝刷屏）。</li>
 *   <li>境界不足 / 非灵脉 / 超距等由 server resolver 统一裁决，回执
 *       {@code mineral_probe_result(denied)} 由 {@code MineralProbeResultHandler}
 *       渲染成一条 actionbar overlay（一次性，非刷屏）。</li>
 *   <li>没看向任何方块时按键 → no-op（不发包）。</li>
 * </ul>
 *
 * <p>不注册 server SkillRegistry（故 SkillMeridianDependencies::declare 红线不适用）；
 * 复用既有 {@code mineral_probe} 协议，不新增任何 C2S / S2C 包。
 */
public final class MineralSenseBootstrap {
    private static final String CATEGORY = "category.bong-client.mineral";
    private static final String SENSE_KEY_TRANSLATION = "key.bong-client.mineral_sense";

    private static KeyBinding senseKey;

    /**
     * 探针发送回调 seam —— 默认走真实 {@link ClientRequestSender}，测试可替换以断言
     * 发包行为，避免依赖真实网络后端。
     */
    private static Consumer<BlockPos> probeSender = MineralSenseBootstrap::sendProbe;

    private MineralSenseBootstrap() {}

    public static void register() {
        senseKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(SENSE_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_N, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(MineralSenseBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered mineral-sense interaction: key N (active probe, edge-triggered).");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        // wasPressed() 把整帧累计的按下次数逐个排出 —— 用 while 防丢键，每次按下最多发一包。
        while (senseKey.wasPressed()) {
            BlockPos target = targetedBlock(client);
            if (target != null) {
                probeSender.accept(target);
            }
        }
    }

    private static void sendProbe(BlockPos pos) {
        ClientRequestSender.sendMineralProbe(pos.getX(), pos.getY(), pos.getZ());
    }

    private static BlockPos targetedBlock(MinecraftClient client) {
        if (client.crosshairTarget instanceof BlockHitResult hit
            && hit.getType() == HitResult.Type.BLOCK) {
            return hit.getBlockPos();
        }
        return null;
    }

    // ─── 测试 seam ──────────────────────────────────────────────────

    /** 测试用：注入一个捕获 probeSender，断言「看向方块按键时发包、空准星不发包」。 */
    static void setProbeSenderForTests(Consumer<BlockPos> sender) {
        probeSender = sender == null ? MineralSenseBootstrap::sendProbe : sender;
    }

    static void resetForTests() {
        probeSender = MineralSenseBootstrap::sendProbe;
    }

    /**
     * 测试用：模拟「玩家正看向 {@code target}（null 表示空准星）时按下感矿脉键」一次的派发逻辑。
     * 返回是否发出探针。直接复刻 {@link #onEndClientTick} 内核心分支，不依赖 MC tick 循环。
     */
    static boolean dispatchSenseForTests(BlockPos target) {
        if (target == null) {
            return false;
        }
        probeSender.accept(target);
        return true;
    }
}
