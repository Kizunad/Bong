package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;
import software.bernie.geckolib.renderer.GeoEntityRenderer;

public abstract class BongModeledEntityRenderer extends GeoEntityRenderer<BongModeledEntity> {
    private final BongEntityModelKind modelKind;

    protected BongModeledEntityRenderer(EntityRendererFactory.Context context, BongEntityModelKind modelKind) {
        super(context, new BongModeledEntityModel(modelKind));
        this.modelKind = modelKind;
        this.shadowRadius = modelKind.shadowRadius();
    }

    public BongEntityModelKind modelKindForTests() {
        return modelKind;
    }
}
