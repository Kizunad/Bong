package com.bong.client.insight;

/**
 * 顿悟效果的 7 类白名单 (对应 docs/plan-cultivation-v1.md §5.2)。
 * 客户端只展示用——校验在服务端 Arbiter，客户端不重复实现。
 */
public enum InsightCategory {
    MERIDIAN("A", "经脉", 0xFF6FB7F5),
    QI("B", "真元", 0xFF7AD0A8),
    COMPOSURE("C", "心境", 0xFFE5C77B),
    QI_COLOR("D", "染色", 0xFFC592E0),
    BREAKTHROUGH("E", "突破", 0xFFE57373),
    SCHOOL("F", "流派", 0xFFD2A36C),
    PERCEPTION("G", "感知", 0xFF9CA8B6);

    private final String code;
    private final String displayName;
    private final int accentArgb;

    InsightCategory(String code, String displayName, int accentArgb) {
        this.code = code;
        this.displayName = displayName;
        this.accentArgb = accentArgb;
    }

    public String code() {
        return code;
    }

    public String displayName() {
        return displayName;
    }

    /** 卡片左侧装饰条 / 类别标签的高亮色 (ARGB)。 */
    public int accentArgb() {
        return accentArgb;
    }

    /** 完整标签：「类 E · 突破」。 */
    public String label() {
        return "类 " + code + " · " + displayName;
    }
}
