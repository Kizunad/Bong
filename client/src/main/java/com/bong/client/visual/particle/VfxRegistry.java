package com.bong.client.visual.particle;

import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;

/**
 * plan-particle-system-v1 §2.7：{@code event_id → VfxPlayer} 的查表。
 *
 * <p>线程模型：{@link #register} 在客户端 bootstrap（{@code onInitializeClient}）调用，
 * {@link #lookup} 在 MC 主线程调用。用 {@link ConcurrentHashMap} 支持 Phase 3 可能的运行时
 * 动态注册（例如 datapack 热加载）。
 *
 * <p>单例模式的原因：MC 客户端本身就是单进程单 world 上下文，没有多实例需求；且 {@link VfxRegistry}
 * 维护的是 compile-time 注册表，与游戏状态解耦。
 */
public final class VfxRegistry {
    private static final VfxRegistry INSTANCE = new VfxRegistry();

    private final Map<Identifier, VfxPlayer> players = new ConcurrentHashMap<>();

    private VfxRegistry() {
    }

    public static VfxRegistry instance() {
        return INSTANCE;
    }

    /**
     * 注册或替换一个事件的播放器。重复注册返回上一版的 player —— 用于热重载 / 测试清理。
     *
     * @return 被替换的旧 player，若为首次注册则 {@code null}
     */
    public VfxPlayer register(Identifier eventId, VfxPlayer player) {
        Objects.requireNonNull(eventId, "eventId");
        Objects.requireNonNull(player, "player");
        return players.put(eventId, player);
    }

    public Optional<VfxPlayer> lookup(Identifier eventId) {
        return Optional.ofNullable(players.get(eventId));
    }

    public boolean contains(Identifier eventId) {
        return players.containsKey(eventId);
    }

    /** 仅测试。生产代码不要调用。 */
    public void clearForTests() {
        players.clear();
    }
}
