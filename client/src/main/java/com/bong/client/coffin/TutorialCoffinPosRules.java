package com.bong.client.coffin;

import net.minecraft.util.math.BlockPos;

import java.util.Optional;

/**
 * F9 跨层修复 — 出生引导棺坐标判定的纯逻辑，从
 * {@link com.bong.client.mixin.MixinClientPlayerInteractionManagerAlchemy} 抽出以便直接单测
 * （mixin 内的方法依赖活的 {@code MinecraftClient.world}，没法脱离游戏环境测；
 * 仿 {@link com.bong.client.alchemy.AlchemyFurnaceInteractionRules} 的纯函数模式）。
 *
 * <p>取代此前硬编码的 {@code |x|<=8, y∈[60,90], |z|<=8} 判定盒——spawn 区域随地形重
 * 生成迁移后该盒会失配，真棺静默打不开。</p>
 */
public final class TutorialCoffinPosRules {
    private TutorialCoffinPosRules() {}

    /**
     * @param broadcastPos server join 时广播、由 {@link TutorialCoffinPosStore} 缓存的权威坐标；
     *                     {@code Optional.empty()} 表示尚未收到广播。
     * @param candidatePos 玩家右键命中的方块坐标。
     * @return {@code true} 当且仅当已收到广播且与命中坐标精确相等（整数坐标，无需容差）。
     *         未收到广播时 fail-closed 返回 {@code false}（不回退旧硬编码盒）。
     */
    public static boolean isSpawnTutorialCoffinPos(Optional<BlockPos> broadcastPos, BlockPos candidatePos) {
        return broadcastPos.map(candidatePos::equals).orElse(false);
    }
}
