package com.bong.client.craft;

/** 制作入口的上下文；工位身份只在本次连接内有效，不写入布局偏好。 */
public record CraftContext(Workbench workbench) {
    public static final CraftContext HANDCRAFT = new CraftContext(null);

    public boolean accepts(CraftRecipe recipe) {
        return workbench == null ? recipe.isHandcraft() : recipe.isWorkbenchRecipe();
    }

    public String title() {
        return workbench == null ? "制作" : "制作 · 工位";
    }

    public record Workbench(int entityId, int x, int y, int z) {}
}
