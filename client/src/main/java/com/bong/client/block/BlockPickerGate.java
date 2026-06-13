package com.bong.client.block;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerInteractionManager;
import net.minecraft.world.GameMode;

/**
 * plan-worldgen-v4 P5 §8.1#5 — dev-only 方块审阅面板的显示门控。
 *
 * <p>对齐 HUD conditional display 约束：未满足 dev/creative 前置时面板**完全不挂载**
 * （而非灰态）。当前以 creative gamemode 作为 dev 信号——画廊审阅是 dev 工作流，玩家
 * survival 进度路径不应看到此面板。</p>
 *
 * <p>门控判定经 {@link GameModeSupplier} seam 暴露，便于 headless 单测注入 gamemode
 * 而无需启动客户端。</p>
 */
public final class BlockPickerGate {
    /** gamemode 来源 seam（默认读运行时 client）。 */
    @FunctionalInterface
    public interface GameModeSupplier {
        /** @return 当前 gamemode，未连接 / 无 interactionManager 时返回 {@code null}。 */
        GameMode currentGameMode();
    }

    private static final GameModeSupplier DEFAULT_SUPPLIER = () -> {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return null;
        }
        ClientPlayerInteractionManager im = client.interactionManager;
        return im == null ? null : im.getCurrentGameMode();
    };

    private static volatile GameModeSupplier supplier = DEFAULT_SUPPLIER;

    private BlockPickerGate() {
    }

    /**
     * @return {@code true} 当且仅当面板应显示（当前 gamemode 为 creative）。
     */
    public static boolean isDevOrCreative() {
        GameMode mode = supplier.currentGameMode();
        return mode != null && mode.isCreative();
    }

    /** 纯函数门控核心，便于覆盖每个 gamemode 变体（含 null）。 */
    public static boolean isDevOrCreative(GameMode mode) {
        return mode != null && mode.isCreative();
    }

    public static void setGameModeSupplierForTests(GameModeSupplier s) {
        supplier = s == null ? DEFAULT_SUPPLIER : s;
    }

    public static void resetForTests() {
        supplier = DEFAULT_SUPPLIER;
    }
}
