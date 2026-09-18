package com.bong.client.ui.model;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ModelPreviewAccessTest {
    @Test void onlyServerGrantedOperatorScopeExposesCatalogAndRevocationRemovesAccess() {
        var ordinary = new CommandDispatcher<Object>();
        ordinary.register(LiteralArgumentBuilder.literal("ping"));
        var operator = new CommandDispatcher<Object>();
        operator.register(LiteralArgumentBuilder.literal("give"));
        assertFalse(ModelPreviewAccess.allowed(null), "未收到权限树时必须拒绝");
        assertFalse(ModelPreviewAccess.allowed(ordinary.getRoot()), "公共命令不授予模型审阅权限");
        assertTrue(ModelPreviewAccess.allowed(operator.getRoot()));
        assertFalse(ModelPreviewAccess.allowed(ordinary.getRoot()), "切服或撤权后不能沿用之前的 OP 状态");
    }
}
