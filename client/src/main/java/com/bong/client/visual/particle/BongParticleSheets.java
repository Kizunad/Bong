package com.bong.client.visual.particle;

import com.mojang.blaze3d.systems.VertexSorter;
import net.minecraft.client.particle.ParticleTextureSheet;
import net.minecraft.client.render.BufferBuilder;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.Tessellator;
import net.minecraft.client.texture.SpriteAtlasTexture;
import net.minecraft.client.texture.TextureManager;

/** Custom particle sheets used by Bong world-space VFX. */
public final class BongParticleSheets {
    private BongParticleSheets() {
    }

    public static final ParticleTextureSheet LINE_EMISSIVE = new ParticleTextureSheet() {
        private final RenderLayer layer = RenderLayer.getEntityTranslucentEmissive(
            SpriteAtlasTexture.PARTICLE_ATLAS_TEXTURE
        );

        @Override
        public void begin(BufferBuilder bufferBuilder, TextureManager textureManager) {
            bufferBuilder.begin(layer.getDrawMode(), layer.getVertexFormat());
        }

        @Override
        public void draw(Tessellator tessellator) {
            layer.draw(tessellator.getBuffer(), VertexSorter.BY_DISTANCE);
        }

        @Override
        public String toString() {
            return "BONG_LINE_EMISSIVE";
        }
    };
}
