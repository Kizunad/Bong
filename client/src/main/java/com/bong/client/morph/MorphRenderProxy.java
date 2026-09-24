package com.bong.client.morph;

import com.bong.client.inventory.state.MorphStateStore;
import com.bong.client.whale.WhaleEntities;
import com.bong.client.whale.WhaleEntity;
import net.minecraft.entity.player.PlayerEntity;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * plan-race-system-v1 PR-5b — 易形玩家渲染代理：GeckoLib
 * {@code GeoEntityRenderer<WhaleEntity>} 只能渲染真实 {@link WhaleEntity} 实例，
 * player 实体本身不实现 {@code GeoEntity}，无法直接复用同一个 renderer 画。
 *
 * <p>本类为每个当前易形为 whale 的玩家 entity id 维护一个复用的「代理」
 * {@link WhaleEntity}（只借它的动画/模型渲染管线，不参与物理/碰撞/tick），
 * 渲染 mixin（{@link com.bong.client.mixin.MixinMorphedPlayerRenderer}）
 * 每帧取代理实例、同步 {@code age} 保证动画相位跟随真实玩家推进，再交给
 * {@code EntityRenderDispatcher} 正常渲染流程（含 shadow/fire，位置/朝向由
 * dispatcher 按调用方传入的 x/y/z/yaw 精确摆放，代理自身坐标不参与）。
 *
 * <p>代理生命周期跟随 {@link MorphStateStore}：玩家解除易形/下线导致表里
 * 该 entity_id 消失时，{@link #pruneStale()}（挂在 store 的变更监听器上）
 * 清掉对应代理，防止 {@link ConcurrentHashMap} 无限增长。
 */
public final class MorphRenderProxy {
    private static final Map<Integer, WhaleEntity> WHALE_PROXIES = new ConcurrentHashMap<>();

    static {
        MorphStateStore.addListener(MorphRenderProxy::pruneStale);
    }

    private MorphRenderProxy() {}

    /** 取（或惰性创建）给定玩家当前 tick 用于渲染鲸形态的代理实体，并同步动画相位。 */
    public static WhaleEntity whaleFor(PlayerEntity player) {
        WhaleEntity proxy = WHALE_PROXIES.computeIfAbsent(
            player.getId(),
            id -> new WhaleEntity(WhaleEntities.whale(), player.getWorld())
        );
        proxy.age = player.age;
        return proxy;
    }

    /** 清掉表里已不再易形的玩家对应的代理实体（{@link MorphStateStore} 变更时调用）。 */
    private static void pruneStale() {
        WHALE_PROXIES.keySet().removeIf(
            entityId -> MorphStateStore.morphOf(entityId).isEmpty()
        );
    }

    /** 测试专用：清空全部代理缓存，防止跨测试用例状态泄漏。 */
    public static void resetForTests() {
        WHALE_PROXIES.clear();
    }
}
