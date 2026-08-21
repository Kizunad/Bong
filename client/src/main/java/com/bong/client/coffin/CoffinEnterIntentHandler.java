package com.bong.client.coffin;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.EntityHitResult;
import net.minecraft.util.math.BlockPos;

import java.util.Optional;

/**
 * 延寿棺 marker 实体交互 intent handler（plan-coffin-tiers-v1 P3）。
 *
 * <p>玩家准星瞄准四档延寿棺 marker（COFFIN_MUNDANE/JADE/STONE/BRONZE）时：
 * <ul>
 *   <li><b>G 键</b> → {@link #dispatch} 打开 {@link CoffinMenuScreen}，
 *       玩家在菜单内选 [入眠]（→ {@code coffin_enter}）或 [回收]（→ {@code coffin_menu_reclaim}）。
 *       右键（interactEntity）仅更新 TargetInfoStateStore，不触发菜单。</li>
 * </ul>
 *
 * <p><b>左键攻击（break）</b>：通过
 * {@link com.bong.client.mixin.MixinClientPlayerInteractionManagerAlchemy}
 * 的 {@code attackEntity} injection 处理，本 handler 不参与。</p>
 *
 * <p>出棺仍由 server 的 SneakEvent 触发（玩家潜行），无需 client 额外处理。</p>
 *
 * <p>仿 {@link com.bong.client.inventory.SupplyCoffinInteractIntentHandler} 模式：
 * entity hit 从 {@code client.crosshairTarget} 取，dispatch 读 debugLabel 做二次校验。</p>
 */
public final class CoffinEnterIntentHandler implements IntentHandler {
    private static final double MAX_INTERACT_DISTANCE_SQ = 6.0 * 6.0;
    private static final String LABEL_PREFIX = "coffin_enter:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return Optional.empty();
        }
        if (!(hit.getEntity() instanceof BongModeledEntity modeled)) {
            return Optional.empty();
        }
        BongEntityModelKind kind = modeled.modelKind();
        double distSq = client.player.squaredDistanceTo(hit.getEntity());
        return candidateForCoffin(kind, distSq, hit.getEntity().getId());
    }

    /**
     * <b>生产 candidate 门控</b>（package-private 纯函数，{@code candidate} 直接调用）：
     * 只对四档延寿棺 kind 且落在 6 格（{@code MAX_INTERACT_DISTANCE_SQ}=36）内的目标产出
     * OpenContainer candidate。抽出供 {@code CoffinEnterIntentHandlerTest} 正向按压
     * 「合法 marker 不返回 empty」——review finding [1]：Python 场景是 server 端点镜像，
     * 无法发现让 candidate 对合法 marker 恒返 empty 的变异；本条正面用例在 Java 侧锁死。
     */
    static Optional<InteractCandidate> candidateForCoffin(
        BongEntityModelKind kind,
        double distSq,
        int entityId
    ) {
        if (kind == null || !isCoffinKind(kind)) {
            return Optional.empty();
        }
        if (distSq > MAX_INTERACT_DISTANCE_SQ) {
            return Optional.empty();
        }
        return Optional.of(InteractCandidate.of(
            InteractIntent.OpenContainer,
            ReservedInteractionIntents.OPEN_CONTAINER_PRIORITY,
            distSq,
            LABEL_PREFIX + entityId
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        int candidateEntityId = candidateEntityId(candidate);
        if (candidateEntityId < 0) {
            return false;
        }
        EntityHitResult hit = entityHit(client);
        if (hit == null || hit.getEntity().getId() != candidateEntityId) {
            return false;
        }
        // marker 实体坐标 → 棺 block pos（生产派生，见 {@link #coffinBlockPos}）。
        // marker 取棺两格中心（server coffin/mod.rs coffin_marker_position），对 marker
        // 坐标取 floor 得到的是棺的 **upper** 格（lower.x+1, lower.y, lower.z）——
        // plan-coffin-tiers-v1 P3 生产者链（candidate → dispatch → CoffinMenuScreen →
        // [回收] → ClientRequestSender）实际发出的就是 upper 格，与
        // CoffinGMenuProducerChainTest 钉死的 payload 一致。
        BlockPos coffinPos = coffinBlockPos(
            hit.getEntity().getX(),
            hit.getEntity().getY(),
            hit.getEntity().getZ()
        );
        client.setScreen(new CoffinMenuScreen(coffinPos));
        return true;
    }

    /**
     * 四档延寿棺 MUNDANE/JADE/STONE/BRONZE。
     * COFFIN_COMMON/RARE/PRECIOUS 属于物资棺，不在此范围内。
     */
    public static boolean isCoffinKind(BongEntityModelKind kind) {
        return kind == BongEntityModelKind.COFFIN_MUNDANE
            || kind == BongEntityModelKind.COFFIN_JADE
            || kind == BongEntityModelKind.COFFIN_STONE
            || kind == BongEntityModelKind.COFFIN_BRONZE;
    }

    /**
     * marker 实体坐标 → 延寿棺 block pos 的生产派生（floor 语义，与 {@code Entity.getBlockPos}
     * 一致）。marker 位于棺两格中心：lower+(1, 0, 0.5)（server coffin/mod.rs
     * {@code coffin_marker_position}）→ floor 后得到棺的 **upper** 格（lower.x+1, lower.y,
     * lower.z），正是真实 G 菜单 [回收] 生产者发送的坐标。抽出为 package-private 纯函数，
     * 供 {@code CoffinGMenuProducerChainTest} 对 Java 侧产出的 upper-coordinate payload 做
     * 端到端断言（review finding [1]：场景侧不得再用 lower 冷注入镜像覆盖这条生产派生）。
     */
    static BlockPos coffinBlockPos(double x, double y, double z) {
        return new BlockPos((int) Math.floor(x), (int) Math.floor(y), (int) Math.floor(z));
    }

    /** Package-private for testing the label-parsing contract. */
    static int candidateEntityId(InteractCandidate candidate) {
        if (candidate == null || candidate.debugLabel() == null) {
            return -1;
        }
        if (!candidate.debugLabel().startsWith(LABEL_PREFIX)) {
            return -1;
        }
        try {
            return Integer.parseInt(candidate.debugLabel().substring(LABEL_PREFIX.length()));
        } catch (NumberFormatException e) {
            return -1;
        }
    }

    private static EntityHitResult entityHit(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        if (!(client.crosshairTarget instanceof EntityHitResult hit)) {
            return null;
        }
        return hit;
    }
}
