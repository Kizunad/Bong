package com.bong.client.environment;

import net.minecraft.util.math.Vec3d;

public sealed interface EnvironmentEffect
    permits EnvironmentEffect.AshFall,
        EnvironmentEffect.DustDevil,
        EnvironmentEffect.EmberDrift,
        EnvironmentEffect.FogVeil,
        EnvironmentEffect.HeatHaze,
        EnvironmentEffect.LightningPillar,
        EnvironmentEffect.SnowDrift,
        EnvironmentEffect.TornadoColumn {

    String kind();

    Vec3d anchor();

    default double viewRadius() {
        return 80.0;
    }

    default int fadeInTicks() {
        return 40;
    }

    default int fadeOutTicks() {
        return 40;
    }

    default String ambientLoopRecipe() {
        return null;
    }

    String stableKey();

    boolean contains(Vec3d pos);

    default boolean isNear(Vec3d pos, double radius) {
        double limit = Math.max(0.0, radius);
        return anchor().squaredDistanceTo(pos) <= limit * limit || contains(pos);
    }

    default double cullRadius() {
        return viewRadius();
    }

    default double renderIntensity() {
        return 1.0;
    }

    record TornadoColumn(
        double centerX,
        double centerY,
        double centerZ,
        double radius,
        double height,
        double particleDensity
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "tornado_column";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d(centerX, centerY, centerZ);
        }

        @Override
        public String ambientLoopRecipe() {
            return "wind_howl_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + centerX + "," + centerY + "," + centerZ + "," + radius + "," + height;
        }

        @Override
        public double renderIntensity() {
            return particleDensity;
        }

        @Override
        public boolean contains(Vec3d pos) {
            double dx = pos.x - centerX;
            double dz = pos.z - centerZ;
            return dx * dx + dz * dz <= radius * radius
                && pos.y >= centerY
                && pos.y <= centerY + height;
        }
    }

    record LightningPillar(
        double centerX,
        double centerY,
        double centerZ,
        double radius,
        double strikeRatePerMin
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "lightning_pillar";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d(centerX, centerY, centerZ);
        }

        @Override
        public String ambientLoopRecipe() {
            return "thunder_distant_loop";
        }

        @Override
        public int fadeInTicks() {
            return 20;
        }

        @Override
        public int fadeOutTicks() {
            return 20;
        }

        @Override
        public String stableKey() {
            return kind() + ":" + centerX + "," + centerY + "," + centerZ + "," + radius;
        }

        @Override
        public double renderIntensity() {
            return Math.max(0.25, strikeRatePerMin / 2.0);
        }

        @Override
        public boolean contains(Vec3d pos) {
            double dx = pos.x - centerX;
            double dz = pos.z - centerZ;
            return dx * dx + dz * dz <= radius * radius;
        }
    }

    record AshFall(
        double minX,
        double minY,
        double minZ,
        double maxX,
        double maxY,
        double maxZ,
        double density
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "ash_fall";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d((minX + maxX) * 0.5, (minY + maxY) * 0.5, (minZ + maxZ) * 0.5);
        }

        @Override
        public String ambientLoopRecipe() {
            return "static_crackle_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + minX + "," + minY + "," + minZ + "," + maxX + "," + maxY + "," + maxZ;
        }

        @Override
        public double renderIntensity() {
            return density;
        }

        @Override
        public boolean contains(Vec3d pos) {
            return pos.x >= minX
                && pos.x <= maxX
                && pos.y >= minY
                && pos.y <= maxY
                && pos.z >= minZ
                && pos.z <= maxZ;
        }

        @Override
        public double cullRadius() {
            return anchor().distanceTo(new Vec3d(maxX, maxY, maxZ));
        }
    }

    record FogVeil(
        double minX,
        double minY,
        double minZ,
        double maxX,
        double maxY,
        double maxZ,
        int tintRgb,
        double density
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "fog_veil";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d((minX + maxX) * 0.5, (minY + maxY) * 0.5, (minZ + maxZ) * 0.5);
        }

        @Override
        public String ambientLoopRecipe() {
            return "mist_low_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + minX + "," + minY + "," + minZ + "," + maxX + "," + maxY + "," + maxZ;
        }

        @Override
        public double renderIntensity() {
            return density;
        }

        @Override
        public boolean contains(Vec3d pos) {
            return pos.x >= minX
                && pos.x <= maxX
                && pos.y >= minY
                && pos.y <= maxY
                && pos.z >= minZ
                && pos.z <= maxZ;
        }

        @Override
        public double cullRadius() {
            return anchor().distanceTo(new Vec3d(maxX, maxY, maxZ));
        }
    }

    record DustDevil(
        double centerX,
        double centerY,
        double centerZ,
        double radius,
        double height
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "dust_devil";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d(centerX, centerY, centerZ);
        }

        @Override
        public String ambientLoopRecipe() {
            return "wind_dry_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + centerX + "," + centerY + "," + centerZ + "," + radius + "," + height;
        }

        @Override
        public boolean contains(Vec3d pos) {
            double dx = pos.x - centerX;
            double dz = pos.z - centerZ;
            return dx * dx + dz * dz <= radius * radius
                && pos.y >= centerY
                && pos.y <= centerY + height;
        }
    }

    record EmberDrift(
        double minX,
        double minY,
        double minZ,
        double maxX,
        double maxY,
        double maxZ,
        double density,
        double glow
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "ember_drift";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d((minX + maxX) * 0.5, (minY + maxY) * 0.5, (minZ + maxZ) * 0.5);
        }

        @Override
        public String ambientLoopRecipe() {
            return "static_crackle_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + minX + "," + minY + "," + minZ + "," + maxX + "," + maxY + "," + maxZ;
        }

        @Override
        public double renderIntensity() {
            return Math.max(density, glow);
        }

        @Override
        public boolean contains(Vec3d pos) {
            return pos.x >= minX
                && pos.x <= maxX
                && pos.y >= minY
                && pos.y <= maxY
                && pos.z >= minZ
                && pos.z <= maxZ;
        }

        @Override
        public double cullRadius() {
            return anchor().distanceTo(new Vec3d(maxX, maxY, maxZ));
        }
    }

    record HeatHaze(
        double minX,
        double minY,
        double minZ,
        double maxX,
        double maxY,
        double maxZ,
        double distortionStrength
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "heat_haze";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d((minX + maxX) * 0.5, (minY + maxY) * 0.5, (minZ + maxZ) * 0.5);
        }

        @Override
        public String ambientLoopRecipe() {
            return "cicada_summer_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + minX + "," + minY + "," + minZ + "," + maxX + "," + maxY + "," + maxZ;
        }

        @Override
        public double renderIntensity() {
            return distortionStrength;
        }

        @Override
        public boolean contains(Vec3d pos) {
            return pos.x >= minX
                && pos.x <= maxX
                && pos.y >= minY
                && pos.y <= maxY
                && pos.z >= minZ
                && pos.z <= maxZ;
        }

        @Override
        public double cullRadius() {
            return anchor().distanceTo(new Vec3d(maxX, maxY, maxZ));
        }
    }

    record SnowDrift(
        double minX,
        double minY,
        double minZ,
        double maxX,
        double maxY,
        double maxZ,
        double density,
        double windX,
        double windY,
        double windZ
    ) implements EnvironmentEffect {
        @Override
        public String kind() {
            return "snow_drift";
        }

        @Override
        public Vec3d anchor() {
            return new Vec3d((minX + maxX) * 0.5, (minY + maxY) * 0.5, (minZ + maxZ) * 0.5);
        }

        @Override
        public String ambientLoopRecipe() {
            return "wind_cold_loop";
        }

        @Override
        public String stableKey() {
            return kind() + ":" + minX + "," + minY + "," + minZ + "," + maxX + "," + maxY + "," + maxZ;
        }

        @Override
        public double renderIntensity() {
            return density;
        }

        @Override
        public boolean contains(Vec3d pos) {
            return pos.x >= minX
                && pos.x <= maxX
                && pos.y >= minY
                && pos.y <= maxY
                && pos.z >= minZ
                && pos.z <= maxZ;
        }

        @Override
        public double cullRadius() {
            return anchor().distanceTo(new Vec3d(maxX, maxY, maxZ));
        }
    }
}
