package com.bong.client.cultivation;

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
 * F7：远距离突破方位指示器渲染管线接线。
 *
 * <p>{@link DistantBreakthroughRenderer#billboardFor} 是纯函数（已有专属单测），此前从未被
 * 生产渲染路径调用——唯一调用者是测试。本类负责把它接进真实渲染帧：
 * <ul>
 *   <li>先例：{@code com.bong.client.npc.NpcLodWorldRenderer}——{@link #register()} 挂
 *       {@link WorldRenderEvents} 回调，取 camera 位置，billboard 文字用 {@code TextRenderer}
 *       + camera-facing 矩阵变换画出</li>
 *   <li>数据源：{@link BreakthroughRenderStateStore}，由 {@link BreakthroughCinematicHandler}
 *       每次收到 payload 时写入</li>
 * </ul>
 *
 * <p>渲染回调本身不做单测（需要真实 {@link MinecraftClient} 环境），仅 {@link #labelFor} /
 * {@link #applyAlpha} 等纯逻辑部分可测，测试风格仿 {@code NpcLodWorldRendererTest}。
 */
public final class BreakthroughBillboardWorldRenderer {

    /** 文字基础缩放；billboard.scale() 在此基础上再乘一层距离衰减。 */
    private static final float BASE_SCALE = 0.045F;

    private BreakthroughBillboardWorldRenderer() {
    }

    /** 在 {@code BongClient.onInitializeClient()} 中调用一次。 */
    public static void register() {
        WorldRenderEvents.END.register(BreakthroughBillboardWorldRenderer::render);
    }

    private static void render(WorldRenderContext context) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.world == null || client.player == null) {
            return;
        }
        BreakthroughRenderState state = BreakthroughRenderStateStore.snapshot();
        if (state == null || state.isExpired(System.currentTimeMillis())) {
            return;
        }

        Vec3d camera = context.camera().getPos();
        DistantBreakthroughRenderer.Billboard billboard =
            DistantBreakthroughRenderer.billboardFor(state.payload(), camera.x, camera.y, camera.z);
        if (!billboard.visible()) {
            return;
        }

        VertexConsumerProvider consumers = context.consumers();
        MatrixStack matrices = context.matrixStack();
        if (consumers == null || matrices == null) {
            return;
        }

        BreakthroughCinematicPayload payload = state.payload();
        TextRenderer textRenderer = client.textRenderer;
        String label = labelFor(payload);
        int color = applyAlpha(billboard.tintArgb(), billboard.alpha());
        float scale = (float) (BASE_SCALE * billboard.scale());

        double worldX = payload.worldX();
        double worldY = payload.worldY() + 3.0;
        double worldZ = payload.worldZ();

        matrices.push();
        matrices.translate(worldX - camera.x, worldY - camera.y, worldZ - camera.z);
        matrices.multiply(context.camera().getRotation());
        matrices.scale(-scale, -scale, scale);
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

    /**
     * 远景 billboard 单字标签：aftermath 按成败区分（"成"/"破"），进行中阶段统一显示"劫"
     * （渡劫中，与 tribulation 叙事一致）。package-private 以便测试直接断言。
     */
    static String labelFor(BreakthroughCinematicPayload payload) {
        if (payload.phase() == BreakthroughCinematicPayload.Phase.AFTERMATH) {
            return (payload.result().failed() || payload.interrupted()) ? "破" : "成";
        }
        return "劫";
    }

    /**
     * 用距离衰减出的 alpha（{@link DistantBreakthroughRenderer.Billboard#alpha()}）覆盖
     * tint 自带的透明度字节——tint 的 alpha 只是 {@code tintFor} 里的固定占位值，
     * 真正的"越远越淡"由 billboard.alpha() 控制。
     */
    static int applyAlpha(int rgb, double alpha) {
        int a = (int) Math.round(clamp01(alpha) * 255.0);
        return (a << 24) | (rgb & 0x00FFFFFF);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
