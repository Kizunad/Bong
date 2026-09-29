/** 从已验收的作者模型导出丹炉；骨架和贴图不另造一份。 */
const fs = require("node:fs");
const path = require("node:path");
const { convert, formatJson } = require("./creature_codec.cjs");

const root = path.resolve(__dirname, "../..");
const source = JSON.parse(fs.readFileSync(path.join(root, "modelScript/models/AlchemyFurnace.bbmodel"), "utf8"));
const { geometry } = convert(source, "alchemy_furnace");
const bones = new Set(geometry["minecraft:geometry"][0].bones.map(bone => bone.name));
const bodyBones = ["group_lower_bowl", "group_hearth_core", "group_outer_shell", "group_brass_collar", "group_side_handles"];
for (const name of ["group_top_cap", ...bodyBones]) {
    if (!bones.has(name)) throw new Error(`作者模型缺少动画骨骼 ${name}`);
}

// 炉脚不动；各层共用极小位移，让炉身在原位振动，不改变作者骨架。
function bodyMotion(position) {
    return Object.fromEntries(bodyBones.map(name => [name, { position }]));
}

const simmer = ["math.sin(query.anim_time * 1440) * 0.045", 0, "math.cos(query.anim_time * 1080) * 0.03"];
const shock = {
    "0": [0, 0, 0], "0.08": [-0.5, 0.15, 0], "0.16": [0.6, 0.3, 0],
    "0.24": [-0.4, 0.15, 0], "0.36": [0.25, 0, 0], "0.55": [-0.1, 0, 0], "0.8": [0, 0, 0],
};

const animation = {
    format_version: "1.8.0",
    animations: {
        "animation.bong.alchemy_furnace.idle": { loop: true, bones: {} },
        "animation.bong.alchemy_furnace.brewing": {
            loop: true,
            animation_length: 2,
            bones: {
                ...bodyMotion(simmer),
                group_top_cap: {
                    position: ["math.sin(query.anim_time * 1440) * 0.07", "0.18 + math.sin(query.anim_time * 1080) * 0.1", 0],
                    rotation: [0, 0, "math.sin(query.anim_time * 1440) * 0.45"],
                },
                group_hearth_core: { position: simmer, scale: "1 + math.sin(query.anim_time * 360) * 0.025" },
            },
        },
        "animation.bong.alchemy_furnace.open": {
            animation_length: 0.5,
            bones: { group_top_cap: { position: { "0": [0, 0, 0], "0.25": [0, 3, 0], "0.5": [0, 0.4, 0] } } },
        },
        "animation.bong.alchemy_furnace.close": {
            animation_length: 0.5,
            bones: { group_top_cap: { position: { "0": [0, 0.4, 0], "0.25": [0, 3, 0], "0.5": [0, 0, 0] } } },
        },
        "animation.bong.alchemy_furnace.feed": {
            animation_length: 0.8,
            bones: {
                group_top_cap: {
                    position: { "0": [0, 0.4, 0], "0.2": [0, 5, 0], "0.5": [0, 5, 0], "0.8": [0, 0.4, 0] },
                    rotation: { "0": [0, 0, 0], "0.2": [-12, 0, 0], "0.5": [-12, 0, 0], "0.8": [0, 0, 0] },
                },
            },
        },
        "animation.bong.alchemy_furnace.complete": {
            animation_length: 1.2,
            bones: { group_top_cap: { position: { "0": [0, 0, 0], "0.3": [0, 2.5, 0], "0.8": [0, 2.5, 0], "1.2": [0, 0, 0] } } },
        },
        "animation.bong.alchemy_furnace.early_take": {
            animation_length: 1.2,
            bones: {
                group_top_cap: {
                    position: { "0": [0, 0.4, 0], "0.15": [0, 4, 0], "0.6": [0, 4, 0], "1.2": [0, 0, 0] },
                    rotation: { "0": [0, 0, 0], "0.15": [0, 0, 8], "0.6": [0, 0, 8], "1.2": [0, 0, 0] },
                },
            },
        },
        "animation.bong.alchemy_furnace.flawed": {
            animation_length: 1.3,
            bones: {
                group_top_cap: { position: { "0": [0, 0, 0], "0.2": [0, 1.8, 0], "0.4": [0, 0.3, 0], "0.65": [0, 1.2, 0], "1.3": [0, 0, 0] } },
            },
        },
        "animation.bong.alchemy_furnace.waste": {
            animation_length: 1.4,
            bones: {
                group_top_cap: { position: { "0": [0, 0.4, 0], "0.2": [0, 1.4, 0], "0.9": [0, 1.4, 0], "1.4": [0, 0, 0] } },
                group_hearth_core: { scale: { "0": [1, 1, 1], "0.25": [1.04, 1.02, 1.04], "0.6": [0.97, 0.97, 0.97], "1.4": [1, 1, 1] } },
            },
        },
        "animation.bong.alchemy_furnace.explode": {
            animation_length: 1.8,
            bones: {
                ...bodyMotion(shock),
                group_top_cap: {
                    position: { "0": [0, 0, 0], "0.12": [0, 4, 0], "0.4": [1.5, 9, 0], "0.75": [0.5, 7, 0], "1.25": [0, 0.5, 0], "1.5": [0, 1, 0], "1.8": [0, 0, 0] },
                    rotation: { "0": [0, 0, 0], "0.4": [0, 0, 28], "0.75": [0, 0, -15], "1.25": [0, 0, 4], "1.8": [0, 0, 0] },
                },
            },
        },
    },
};

if (source.textures.length !== 1 || !source.textures[0].source.startsWith("data:image/png;base64,")) {
    throw new Error("丹炉导出要求一张嵌入的 PNG 贴图");
}
const texture = Buffer.from(source.textures[0].source.split(",")[1], "base64");
const assets = path.join(root, "client/src/main/resources/assets/bong");
function writeJson(relative, document) {
    fs.writeFileSync(path.join(assets, relative), formatJson(document));
}
writeJson("geo/alchemy_furnace.geo.json", geometry);
writeJson("animations/alchemy_furnace.animation.json", animation);
for (const state of ["idle", "brewing", "tier4"]) {
    fs.writeFileSync(path.join(assets, `textures/entity/alchemy_furnace_${state}.png`), texture);
}
console.log(`已导出丹炉：${bones.size} 根骨骼、${source.elements.length} 个方块、${Object.keys(animation.animations).length} 段动画`);
