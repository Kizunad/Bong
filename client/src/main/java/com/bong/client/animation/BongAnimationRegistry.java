package com.bong.client.animation;

import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationRegistry;
import net.minecraft.util.Identifier;

import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

/**
 * 全局 KeyframeAnimation 注册表。
 *
 * <p><b>查询顺序</b>：JSON（资源包） → Java（硬编码 fallback）。
 * <ul>
 *   <li>JSON 源：{@link PlayerAnimationRegistry}——PlayerAnimator 自带的资源 reload listener
 *       扫描 {@code assets/{namespace}/player_animation/*.json}，启动时 + F3+T 时自动填充</li>
 *   <li>Java 源：{@link #register} 可注入备用动画；Phase 2 之后 19 个 Phase 1 动画已全部
 *       迁到 JSON, Java map 默认为空 —— 保留 register API 是为了第三方 mod 注入 / 单元测试
 *       stub / 未来动态生成动画的可能</li>
 * </ul>
 *
 * <p><b>为什么 JSON-first 要下沉到查询层而不是 bootstrap 层</b>：
 * {@code BongAnimations.bootstrap()} 在 Fabric ClientModInitializer 阶段跑，比 vanilla
 * 资源 manager 首轮 load 更早——那一刻 {@link PlayerAnimationRegistry#getAnimations()}
 * 还是空的。如果 bootstrap 时做 JSON 检查，必然永远 fallback。放在查询层则同时覆盖
 * "首轮加载后"和"F3+T 热重载"两种情况，零时序负担。
 *
 * <p>幂等：Java 源同 id 二次注册会覆盖旧值并返回原值，方便开发期热替换。
 *
 * <p>线程模型：仅客户端主线程访问，不加锁；调用方自觉。
 */
public final class BongAnimationRegistry {
    /** Java 硬编码源。Phase 2 起 19 个 Phase 1 动画全部走 JSON，此 map 默认为空。 */
    private static final Map<Identifier, KeyframeAnimation> ANIMATIONS = new LinkedHashMap<>();

    /** 标识动画来源，用于 {@code /anim list} 诊断。 */
    public enum Source { JSON, JAVA, NONE }

    private BongAnimationRegistry() {
    }

    /** 注册或覆盖 <b>Java fallback</b> 动画，返回被覆盖的旧值（或 null）。 */
    public static KeyframeAnimation register(Identifier id, KeyframeAnimation animation) {
        if (id == null || animation == null) {
            throw new IllegalArgumentException("id 和 animation 均不能为 null");
        }
        return ANIMATIONS.put(id, animation);
    }

    /**
     * 按 id 取动画：JSON 优先，未命中回落到 Java fallback；都没有返回 null。
     */
    public static KeyframeAnimation get(Identifier id) {
        if (id == null) return null;
        KeyframeAnimation fromJson = lookupJson(id);
        if (fromJson != null) return fromJson;
        return ANIMATIONS.get(id);
    }

    /** JSON ∪ Java 任一源命中即视为存在。 */
    public static boolean contains(Identifier id) {
        if (id == null) return false;
        return lookupJson(id) != null || ANIMATIONS.containsKey(id);
    }

    /**
     * JSON ∪ Java 的 id 并集。用于 {@code /anim list} 和 tab 补全。
     *
     * <p>顺序：先 Java（保持老 LinkedHashMap 注册顺序），后 JSON 追加的新 id。
     */
    public static Collection<Identifier> ids() {
        Set<Identifier> merged = new LinkedHashSet<>(ANIMATIONS.keySet());
        // 只纳入 Bong 自己 namespace 下的 JSON 动画；资源包可能添加其它 namespace 的，
        // 那些不应当出现在 Bong 的命令行补全里（否则 /anim play seriousplayeranimations:eating
        // 会乱成一锅粥）。
        for (Identifier jsonId : PlayerAnimationRegistry.getAnimations().keySet()) {
            if (BongAnimations.MOD_ID.equals(jsonId.getNamespace())) {
                merged.add(jsonId);
            }
        }
        return Collections.unmodifiableCollection(merged);
    }

    /** 诊断：某个 id 当前是从哪里取到的（JSON / JAVA / NONE）。 */
    public static Source sourceOf(Identifier id) {
        if (id == null) return Source.NONE;
        if (lookupJson(id) != null) return Source.JSON;
        if (ANIMATIONS.containsKey(id)) return Source.JAVA;
        return Source.NONE;
    }

    /**
     * 仅查 PlayerAnimator 资源包源。抽出来便于未来做 null-safe 空环境（比如单元测试
     * classloader 里 {@link PlayerAnimationRegistry} 未初始化时）——暂时没遇到问题，
     * 留这个钩子以防万一。
     */
    private static KeyframeAnimation lookupJson(Identifier id) {
        return PlayerAnimationRegistry.getAnimation(id);
    }
}
