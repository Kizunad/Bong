package com.bong.client.npc;

import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix4f;

/**
 * 远视野低 LOD NPC 渲染器（plan-offscreen-war-v1 §P7 PR-11）。
 *
 * <p>消费 {@link NpcLodStore} 中的 Mid tier NPC 快照，在 80–256 格范围内渲染
 * 简化 label（境界标签 + archetype icon）。
 *
 * <h3>LOD gate 规则</h3>
 * <ul>
 *   <li>&lt;80 格：近程 Near 档，走 Valence 原生 entity sync + {@code NpcNametagRenderer}，
 *       不走此路径（正常 MC entity 渲染 + 完整 nametag）
 *   <li>80–256 格：Mid 档，此渲染器生效——只显示境界 icon（中文单字），无动画细节
 *   <li>&gt;256 格：超出 server 发包范围，store 中无数据，跳过
 * </ul>
 *
 * <h3>视觉规格</h3>
 * <ul>
 *   <li>80–256 格：渲染单色境界图标（中文字符），颜色对应 archetype 色调
 *   <li>文字本身偏小（scale=0.018F），表示"远处模糊轮廓"而非清晰名牌
 *   <li>see-through 模式（隔墙可见），与 NpcNametagRenderer 保持一致
 * </ul>
 *
 * <h3>守恒红线</h3>
 * 此渲染器只读 {@link NpcLodStore}（只读渲染数据），零真元流动，
 * 不触碰 qi / ledger / NpcDormantStore。
 */
public final class NpcLodWorldRenderer {
    /**
     * LOD 近端门控距离（格）：小于此值的 NPC 由 {@code NpcNametagRenderer} 处理，
     * 此渲染器跳过。对应 server {@code NpcLodConfig.near_radius = 80}。
     */
    static final double NEAR_CUTOFF_DISTANCE = 80.0;

    /**
     * LOD 远端门控距离（格）：大于此值的 NPC 超出 server 发包范围，跳过。
     * 对应 server {@code NpcLodConfig.mid_radius = 256}。
     */
    static final double FAR_CUTOFF_DISTANCE = 256.0;

    private static final float SCALE = 0.018F;

    // 境界标签颜色（明度刻意降低，表达"远距模糊"感）
    private static final int COLOR_LOD_DEFAULT = 0x88B0B0B0;
    private static final int COLOR_LOD_HOSTILE = 0x99CC4040;
    private static final int COLOR_LOD_BEAST = 0x99A0784C;

    private NpcLodWorldRenderer() {
    }

    /** 在 {@code BongClient.onInitializeClient()} 中调用一次。 */
    public static void register() {
        WorldRenderEvents.AFTER_ENTITIES.register(NpcLodWorldRenderer::render);
    }

    /**
     * 根据 archetype 选择远视野 LOD label（单字，表示"远处可辨轮廓"）。
     *
     * <p>此方法是 package-private 以便测试直接断言。
     */
    static String lodLabelFor(String archetype) {
        if (archetype == null) {
            return "人";
        }
        return switch (archetype.trim().toLowerCase(java.util.Locale.ROOT)) {
            case "zombie" -> "尸";
            case "rogue" -> "散";
            case "commoner" -> "凡";
            case "disciple" -> "余";
            case "beast" -> "兽";
            case "guardian_relic" -> "守";
            case "daoxiang" -> "伥";
            case "zhinian" -> "执";
            case "fuya" -> "压";
            default -> "人";
        };
    }

    /**
     * 根据 archetype 选择颜色。
     *
     * <p>hostile-ish archetype（zombie/guardian_relic）用偏红色，beast 用棕色，其余灰色。
     */
    static int lodColorFor(String archetype) {
        if (archetype == null) {
            return COLOR_LOD_DEFAULT;
        }
        return switch (archetype.trim().toLowerCase(java.util.Locale.ROOT)) {
            case "zombie", "guardian_relic" -> COLOR_LOD_HOSTILE;
            case "beast" -> COLOR_LOD_BEAST;
            default -> COLOR_LOD_DEFAULT;
        };
    }

    /**
     * 判断此 snapshot 是否在 LOD 渲染范围（80–256 格）内。
     *
     * @param snapshot     NPC 快照
     * @param playerX      玩家 X
     * @param playerZ      玩家 Z
     * @return {@code true} 在范围内，需渲染
     */
    static boolean isInLodRange(NpcLodSnapshot snapshot, double playerX, double playerZ) {
        double distSq = snapshot.distanceSquaredXzTo(playerX, playerZ);
        double nearSq = NEAR_CUTOFF_DISTANCE * NEAR_CUTOFF_DISTANCE;
        double farSq = FAR_CUTOFF_DISTANCE * FAR_CUTOFF_DISTANCE;
        return distSq >= nearSq && distSq <= farSq;
    }

    private static void render(WorldRenderContext context) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.world == null || client.player == null) {
            return;
        }
        VertexConsumerProvider consumers = context.consumers();
        MatrixStack matrices = context.matrixStack();
        if (consumers == null || matrices == null) {
            return;
        }

        Vec3d camera = context.camera().getPos();
        TextRenderer textRenderer = client.textRenderer;
        Vec3d playerPos = client.player.getPos();
        double px = playerPos.x;
        double pz = playerPos.z;

        for (NpcLodSnapshot snapshot : NpcLodStore.snapshot()) {
            if (!isInLodRange(snapshot, px, pz)) {
                continue;
            }

            // 若 NpcMetadataStore 里有该 entity 的完整数据（近程已同步），
            // 则此处不重复渲染——NpcNametagRenderer 已负责近程 label。
            // （距离判断已将 <80格 过滤掉，此分支是额外安全保障）
            if (NpcMetadataStore.get(snapshot.entityId()) != null) {
                // 近程已有完整 metadata，跳过——但若距离 >80格 仍有 metadata（缓存残留）
                // 则保留 LOD label 避免闪烁。仅当距离小于近端门控时真正跳过。
                double distSq = snapshot.distanceSquaredXzTo(px, pz);
                if (distSq < NEAR_CUTOFF_DISTANCE * NEAR_CUTOFF_DISTANCE) {
                    continue;
                }
            }

            String label = lodLabelFor(snapshot.archetype());
            int color = lodColorFor(snapshot.archetype());

            // 渲染位置：NPC 坐标头顶（+2.5格 Y，远距时 overhead 标记）
            double worldX = snapshot.x();
            double worldY = snapshot.y() + 2.5;
            double worldZ = snapshot.z();

            matrices.push();
            matrices.translate(worldX - camera.x, worldY - camera.y, worldZ - camera.z);
            matrices.multiply(context.camera().getRotation());
            matrices.scale(-SCALE, -SCALE, SCALE);
            Matrix4f matrix = matrices.peek().getPositionMatrix();
            float tx = -textRenderer.getWidth(label) / 2.0F;
            textRenderer.draw(
                label,
                tx,
                0.0F,
                color,
                false,
                matrix,
                consumers,
                TextRenderer.TextLayerType.SEE_THROUGH,
                0x20000000,
                LightmapTextureManager.MAX_LIGHT_COORDINATE
            );
            matrices.pop();
        }
    }
}
