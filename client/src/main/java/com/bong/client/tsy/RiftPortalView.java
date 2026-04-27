package com.bong.client.tsy;

public record RiftPortalView(
    long entityId,
    String kind,
    String direction,
    String familyId,
    double x,
    double y,
    double z,
    double triggerRadius,
    int currentExtractTicks,
    Long activationWindowEnd
) {
    public boolean isExpired(long serverTick) {
        return activationWindowEnd != null && serverTick > activationWindowEnd;
    }
}
