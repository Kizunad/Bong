package com.bong.client.social;

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

/**
 * plan-interaction-intent-cleanup-v1 P2 — 感灵龛「主动触发」入口。
 *
 * <p>历史实现是「被动凝视 3 秒自动发 gaze」+「M 键发 mark_coordinate」两条并存路径。
 * 被动凝视会在玩家只是随便看一眼方块时偷偷发包，属于隐式/意外触发。本版删除被动凝视轮询，
 * 统一为「看向方块按 M 键」一次性同时发 {@code spirit_niche_gaze} + {@code spirit_niche_mark_coordinate}：
 * gaze 走揭示判定链路，mark_coordinate 走标记/发现链路，server 侧两者各自独立消费。
 *
 * <p>边沿触发（{@link KeyBinding#wasPressed()} 排空），看向空准星按键 → no-op 不发包。
 */
public final class SpiritNicheRevealBootstrap {
    private static final String CATEGORY = "category.bong-client.social";
    private static final String MARK_KEY_TRANSLATION = "key.bong-client.spirit_niche_mark_coordinate";

    private static KeyBinding markKey;

    private SpiritNicheRevealBootstrap() {}

    public static void register() {
        markKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(MARK_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_M, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(SpiritNicheRevealBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered spirit niche reveal interaction: key M (active gaze+mark, edge-triggered).");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        while (markKey.wasPressed()) {
            BlockPos target = targetedBlock(client);
            if (target != null) {
                dispatchSense(target);
            }
        }
    }

    /**
     * 看向 {@code target} 按下 M 键 → 同时发 gaze（揭示判定）+ mark_coordinate（标记/发现）。
     * package-private 以便测试直接断言「一次按键发两包、坐标透传」「空准星不发包」。
     */
    static void dispatchSense(BlockPos target) {
        if (target == null) {
            return;
        }
        ClientRequestSender.sendSpiritNicheGaze(target.getX(), target.getY(), target.getZ());
        ClientRequestSender.sendSpiritNicheMarkCoordinate(target.getX(), target.getY(), target.getZ());
    }

    private static BlockPos targetedBlock(MinecraftClient client) {
        if (client.crosshairTarget instanceof BlockHitResult hit
            && hit.getType() == HitResult.Type.BLOCK) {
            return hit.getBlockPos();
        }
        return null;
    }
}
