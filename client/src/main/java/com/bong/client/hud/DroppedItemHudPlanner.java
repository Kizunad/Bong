package com.bong.client.hud;

import com.bong.client.inventory.state.DroppedItemStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.Vec3d;

import java.util.List;
import java.util.Locale;

public final class DroppedItemHudPlanner {
    private static final int ICON_SIZE = 14;
    private static final int EDGE_ACCENT_THICKNESS = 2;
    private static final int TEXT_COLOR = 0xFFD8D8B0;
    private static final int BACKGROUND_COLOR = 0x880F1116;
    private static final int EDGE_ACCENT_COLOR = 0x90D8D8B0;
    private static final int SCREEN_PADDING = 6;
    private static final int INNER_PADDING_X = 4;
    private static final int INNER_PADDING_Y = 3;
    private static final int LABEL_GAP = 4;
    private static final int MARKER_VERTICAL_OFFSET = 24;
    private static final double MARKER_WORLD_HEIGHT = 0.35;
    private static final double MIN_DEPTH = 0.05;
    private static final double MIN_VECTOR_LENGTH_SQ = 0.000001;
    private static final double POSITION_DEADZONE_PX = 1.25;
    private static final double POSITION_LERP_FACTOR = 0.35;
    private static final double POSITION_SNAP_DISTANCE_PX = 24.0;
    private static final MarkerStabilityState SHARED_STABILITY_STATE = new MarkerStabilityState();

    private DroppedItemHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(
            widthMeasurer,
            maxWidth,
            screenWidth,
            screenHeight,
            captureProjectionContext(),
            SHARED_STABILITY_STATE
        );
    }

    static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int screenWidth,
        int screenHeight,
        ProjectionContext context
    ) {
        return buildCommands(widthMeasurer, maxWidth, screenWidth, screenHeight, context, SHARED_STABILITY_STATE);
    }

    static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int screenWidth,
        int screenHeight,
        ProjectionContext context,
        MarkerStabilityState stabilityState
    ) {
        if (widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0 || context == null || !context.isUsable()) {
            if (stabilityState != null) {
                stabilityState.clear();
            }
            return List.of();
        }

        DroppedItemStore.Entry nearest = DroppedItemStore.nearestTo(
            context.playerPos().x,
            context.playerPos().y,
            context.playerPos().z
        );
        if (nearest == null || nearest.item() == null) {
            if (stabilityState != null) {
                stabilityState.clear();
            }
            return List.of();
        }

        MarkerLayout layout = layoutMarker(nearest, widthMeasurer, maxWidth, screenWidth, screenHeight, context, stabilityState);
        if (layout == null) {
            if (stabilityState != null) {
                stabilityState.clear();
            }
            return List.of();
        }

        // "只画 icon" 策略：layout 计算（投影 + 边缘 clamp + stabilize）保留，
        // 但 emit 阶段不再绘制 background rect / edge accent / 文字标签——
        // 仅一个小图标浮在物品上方，不会有"标签位置乱飘"之类的视觉噪音。
        return List.of(HudRenderCommand.itemTexture(
            HudRenderLayer.BASELINE,
            nearest.item().itemId(),
            layout.iconX(),
            layout.iconY(),
            ICON_SIZE
        ));
    }

    static ProjectionContext captureProjectionContext() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null) {
            return null;
        }

        Vec3d playerPos = new Vec3d(client.player.getX(), client.player.getY(), client.player.getZ());
        Vec3d cameraPos = client.player.getCameraPosVec(1.0f);
        Vec3d forward = client.player.getRotationVec(1.0f);
        if (forward.lengthSquared() <= MIN_VECTOR_LENGTH_SQ) {
            return null;
        }
        forward = forward.normalize();

        Vec3d right = forward.crossProduct(new Vec3d(0.0, 1.0, 0.0));
        if (right.lengthSquared() <= MIN_VECTOR_LENGTH_SQ) {
            right = new Vec3d(1.0, 0.0, 0.0);
        } else {
            right = right.normalize();
        }

        Vec3d up = right.crossProduct(forward);
        if (up.lengthSquared() <= MIN_VECTOR_LENGTH_SQ) {
            up = new Vec3d(0.0, 1.0, 0.0);
        } else {
            up = up.normalize();
        }

        double fovDegrees = client.options.getFov().getValue();
        if (!Double.isFinite(fovDegrees) || fovDegrees <= 1.0) {
            return null;
        }

        return new ProjectionContext(playerPos, cameraPos, forward, right, up, fovDegrees);
    }

    private static MarkerLayout layoutMarker(
        DroppedItemStore.Entry entry,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int screenWidth,
        int screenHeight,
        ProjectionContext context,
        MarkerStabilityState stabilityState
    ) {
        Vec3d liftedWorldPos = new Vec3d(entry.worldPosX(), entry.worldPosY() + MARKER_WORLD_HEIGHT, entry.worldPosZ());
        Vec3d relative = liftedWorldPos.subtract(context.cameraPos());
        double depth = relative.dotProduct(context.forward());
        if (depth <= MIN_DEPTH) {
            return null;
        }

        double cameraX = relative.dotProduct(context.right());
        double cameraY = relative.dotProduct(context.up());
        double halfScreenWidth = screenWidth / 2.0;
        double halfScreenHeight = screenHeight / 2.0;
        double focalLength = halfScreenWidth / Math.tan(Math.toRadians(context.fovDegrees()) / 2.0);
        if (!Double.isFinite(focalLength) || focalLength <= 0.0) {
            return null;
        }

        double projectedX = halfScreenWidth + (cameraX * focalLength / depth);
        double projectedY = halfScreenHeight - (cameraY * focalLength / depth);
        int maxLabelWidth = Math.max(
            24,
            Math.min(maxWidth, Math.max(24, screenWidth - SCREEN_PADDING * 2))
                - ICON_SIZE
                - LABEL_GAP
                - INNER_PADDING_X * 2
        );
        String baseLabel = buildBaseLabel(entry, context.playerPos());
        String clippedBaseLabel = HudTextHelper.clipToWidth(baseLabel, maxLabelWidth, widthMeasurer);
        int baseTextWidth = Math.max(0, widthMeasurer.measure(clippedBaseLabel));
        int backgroundWidth = INNER_PADDING_X * 2 + ICON_SIZE + LABEL_GAP + baseTextWidth;
        int backgroundHeight = INNER_PADDING_Y * 2 + ICON_SIZE;

        int minBackgroundX = SCREEN_PADDING;
        int maxBackgroundX = Math.max(SCREEN_PADDING, screenWidth - SCREEN_PADDING - backgroundWidth);
        int minBackgroundY = SCREEN_PADDING;
        int maxBackgroundY = Math.max(SCREEN_PADDING, screenHeight - SCREEN_PADDING - backgroundHeight);
        double rawBackgroundX = projectedX - backgroundWidth / 2.0;
        double rawBackgroundY = projectedY - MARKER_VERTICAL_OFFSET - backgroundHeight / 2.0;
        boolean clampedLeft = rawBackgroundX < minBackgroundX;
        boolean clampedRight = rawBackgroundX > maxBackgroundX;
        boolean clampedTop = rawBackgroundY < minBackgroundY;
        boolean clampedBottom = rawBackgroundY > maxBackgroundY;
        int overflowLeft = clampedLeft ? (int) Math.ceil(minBackgroundX - rawBackgroundX) : 0;
        int overflowRight = clampedRight ? (int) Math.ceil(rawBackgroundX - maxBackgroundX) : 0;
        int overflowTop = clampedTop ? (int) Math.ceil(minBackgroundY - rawBackgroundY) : 0;
        int overflowBottom = clampedBottom ? (int) Math.ceil(rawBackgroundY - maxBackgroundY) : 0;
        String directionalLabel = buildDirectionalLabel(
            baseLabel,
            clampedLeft,
            clampedRight,
            clampedTop,
            clampedBottom,
            overflowLeft,
            overflowRight,
            overflowTop,
            overflowBottom
        );
        String clippedLabel = HudTextHelper.clipToWidth(directionalLabel, maxLabelWidth, widthMeasurer);

        int clampMask = clampMask(clampedLeft, clampedRight, clampedTop, clampedBottom);
        StabilizedPosition stabilized = stabilityState == null
            ? new StabilizedPosition(rawBackgroundX, rawBackgroundY)
            : stabilityState.stabilize(entry.instanceId(), clampMask, rawBackgroundX, rawBackgroundY);
        int backgroundX = clamp((int) Math.round(stabilized.backgroundX()), minBackgroundX, maxBackgroundX);
        int backgroundY = clamp((int) Math.round(stabilized.backgroundY()), minBackgroundY, maxBackgroundY);
        int iconX = backgroundX + INNER_PADDING_X;
        int iconY = backgroundY + INNER_PADDING_Y;
        int textX = iconX + ICON_SIZE + LABEL_GAP;
        int textY = backgroundY + INNER_PADDING_Y + 3;

        return new MarkerLayout(
            backgroundX,
            backgroundY,
            backgroundWidth,
            backgroundHeight,
            iconX,
            iconY,
            textX,
            textY,
            clippedLabel,
            clampedLeft,
            clampedRight,
            clampedTop,
            clampedBottom
        );
    }

    private static void appendEdgeAccent(List<HudRenderCommand> commands, MarkerLayout layout) {
        if (layout.clampedLeft()) {
            commands.add(HudRenderCommand.rect(
                HudRenderLayer.BASELINE,
                layout.backgroundX(),
                layout.backgroundY(),
                EDGE_ACCENT_THICKNESS,
                layout.backgroundHeight(),
                EDGE_ACCENT_COLOR
            ));
        }
        if (layout.clampedRight()) {
            commands.add(HudRenderCommand.rect(
                HudRenderLayer.BASELINE,
                layout.backgroundX() + layout.backgroundWidth() - EDGE_ACCENT_THICKNESS,
                layout.backgroundY(),
                EDGE_ACCENT_THICKNESS,
                layout.backgroundHeight(),
                EDGE_ACCENT_COLOR
            ));
        }
        if (layout.clampedTop()) {
            commands.add(HudRenderCommand.rect(
                HudRenderLayer.BASELINE,
                layout.backgroundX(),
                layout.backgroundY(),
                layout.backgroundWidth(),
                EDGE_ACCENT_THICKNESS,
                EDGE_ACCENT_COLOR
            ));
        }
        if (layout.clampedBottom()) {
            commands.add(HudRenderCommand.rect(
                HudRenderLayer.BASELINE,
                layout.backgroundX(),
                layout.backgroundY() + layout.backgroundHeight() - EDGE_ACCENT_THICKNESS,
                layout.backgroundWidth(),
                EDGE_ACCENT_THICKNESS,
                EDGE_ACCENT_COLOR
            ));
        }
    }

    private static String buildBaseLabel(DroppedItemStore.Entry entry, Vec3d playerPos) {
        double distance = Math.sqrt(playerPos.squaredDistanceTo(entry.worldPosX(), entry.worldPosY(), entry.worldPosZ()));
        return String.format(Locale.ROOT, "%s · %.1fm", entry.item().displayName(), distance);
    }

    private static String buildDirectionalLabel(
        String baseLabel,
        boolean clampedLeft,
        boolean clampedRight,
        boolean clampedTop,
        boolean clampedBottom,
        int overflowLeft,
        int overflowRight,
        int overflowTop,
        int overflowBottom
    ) {
        String prefix = directionalPrefix(
            clampedLeft,
            clampedRight,
            clampedTop,
            clampedBottom,
            overflowLeft,
            overflowRight,
            overflowTop,
            overflowBottom
        );
        if (prefix.isEmpty()) {
            return baseLabel;
        }
        return prefix + " " + baseLabel;
    }

    private static int clampMask(
        boolean clampedLeft,
        boolean clampedRight,
        boolean clampedTop,
        boolean clampedBottom
    ) {
        int mask = 0;
        if (clampedLeft) mask |= 1;
        if (clampedRight) mask |= 1 << 1;
        if (clampedTop) mask |= 1 << 2;
        if (clampedBottom) mask |= 1 << 3;
        return mask;
    }

    private static String directionalPrefix(
        boolean clampedLeft,
        boolean clampedRight,
        boolean clampedTop,
        boolean clampedBottom,
        int overflowLeft,
        int overflowRight,
        int overflowTop,
        int overflowBottom
    ) {
        boolean horizontalClamped = clampedLeft || clampedRight;
        boolean verticalClamped = clampedTop || clampedBottom;
        if (horizontalClamped && verticalClamped) {
            int horizontalOverflow = Math.max(overflowLeft, overflowRight);
            int verticalOverflow = Math.max(overflowTop, overflowBottom);
            if (verticalOverflow > horizontalOverflow) {
                if (clampedTop) {
                    return "↑";
                }
                if (clampedBottom) {
                    return "↓";
                }
            }
        }
        if (clampedLeft) {
            return "←";
        }
        if (clampedRight) {
            return "→";
        }
        if (clampedTop) {
            return "↑";
        }
        if (clampedBottom) {
            return "↓";
        }
        return "";
    }

    private static int clamp(int value, int min, int max) {
        if (max < min) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }

    static void resetForTests() {
        SHARED_STABILITY_STATE.clear();
    }

    record ProjectionContext(
        Vec3d playerPos,
        Vec3d cameraPos,
        Vec3d forward,
        Vec3d right,
        Vec3d up,
        double fovDegrees
    ) {
        boolean isUsable() {
            return playerPos != null
                && cameraPos != null
                && forward != null
                && right != null
                && up != null
                && forward.lengthSquared() > MIN_VECTOR_LENGTH_SQ
                && right.lengthSquared() > MIN_VECTOR_LENGTH_SQ
                && up.lengthSquared() > MIN_VECTOR_LENGTH_SQ
                && Double.isFinite(fovDegrees)
                && fovDegrees > 1.0;
        }
    }

    private record MarkerLayout(
        int backgroundX,
        int backgroundY,
        int backgroundWidth,
        int backgroundHeight,
        int iconX,
        int iconY,
        int textX,
        int textY,
        String label,
        boolean clampedLeft,
        boolean clampedRight,
        boolean clampedTop,
        boolean clampedBottom
    ) {}

    static final class MarkerStabilityState {
        private boolean initialized;
        private long instanceId = -1L;
        private int clampMask;
        private double backgroundX;
        private double backgroundY;

        StabilizedPosition stabilize(long targetInstanceId, int targetClampMask, double targetBackgroundX, double targetBackgroundY) {
            if (!initialized || instanceId != targetInstanceId || clampMask != targetClampMask) {
                initialized = true;
                instanceId = targetInstanceId;
                clampMask = targetClampMask;
                backgroundX = targetBackgroundX;
                backgroundY = targetBackgroundY;
                return new StabilizedPosition(backgroundX, backgroundY);
            }

            backgroundX = stabilizeAxis(backgroundX, targetBackgroundX);
            backgroundY = stabilizeAxis(backgroundY, targetBackgroundY);
            return new StabilizedPosition(backgroundX, backgroundY);
        }

        void clear() {
            initialized = false;
            instanceId = -1L;
            clampMask = 0;
            backgroundX = 0.0;
            backgroundY = 0.0;
        }

        private static double stabilizeAxis(double current, double target) {
            double delta = target - current;
            double magnitude = Math.abs(delta);
            if (magnitude <= POSITION_DEADZONE_PX) {
                return current;
            }
            if (magnitude >= POSITION_SNAP_DISTANCE_PX) {
                return target;
            }
            return current + delta * POSITION_LERP_FACTOR;
        }
    }

    private record StabilizedPosition(double backgroundX, double backgroundY) {}
}
