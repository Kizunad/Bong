package com.bong.client.ui.contract;

import java.util.Objects;

/** 所有适配器共享的纯逻辑/物理视口元数据。 */
public record UiViewport(int logicalWidth, int logicalHeight, int guiScale, double windowScale) {
    public static final int MIN_SUPPORTED_WIDTH = 320;
    public static final int MIN_SUPPORTED_HEIGHT = 240;

    public UiViewport {
        if (logicalWidth <= 0 || logicalHeight <= 0) {
            throw new IllegalArgumentException("logical viewport dimensions must be positive");
        }
        if (guiScale <= 0) {
            throw new IllegalArgumentException("guiScale must be positive");
        }
        if (!Double.isFinite(windowScale) || windowScale <= 0.0d) {
            throw new IllegalArgumentException("windowScale must be finite and positive");
        }
    }

    public boolean belowMinimum() {
        return logicalWidth < MIN_SUPPORTED_WIDTH || logicalHeight < MIN_SUPPORTED_HEIGHT;
    }

    public Mode mode() {
        if (logicalWidth < 640 || logicalHeight < 360) {
            return Mode.COMPACT;
        }
        if (logicalWidth >= 1280 || logicalHeight >= 720) {
            return Mode.WIDE;
        }
        return Mode.REGULAR;
    }

    public Rect safeRect(double horizontalMargin, double verticalMargin) {
        requireMargin(horizontalMargin, "horizontalMargin");
        requireMargin(verticalMargin, "verticalMargin");
        double x = Math.min(horizontalMargin, logicalWidth / 2.0d);
        double y = Math.min(verticalMargin, logicalHeight / 2.0d);
        return new Rect(x, y, logicalWidth - x * 2.0d, logicalHeight - y * 2.0d);
    }

    public Point physicalToLogical(Point physical) {
        Objects.requireNonNull(physical, "physical point must not be null");
        return new Point(physical.x() / windowScale, physical.y() / windowScale);
    }

    public Point logicalToPhysical(Point logical) {
        Objects.requireNonNull(logical, "logical point must not be null");
        return new Point(logical.x() * windowScale, logical.y() * windowScale);
    }

    private static void requireMargin(double margin, String name) {
        if (!Double.isFinite(margin) || margin < 0.0d) {
            throw new IllegalArgumentException(name + " must be finite and non-negative");
        }
    }

    public enum Mode {
        COMPACT,
        REGULAR,
        WIDE
    }

    public record Point(double x, double y) {
        public Point {
            if (!Double.isFinite(x) || !Double.isFinite(y)) {
                throw new IllegalArgumentException("point coordinates must be finite");
            }
        }
    }

    public record Rect(double x, double y, double width, double height) {
        public Rect {
            if (!Double.isFinite(x) || !Double.isFinite(y)
                || !Double.isFinite(width) || !Double.isFinite(height)
                || width < 0.0d || height < 0.0d) {
                throw new IllegalArgumentException("rect coordinates must be finite and dimensions non-negative");
            }
        }

        public double right() {
            return x + width;
        }

        public double bottom() {
            return y + height;
        }

        public Rect expand(double padding) {
            if (!Double.isFinite(padding) || padding < 0.0d) {
                throw new IllegalArgumentException("padding must be finite and non-negative");
            }
            return new Rect(x - padding, y - padding, width + padding * 2.0d, height + padding * 2.0d);
        }

        public Rect intersection(Rect other) {
            Objects.requireNonNull(other, "other rect must not be null");
            double left = Math.max(x, other.x);
            double top = Math.max(y, other.y);
            double right = Math.min(right(), other.right());
            double bottom = Math.min(bottom(), other.bottom());
            return new Rect(left, top, Math.max(0.0d, right - left), Math.max(0.0d, bottom - top));
        }

        public boolean contains(Point point) {
            Objects.requireNonNull(point, "point must not be null");
            return point.x() >= x && point.x() <= right()
                && point.y() >= y && point.y() <= bottom();
        }
    }
}
