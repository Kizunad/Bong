package com.bong.client.coffin;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * plan-coffin-tiers-v1 P3 — {@link CoffinMenuScreen} 最小单元测试。
 *
 * <p>CodeRabbit minor F: 构造函数 null 检查 + 正常构造不抛异常。</p>
 *
 * <p>注意：{@link CoffinMenuScreen} 继承自 {@code net.minecraft.client.gui.screen.Screen}，
 * 构造函数仅调用 {@code super(Text.literal(...))} 并存储 {@code coffinPos}。
 * Minecraft 的 {@code Screen} 构造函数在 headless 环境不依赖游戏实例，因此可直接 new。</p>
 */
class CoffinMenuScreenTest {

    /**
     * 正常构造（有效 BlockPos）不应抛出任何异常。
     * 期望：coffinPos 非 null → super(title) + this.coffinPos = pos，无 NPE / 运行时异常。
     */
    @Test
    void constructorWithValidPosDoesNotThrow() {
        BlockPos pos = new BlockPos(10, 64, -5);
        // 构造本身不依赖游戏实例（只调 super(Text) + 赋值）。
        CoffinMenuScreen screen = new CoffinMenuScreen(pos);
        assertNotNull(
            screen,
            "CoffinMenuScreen(BlockPos) should construct without throwing; " +
            "期望：非 null 实例，pos=" + pos
        );
    }

    /**
     * null coffinPos 应立即抛出 {@link NullPointerException}（Objects.requireNonNull 防御）。
     * 期望：构造函数检测 null → throw NPE，message 含 "coffinPos"。
     */
    @Test
    void constructorThrowsNpeForNullPos() {
        NullPointerException ex = assertThrows(
            NullPointerException.class,
            () -> new CoffinMenuScreen(null),
            "CoffinMenuScreen(null) should throw NullPointerException; " +
            "期望：Objects.requireNonNull 守卫，防止 NPE 延迟到 onReclaim()/onEnter() 深处"
        );
        assertNotNull(
            ex,
            "NullPointerException should not be null itself (trivially true, confirms throw path)"
        );
    }
}
