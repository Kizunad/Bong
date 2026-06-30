package com.bong.client.fauna;

import com.bong.client.network.VfxEntityAnimationBridge;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.Entity;

/**
 * {@link VfxEntityAnimationBridge} 生产实现：按 MC 协议 entity_id 在当前 {@link ClientWorld}
 * 找实体，若是 {@link FaunaEntity} 就触发其一次性招式动画。
 *
 * <p>黑武士 boss 走此路径（无 UUID 的 Marker 实体，{@code play_anim} 按玩家寻人走不通）。
 *
 * <p>返回值：
 * <ul>
 *   <li>{@code true}：找到 {@link FaunaEntity} 且触发成功（有效 payload）</li>
 *   <li>{@code false}：world 未就绪 / id 找不到 / 不是 FaunaEntity（玩家不在视距等），
 *       或命中实体但触发被拒（空白动画名 / 非正时长）→ router 记 bridgeMiss</li>
 * </ul>
 */
public final class FaunaActionBridge implements VfxEntityAnimationBridge {
    @Override
    public boolean playEntityAnim(int entityId, String anim, int durationTicks) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return false;
        }
        ClientWorld world = client.world;
        if (world == null) {
            return false;
        }
        Entity entity = world.getEntityById(entityId);
        if (entity instanceof FaunaEntity fauna) {
            // 命中实体后回传真实触发结果：无效 payload（空白名 / 非正时长）下 router 应记 bridgeMiss，
            // 不能恒 true 把无效事件吞成 handled。
            return fauna.triggerAction(anim, durationTicks);
        }
        return false;
    }
}
