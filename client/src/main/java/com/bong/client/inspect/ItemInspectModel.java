package com.bong.client.inspect;

import com.bong.client.armor.ArmorModelRegistry;
import com.bong.client.armor.ArmorPartModel;
import com.bong.client.armor.WornPackModel;
import com.bong.client.armor.WornPackModelRegistry;
import com.bong.client.block.BlockVanillaIconMap;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.weapon.WeaponVanillaIconMap;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.json.ModelTransformationMode;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.item.ItemStack;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.Direction;
import net.minecraft.util.math.random.Random;
import org.joml.Vector3f;

import java.util.ArrayList;
import java.util.Optional;

/** 消费现有物品/穿戴模型；检视只调整取景，不建立第二份资产映射。 */
public interface ItemInspectModel {
    Box bounds();
    boolean yUp();
    void render(MatrixStack matrices, VertexConsumerProvider consumers);

    static ItemInspectModel stack(ItemStack stack) {
        return new StackModel(stack);
    }

    static Optional<ItemInspectModel> find(String itemId) {
        var armor = ArmorModelRegistry.get(itemId);
        if (armor.isPresent()) {
            var spec = armor.get();
            ModelPart part = ArmorPartModel.buildModelPart(spec.modelKey());
            if (spec.slot() == EquipSlotType.LEGS || spec.slot() == EquipSlotType.FEET) {
                var mounts = ArmorPartModel.mountsForSlot(spec.slot());
                part.getChild(mounts.get(0).childName()).pivotX = 1.9f;
                part.getChild(mounts.get(1).childName()).pivotX = -1.9f;
            }
            return Optional.of(new PartModel(part, spec.textureId()));
        }
        var pack = WornPackModelRegistry.get(itemId);
        if (pack.isPresent()) {
            return Optional.of(new PartModel(WornPackModel.buildBackModelPart(), pack.get().textureId()));
        }
        ItemStack stack = WeaponVanillaIconMap.createStackFor(itemId);
        if (stack == null) stack = BlockVanillaIconMap.createStackFor(itemId).orElse(null);
        if (stack == null || stack.isEmpty()) return Optional.empty();
        BakedModel model = MinecraftClient.getInstance().getItemRenderer().getModel(stack, null, null, 0);
        if (model == MinecraftClient.getInstance().getBakedModelManager().getMissingModel()) return Optional.empty();
        return Optional.of(new StackModel(stack));
    }

    final class StackModel implements ItemInspectModel {
        private final ItemStack stack;
        private BakedModel measuredModel;
        private Box bounds;

        private StackModel(ItemStack stack) { this.stack = stack; }

        @Override public Box bounds() {
            var renderer = MinecraftClient.getInstance().getItemRenderer();
            BakedModel model = renderer.getModel(stack, null, null, 0);
            if (model != measuredModel) {
                Bounds measured = new Bounds();
                var faces = new ArrayList<Direction>();
                faces.add(null);
                faces.addAll(java.util.List.of(Direction.values()));
                for (Direction face : faces) {
                    for (var quad : model.getQuads(null, face, Random.create(42))) {
                        int[] vertices = quad.getVertexData();
                        int stride = vertices.length / 4;
                        for (int vertex = 0; vertex < 4; vertex++) {
                            int offset = vertex * stride;
                            // ItemRenderer 在 NONE 模式仍将模型中心平移 -0.5。
                            measured.add(Float.intBitsToFloat(vertices[offset]) - .5f,
                                Float.intBitsToFloat(vertices[offset + 1]) - .5f,
                                Float.intBitsToFloat(vertices[offset + 2]) - .5f);
                        }
                    }
                }
                bounds = measured.box();
                measuredModel = model;
            }
            return bounds;
        }

        @Override public boolean yUp() { return true; }

        @Override public void render(MatrixStack matrices, VertexConsumerProvider consumers) {
            MinecraftClient.getInstance().getItemRenderer().renderItem(stack, ModelTransformationMode.NONE,
                LightmapTextureManager.MAX_LIGHT_COORDINATE, OverlayTexture.DEFAULT_UV, matrices, consumers, null, 0);
        }
    }

    final class PartModel implements ItemInspectModel {
        private final ModelPart part;
        private final Identifier texture;
        private final Box bounds;

        private PartModel(ModelPart part, Identifier texture) {
            this.part = part;
            this.texture = texture;
            Bounds measured = new Bounds();
            part.forEachCuboid(new MatrixStack(), (entry, path, index, cube) -> {
                for (float x : new float[]{cube.minX, cube.maxX}) {
                    for (float y : new float[]{cube.minY, cube.maxY}) {
                        for (float z : new float[]{cube.minZ, cube.maxZ}) {
                            Vector3f point = entry.getPositionMatrix().transformPosition(x / 16, y / 16, z / 16, new Vector3f());
                            measured.add(point.x, point.y, point.z);
                        }
                    }
                }
            });
            bounds = measured.box();
        }

        @Override public Box bounds() { return bounds; }
        @Override public boolean yUp() { return false; }
        @Override public void render(MatrixStack matrices, VertexConsumerProvider consumers) {
            part.render(matrices, consumers.getBuffer(RenderLayer.getEntityCutoutNoCull(texture)),
                LightmapTextureManager.MAX_LIGHT_COORDINATE, OverlayTexture.DEFAULT_UV);
        }
    }

    final class Bounds {
        private Box box;
        void add(float x, float y, float z) {
            Box point = new Box(x, y, z, x, y, z);
            box = box == null ? point : box.union(point);
        }
        Box box() { return box == null ? new Box(-.5, -.5, -.5, .5, .5, .5) : box; }
    }
}
