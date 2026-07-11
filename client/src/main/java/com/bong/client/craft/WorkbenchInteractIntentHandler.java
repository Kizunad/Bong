package com.bong.client.craft;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;

/**
 * 制作台 marker 实体右键交互 handler。
 *
 * <p>制作台是 Bong 自定义 Marker + 自定义渲染实体，不能依赖 vanilla
 * InteractEntityEvent。客户端从准星命中的 {@link BongModeledEntity} 取 protocol
 * entity id，再发送 {@code workbench_open} C2S 交给 server 校验距离和打开 UI。</p>
 */
public final class WorkbenchInteractIntentHandler implements IntentHandler {
    /**
     * 制作台交互范围（方块数），必须与 server 侧
     * {@code WORKBENCH_INTERACT_RANGE}（server/src/craft/workbench.rs）保持一致。
     *
     * <p>server 用 Chebyshev 距离（各轴最大分量 ≤ 3）做最终授权，因此这里的候选
     * 预判必须复用同一种距离形状，而不是欧氏球半径——否则会出现 client 认为可交互、
     * server 却拒绝的假交互带（plan-bughunt-entity-interact-range-desync-v1）。</p>
     */
    static final double WORKBENCH_INTERACT_RANGE_BLOCKS = 3.0;
    private static final String LABEL_PREFIX = "workbench:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return Optional.empty();
        }
        if (!(hit.getEntity() instanceof BongModeledEntity modeled)) {
            return Optional.empty();
        }
        if (!isWorkbenchKind(modeled.modelKind())) {
            return Optional.empty();
        }
        double distSq = client.player.squaredDistanceTo(hit.getEntity());
        if (!isWithinInteractRange(
            client.player.getX(), client.player.getY(), client.player.getZ(),
            hit.getEntity().getX(), hit.getEntity().getY(), hit.getEntity().getZ()
        )) {
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
        ClientRequestSender.sendWorkbenchOpen(candidateEntityId);
        return true;
    }

    public static boolean isWorkbenchKind(BongEntityModelKind kind) {
        return kind == BongEntityModelKind.WORKBENCH;
    }

    /**
     * 复现 server {@code is_within_workbench_range} 的判定：先把实体世界坐标 floor
     * 还原成放置时的方块坐标（制作台实体 x/z 落在方块中心 +0.5，y 落在方块底），
     * 再用玩家的原始（未 floor）坐标做 Chebyshev 距离判定。
     */
    static boolean isWithinInteractRange(
        double playerX, double playerY, double playerZ,
        double entityX, double entityY, double entityZ
    ) {
        double blockX = Math.floor(entityX);
        double blockY = Math.floor(entityY);
        double blockZ = Math.floor(entityZ);
        double dx = Math.abs(playerX - blockX);
        double dy = Math.abs(playerY - blockY);
        double dz = Math.abs(playerZ - blockZ);
        return Math.max(dx, Math.max(dy, dz)) <= WORKBENCH_INTERACT_RANGE_BLOCKS;
    }

    static int candidateEntityId(InteractCandidate candidate) {
        if (candidate == null || candidate.debugLabel() == null) {
            return -1;
        }
        if (!candidate.debugLabel().startsWith(LABEL_PREFIX)) {
            return -1;
        }
        try {
            return Integer.parseInt(candidate.debugLabel().substring(LABEL_PREFIX.length()));
        } catch (NumberFormatException exception) {
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
