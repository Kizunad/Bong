package com.bong.client.ui.window;

import java.util.Objects;
import java.util.Set;

/** 普通窗口的稳定声明，隔离窗口策略与 owo/Fabric 细节。 */
public record UiWindowDefinition(
    String windowType,
    String templateId,
    int minimumWidth,
    int minimumHeight,
    Set<Capability> capabilities
) {
    public UiWindowDefinition {
        windowType = requireText(windowType, "windowType");
        templateId = requireText(templateId, "templateId");
        if (minimumWidth <= 0 || minimumHeight <= 0) {
            throw new IllegalArgumentException("minimum window size must be positive");
        }
        capabilities = capabilities == null ? Set.of() : Set.copyOf(capabilities);
    }

    public boolean supports(Capability capability) {
        return capabilities.contains(Objects.requireNonNull(capability, "capability"));
    }

    private static String requireText(String value, String name) {
        Objects.requireNonNull(value, name + " must not be null");
        String normalized = value.strip();
        if (normalized.isEmpty()) {
            throw new IllegalArgumentException(name + " must not be blank");
        }
        return normalized;
    }

    public enum Capability {
        WINDOW,
        /** 工位窗口可以管理布局，但不能固定到 HUD。 */
        STATION,
        OFFER,
        SYSTEM
    }
}
