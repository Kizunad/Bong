package com.bong.client.network;

import net.minecraft.util.Identifier;

import java.util.OptionalInt;
import java.util.UUID;

/**
 * 把 {@link VfxEventPayload} 转成"对玩家对象的 API 调用"。抽成接口是为了：
 * <ul>
 *   <li>单元测试：纯 JVM 环境下不能实例化 {@code MinecraftClient} / 玩家实体，stub 一个
 *       记录型 bridge 就能验证路由逻辑</li>
 *   <li>Phase 2：多玩家广播 / 视距过滤如果要迁到 bridge 实现，路由层不用改</li>
 * </ul>
 *
 * <p>返回值语义：
 * <ul>
 *   <li>true：目标玩家在线 + 动画/层处理成功</li>
 *   <li>false：玩家不在线 / 动画 id 未注册 / 层不存在（stop_anim） —— 由调用方做节流日志</li>
 * </ul>
 */
public interface VfxEventAnimationBridge {
    boolean playAnim(UUID targetPlayer, Identifier animId, int priority, OptionalInt fadeInTicks);

    boolean stopAnim(UUID targetPlayer, Identifier animId, OptionalInt fadeOutTicks);
}
