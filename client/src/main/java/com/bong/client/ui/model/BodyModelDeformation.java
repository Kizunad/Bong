package com.bong.client.ui.model;

import io.github.kosmx.bendylib.MutableCuboid;
import io.github.kosmx.bendylib.impl.BendableCuboid;
import io.github.kosmx.bendylib.impl.RememberingPos;
import net.minecraft.client.model.ModelPart;
import net.minecraft.util.math.Vec3d;

/** 复用已安装动画库的变形算子；只变换内观点，不修改模型的顶点或动画状态。 */
record BodyModelDeformation(BendableCuboid cuboid, float axis, float angle) {
    static BodyModelDeformation capture(ModelPart.Cuboid cube) {
        if (!(cube instanceof MutableCuboid mutable)) return null;
        var active = mutable.getActiveMutator();
        if (active == null || !(active.getRight() instanceof BendableCuboid bend) || bend.getBend() == 0) return null;
        return new BodyModelDeformation(bend, bend.getBendAxis(), bend.getBend());
    }

    Vec3d apply(Vec3d point) {
        var position = new RememberingPos((float) point.x * 16, (float) point.y * 16, (float) point.z * 16);
        // 三参数重载只操作提供的位置集合，不会修改 cuboid 的 bend 或其模型网格。
        cuboid.applyBend(axis, angle, consumer -> consumer.accept(position));
        var result = position.getPos();
        return new Vec3d(result.x / 16.0, result.y / 16.0, result.z / 16.0);
    }
}
