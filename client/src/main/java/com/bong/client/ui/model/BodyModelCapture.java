package com.bong.client.ui.model;

import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.util.math.MatrixStack;
import java.util.IdentityHashMap;
import java.util.Map;
import java.util.Set;

/** 仅在内观收集网格时读取玩家模型的最终姿态，不替换或修改共享模型。 */
public final class BodyModelCapture {
    private static final ThreadLocal<BodyModelCapture> ACTIVE = new ThreadLocal<>();
    private final PlayerEntityModel<?> expected;
    private final Set<ModelPart> expectedParts;
    private final Map<ModelPart, BodyModelGeometry.Region> regions = new IdentityHashMap<>();

    private BodyModelCapture(PlayerEntityModel<?> expected) {
        this.expected = expected;
        expectedParts = Set.of(expected.head, expected.body, expected.leftArm, expected.rightArm, expected.leftLeg, expected.rightLeg);
    }

    public static BodyModelGeometry collect(PlayerEntityModel<?> model, Runnable render) {
        var capture = new BodyModelCapture(model);
        var previous = ACTIVE.get();
        ACTIVE.set(capture);
        try {
            render.run();
            return capture.regions.size() == capture.expectedParts.size()
                ? new BodyModelGeometry(capture.expected, capture.regions::get) : null;
        } finally {
            if (previous == null) ACTIVE.remove(); else ACTIVE.set(previous);
        }
    }

    public static void onPartRender(ModelPart part, MatrixStack matrices) {
        var active = ACTIVE.get();
        if (active != null && active.expectedParts.contains(part)) {
            active.regions.put(part, BodyModelGeometry.region(part, matrices));
        }
    }
}
