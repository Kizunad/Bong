package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;

/** plan-workbench-place-runtime-v1 P2 — 制作台渲染器。 */
public final class WorkbenchRenderer extends BongModeledEntityRenderer {
    public WorkbenchRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.WORKBENCH);
    }
}
