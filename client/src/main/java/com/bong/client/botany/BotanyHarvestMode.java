package com.bong.client.botany;

public enum BotanyHarvestMode {
    MANUAL("manual", "手动采集", "E"),
    AUTO("auto", "自动采集", "R");

    private final String wireName;
    private final String displayName;
    private final String keyLabel;

    BotanyHarvestMode(String wireName, String displayName, String keyLabel) {
        this.wireName = wireName;
        this.displayName = displayName;
        this.keyLabel = keyLabel;
    }

    public String wireName() {
        return wireName;
    }

    public String displayName() {
        return displayName;
    }

    public String keyLabel() {
        return keyLabel;
    }

    public static BotanyHarvestMode fromWireName(String value) {
        if (value == null) {
            return null;
        }
        for (BotanyHarvestMode mode : values()) {
            if (mode.wireName.equalsIgnoreCase(value.trim())) {
                return mode;
            }
        }
        return null;
    }
}
