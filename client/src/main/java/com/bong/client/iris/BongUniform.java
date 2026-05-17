package com.bong.client.iris;

import java.util.Locale;

public enum BongUniform {
    REALM,
    LINGQI,
    TRIBULATION,
    ENLIGHTENMENT,
    INKWASH,
    BLOODMOON,
    MEDITATION,
    DEMONIC,
    WIND_STRENGTH,
    WIND_ANGLE;

    private final String shaderName;

    BongUniform() {
        this.shaderName = "bong_" + name().toLowerCase(Locale.ROOT);
    }

    public String shaderName() {
        return shaderName;
    }

    public static BongUniform fromShaderName(String name) {
        for (BongUniform u : values()) {
            if (u.shaderName.equals(name)) {
                return u;
            }
        }
        return null;
    }
}
