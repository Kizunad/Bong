package com.bong.client.inspect;

import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.ItemIconRegistry;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.DiffuseLighting;
import net.minecraft.util.math.RotationAxis;
import org.lwjgl.glfw.GLFW;

/** 图标与真实模型共用一个内容区；旋转只消费本窗模型区域的按住输入。 */
public final class ItemModelPreviewComponent extends BaseComponent {
    private final String itemId;
    private final ItemInspectModel model;
    private boolean showModel;
    private float yaw = 30;
    private int rotatingButton = -1;
    private long lastFrame;

    public ItemModelPreviewComponent(String itemId) {
        this.itemId = itemId;
        model = ItemInspectModel.find(itemId).orElse(null);
        id("item-media");
        verticalSizing(Sizing.fixed(80));
    }

    public String itemId() { return itemId; }
    public boolean hasModel() { return model != null; }
    public boolean showingModel() { return showModel; }

    public void toggleModel() {
        if (model == null) return;
        showModel = !showModel;
        rotatingButton = -1;
        lastFrame = 0;
        verticalSizing(Sizing.fixed(showModel ? 140 : 80));
    }

    public void cancelRotation() { rotatingButton = -1; }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        MinecraftClient client = MinecraftClient.getInstance();
        long now = System.nanoTime();
        float elapsed = lastFrame == 0 ? 0 : Math.min(.1f, (now - lastFrame) / 1_000_000_000f);
        lastFrame = now;
        if (!client.isWindowFocused() || !(client.currentScreen instanceof InspectScreen)
            || !isInBoundingBox(mouseX, mouseY)) cancelRotation();
        context.fillGradient(x, y, x + width, y + height, 0xFF0E1214, 0xFF272D2C);
        context.drawRectOutline(x, y, width, height, 0xFF3E4540);
        context.fill(x + 1, y + 1, x + width - 1, y + 2, 0xFF070909);
        context.fill(x + 1, y + height - 2, x + width - 1, y + height - 1, 0xFF66695B);
        if (!showModel) {
            RenderSystem.enableBlend();
            RenderSystem.defaultBlendFunc();
            context.drawTexture(ItemIconRegistry.textureIdForItemId(itemId), x + (width - 64) / 2, y + 8,
                64, 64, 0, 0, 128, 128, 128, 128);
            return;
        }
        if (rotatingButton >= 0) yaw = (yaw + elapsed * (rotatingButton == 0 ? -70 : 70)) % 360;
        var bounds = model.bounds();
        double diameter = Math.sqrt(bounds.getXLength() * bounds.getXLength()
            + bounds.getYLength() * bounds.getYLength() + bounds.getZLength() * bounds.getZLength());
        float scale = (float) (Math.min(width - 20, height - 16) / Math.max(.01, diameter));
        var center = bounds.getCenter();
        var matrices = context.getMatrices();
        var buffers = client.getBufferBuilders().getEntityVertexConsumers();
        context.draw();
        matrices.push();
        try {
            matrices.translate(x + width / 2f, y + height / 2f, 30);
            // 保留窗口间的深度间隔，3D 自遮挡仍使用独立的局部深度。
            matrices.scale(1, 1, .1f);
            matrices.scale(scale, model.yUp() ? -scale : scale, scale);
            matrices.multiply(RotationAxis.POSITIVE_X.rotationDegrees(-15));
            matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(yaw));
            matrices.translate(-center.x, -center.y, -center.z);
            DiffuseLighting.enableGuiDepthLighting();
            RenderSystem.enableDepthTest();
            try {
                model.render(matrices, buffers);
            } catch (RuntimeException | Error failure) {
                try {
                    buffers.draw();
                } catch (RuntimeException | Error cleanupFailure) {
                    if (cleanupFailure != failure) failure.addSuppressed(cleanupFailure);
                }
                throw failure;
            }
            buffers.draw();
        } finally {
            matrices.pop();
            DiffuseLighting.enableGuiDepthLighting();
        }
    }

    @Override public boolean canFocus(FocusSource source) { return showModel; }

    @Override public void onFocusLost() {
        cancelRotation();
        super.onFocusLost();
    }

    @Override public boolean onMouseDown(double mouseX, double mouseY, int button) {
        if (!showModel || (button != GLFW.GLFW_MOUSE_BUTTON_LEFT && button != GLFW.GLFW_MOUSE_BUTTON_RIGHT)) return false;
        rotatingButton = button;
        lastFrame = System.nanoTime();
        return true;
    }

    @Override public boolean onMouseUp(double mouseX, double mouseY, int button) {
        boolean rotating = rotatingButton >= 0;
        cancelRotation();
        return rotating;
    }

    @Override public boolean onMouseDrag(double x, double y, double dx, double dy, int button) { return showModel; }
}
