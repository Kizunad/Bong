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
        if (!isCoffinKind(kind)) {
            return Optional.empty();
        }
        double distSq = client.player.squaredDistanceTo(hit.getEntity());
        if (distSq > MAX_INTERACT_DISTANCE_SQ) {
            return Optional.empty();
        }
        return Optional.of(InteractCandidate.of(
            InteractIntent.OpenContainer,
            ReservedInteractionIntents.OPEN_CONTAINER_PRIORITY,
            distSq,
            LABEL_PREFIX + hit.getEntity().getId()
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
        // marker 实体坐标 → 棺 lower/upper 任一格（server 端 registry 归一）。
        BlockPos coffinPos = hit.getEntity().getBlockPos();
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
