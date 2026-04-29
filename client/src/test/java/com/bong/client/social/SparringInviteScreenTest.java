package com.bong.client.social;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class SparringInviteScreenTest {
    @Test
    void describeShowsAnonymousTermsAndCountdown() {
        SocialStateStore.SparringInvite invite = new SocialStateStore.SparringInvite(
            "sparring:1:a:b",
            "char:a",
            "char:b",
            "condense_solidify",
            "气息相试",
            "点到为止",
            1234L
        );

        SparringInviteScreen.RenderContent content = SparringInviteScreen.describe(invite, 9_000L);

        assertTrue(content.lines().contains("发起者气息: 气息相试"));
        assertTrue(content.lines().contains("境界段: condense_solidify"));
        assertTrue(content.lines().contains("条款: 点到为止"));
        assertTrue(content.lines().contains("倒计时: 9s"));
        assertTrue(content.lines().contains("失败方: 5min 谦抑, 真元回复 -30%"));
    }
}
