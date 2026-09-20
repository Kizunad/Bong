package com.bong.client.ui.model;

import com.bong.client.BongClient;
import com.bong.client.inspect.ItemInspectModel;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.DiffuseLighting;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.RotationAxis;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.opengl.GL11;

/** 统一预览实际模型；测量和绘制共享同一帧顶点，不把碰撞箱误当成模型尺寸。 */
public class ModelPreviewComponent extends BaseComponent implements AutoCloseable {
    private final MinecraftClient client = MinecraftClient.getInstance();
    private final ModelPreviewCamera camera = new ModelPreviewCamera();
    private ModelPreviewCatalog.Entry option = ModelPreviewCatalog.Entry.player();
    private ModelPreviewMesh mesh = new ModelPreviewMesh();
    private Entity entity;
    private ItemInspectModel item;
    private boolean material = true;
    private boolean dragging;
    private long lastFrame;
    private Identifier clayTexture;
    private String failure;
    private Box framing;

    public ModelPreviewComponent() {
        id("model-preview");
        sizing(Sizing.fill(100));
    }

    public ModelPreviewCamera camera() { return camera; }
    public boolean material() { return material; }
    public void material(boolean enabled) { material = enabled; }
    public ModelPreviewCatalog.Entry option() { return option; }
    public String failure() { return failure; }

    public void option(ModelPreviewCatalog.Entry next) {
        option = java.util.Objects.requireNonNull(next);
        invalidate();
    }

    public void invalidate() {
        entity = null;
        item = null;
        mesh = new ModelPreviewMesh();
        framing = null;
        failure = null;
        dragging = false;
        camera.reset();
    }

    @Override public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        long now = System.nanoTime();
        float elapsed = lastFrame == 0 ? 0 : (now - lastFrame) / 1_000_000_000f;
        lastFrame = now;
        if (!client.isWindowFocused() || client.currentScreen == null) dragging = false;
        camera.advance(elapsed);
        context.fillGradient(x, y, x + width, y + height, 0xFF0B1219, 0xFF202F36);
        context.drawRectOutline(x, y, width, height, 0xFF40515A);
        context.draw();
        if (client.world == null) { message(context, "进入世界后可预览模型"); return; }
        if (failure != null) { message(context, failure); return; }
        boolean depth = GL11.glIsEnabled(GL11.GL_DEPTH_TEST);
        boolean blending = GL11.glIsEnabled(GL11.GL_BLEND);
        float[] shaderColor = RenderSystem.getShaderColor().clone();
        context.enableScissor(x + 1, y + 1, x + width - 1, y + height - 1);
        try {
            mesh.clear();
            collectModel(new MatrixStack(), mesh, partialTicks);
            Box bounds = mesh.bounds();
            if (bounds == null) { message(context, "此实体没有可显示的模型"); return; }
            // 动画伸展时扩大取景，避免待机动作导致镜头每帧忽大忽小。
            framing = framing == null ? bounds : framing.union(bounds);
            prepareCamera(framing);
            var matrices = context.getMatrices();
            var buffers = client.getBufferBuilders().getEntityVertexConsumers();
            matrices.push();
            try {
                float scale = camera.scale(framing, width, height);
                var center = camera.center(framing);
                matrices.translate(x + width / 2f, y + height / 2f, 60);
                // 窗口之间有固定深度间隔，压缩局部 Z 保留模型自遮挡且不穿透上层窗口。
                matrices.scale(scale, -scale, scale * .1f);
                matrices.multiply(RotationAxis.POSITIVE_X.rotationDegrees(camera.pitch()));
                matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(camera.yaw()));
                matrices.translate(-center.x, -center.y, -center.z);
                RenderSystem.enableDepthTest();
                DiffuseLighting.enableGuiDepthLighting();
                RenderSystem.setShaderColor(1, 1, 1, 1);
                try {
                    mesh.draw(matrices, buffers, material ? null : clayLayer());
                } catch (RuntimeException | Error original) {
                    try { buffers.draw(); }
                    catch (RuntimeException | Error cleanup) { if (cleanup != original) original.addSuppressed(cleanup); }
                    throw original;
                }
                buffers.draw();
                drawModelOverlay(context, matrices, framing, mouseX, mouseY, elapsed);
            } finally { matrices.pop(); }
        } catch (RuntimeException renderFailure) {
            failure = "模型加载失败，请选择其他模型";
            BongClient.LOGGER.error("模型预览失败：{}", option.id(), renderFailure);
        } finally {
            restoreRenderState(context, shaderColor, depth, blending);
        }
    }

    /**
     * Restores every global render state even when an earlier restoration step fails.
     * The semantics match UiPreviewCleanup, but draw() calls this every frame, so varargs
     * cleanup would allocate on the render hot path; keep the sequence local instead.
     */
    private static void restoreRenderState(OwoUIDrawContext context, float[] shaderColor,
                                           boolean depth, boolean blending) {
        Throwable primary = null;
        try {
            context.disableScissor();
        } catch (RuntimeException | Error failure) {
            primary = failure;
        }
        try {
            DiffuseLighting.enableGuiDepthLighting();
        } catch (RuntimeException | Error failure) {
            primary = accumulate(primary, failure);
        }
        try {
            RenderSystem.setShaderColor(shaderColor[0], shaderColor[1], shaderColor[2], shaderColor[3]);
        } catch (RuntimeException | Error failure) {
            primary = accumulate(primary, failure);
        }
        try {
            if (depth) RenderSystem.enableDepthTest(); else RenderSystem.disableDepthTest();
        } catch (RuntimeException | Error failure) {
            primary = accumulate(primary, failure);
        }
        try {
            if (blending) RenderSystem.enableBlend(); else RenderSystem.disableBlend();
        } catch (RuntimeException | Error failure) {
            primary = accumulate(primary, failure);
        }
        if (primary instanceof RuntimeException failure) throw failure;
        if (primary instanceof Error failure) throw failure;
    }

    private static Throwable accumulate(Throwable primary, Throwable failure) {
        if (primary == null) return failure;
        if (failure != primary) primary.addSuppressed(failure);
        return primary;
    }

    protected void prepareCamera(Box bounds) {}

    protected void drawModelOverlay(OwoUIDrawContext context, MatrixStack matrices, Box bounds,
                                    int mouseX, int mouseY, float elapsed) {}

    protected void collectModel(MatrixStack matrices, net.minecraft.client.render.VertexConsumerProvider vertices,
                                float partialTicks) {
        if (option.item() != null) {
            if (item == null) item = option.item().get();
            if (!item.yUp()) matrices.scale(1, -1, 1);
            item.render(matrices, vertices);
            return;
        }
        if (option.category() == ModelPreviewCatalog.Category.PLAYER) entity = client.player;
        else if (entity == null) entity = option.entityType().create(client.world);
        if (entity == null) return;
        if (entity != client.player) entity.age = client.player == null ? 0 : client.player.age;
        // 直接调用 renderer，避免 GUI 里绘制世界阴影、碰撞调试线和火焰。
        var renderer = client.getEntityRenderDispatcher().getRenderer(entity);
        if (renderer == null) return;
        if (entity instanceof LivingEntity living) {
            float body = living.bodyYaw, prevBody = living.prevBodyYaw;
            float head = living.headYaw, prevHead = living.prevHeadYaw;
            float pitch = living.getPitch(), prevPitch = living.prevPitch;
            try {
                living.bodyYaw = living.prevBodyYaw = living.headYaw = living.prevHeadYaw = 0;
                living.setPitch(0); living.prevPitch = 0;
                renderer.render(entity, 0, partialTicks, matrices, vertices, LightmapTextureManager.MAX_LIGHT_COORDINATE);
            } finally {
                living.bodyYaw = body; living.prevBodyYaw = prevBody;
                living.headYaw = head; living.prevHeadYaw = prevHead;
                living.setPitch(pitch); living.prevPitch = prevPitch;
            }
        } else renderer.render(entity, 0, partialTicks, matrices, vertices, LightmapTextureManager.MAX_LIGHT_COORDINATE);
    }

    protected Identifier whiteTexture() {
        if (clayTexture == null) {
            var image = new NativeImage(1, 1, false);
            image.setColor(0, 0, 0xFFFFFFFF);
            clayTexture = client.getTextureManager().registerDynamicTexture("model-preview-clay", new NativeImageBackedTexture(image));
        }
        return clayTexture;
    }

    private RenderLayer clayLayer() { return RenderLayer.getEntitySolid(whiteTexture()); }

    private void message(OwoUIDrawContext context, String text) {
        context.drawText(client.textRenderer, client.textRenderer.trimToWidth(text, Math.max(1, width - 16)),
            x + 8, y + height / 2, 0xFFB6C8CE, false);
    }

    @Override public boolean canFocus(FocusSource source) { return true; }
    @Override public void onFocusLost() { dragging = false; super.onFocusLost(); }
    @Override public boolean onMouseDown(double mouseX, double mouseY, int button) {
        // owo 输入坐标相对于组件左上角，绘制坐标才是屏幕坐标。
        if (button != GLFW.GLFW_MOUSE_BUTTON_LEFT && button != GLFW.GLFW_MOUSE_BUTTON_RIGHT) return false;
        dragging = !camera.autoRotate();
        return true;
    }
    @Override public boolean onMouseUp(double mouseX, double mouseY, int button) { dragging = false; return true; }
    @Override public boolean onMouseDrag(double mouseX, double mouseY, double dx, double dy, int button) {
        if (dragging) camera.drag(dx, dy);
        return true;
    }
    @Override public boolean onMouseScroll(double mouseX, double mouseY, double amount) { camera.scroll(amount); return true; }
    @Override public void close() {
        var texture = clayTexture;
        clayTexture = null;
        entity = null;
        item = null;
        mesh.clear();
        if (texture != null) client.getTextureManager().destroyTexture(texture);
    }
}
