package com.bong.client.ui.model;

import com.mojang.brigadier.tree.CommandNode;

/** Bong 服务端按 bong.dev scope 过滤命令树；give 始终注册且只下发给 OP。 */
public final class ModelPreviewAccess {
    private ModelPreviewAccess() {}

    public static boolean allowed(CommandNode<?> serverRoot) {
        // 不使用创造模式或本地用户名推断权限。未收到服务端树时默认拒绝。
        return serverRoot != null && serverRoot.getChild("give") != null;
    }
}
