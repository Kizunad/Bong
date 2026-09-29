package com.bong.client.alchemy;

/** 服务端结算对应的短暂表现；不参与成丹判定或伤害结算。 */
public enum AlchemyResultEffect {
    COMPLETE("complete", 24),
    FLAWED("flawed", 26),
    WASTE("waste", 28),
    EXPLODE("explode", 36),
    EARLY_TAKE("early_take", 24);

    private final String animation;
    private final int ticks;

    AlchemyResultEffect(String animation, int ticks) {
        this.animation = "animation.bong.alchemy_furnace." + animation;
        this.ticks = ticks;
    }

    public String animation() { return animation; }
    public int ticks() { return ticks; }

    public static AlchemyResultEffect fromOutcome(String bucket, boolean early) {
        return switch (bucket) {
            case "perfect", "good" -> COMPLETE;
            case "flawed" -> FLAWED;
            case "waste" -> early ? EARLY_TAKE : WASTE;
            case "explode" -> EXPLODE;
            default -> null;
        };
    }
}
