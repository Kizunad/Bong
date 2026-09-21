/**
 * 生物流水线的离线导出适配：几何复用仓库已有的 Blockbench Bedrock codec。
 * 只接受这批模型使用的 cuboid + 单值、线性、数值关键帧；其他输入交回 Blockbench。
 *
 * 动画坐标契约来自 GeckoLib 官方插件的 checkAndPatchKeyframeValues：
 * https://github.com/JannisX11/blockbench-plugins/blob/master/plugins/geckolib/geckolib.js
 * blob 569a418e198ddfdd67ba7e57ab82013a54c147fe，rotation 翻 X/Y，position 翻 X。
 * 不读取未经验证的参考 animation.json，也不更改作者 bbmodel。
 */
const fs = require("node:fs");
const path = require("node:path");
const { exportModel } = require("../../client/tools/bbmodel_convert/blockbench_convert.js");

function normalizeModel(source) {
    const model = structuredClone(source);
    const groups = new Map((model.groups || []).map(group => [group.uuid, group]));
    const bones = new Map();
    const names = new Set();
    const assigned = new Set();
    const elements = new Set(model.elements.map(element => element.uuid));
    function inline(node) {
        if (typeof node === "string") {
            if (!elements.has(node) || assigned.has(node)) throw new Error("缺失或重复挂载的 cube");
            assigned.add(node);
            return node;
        }
        const group = { ...(groups.get(node.uuid) || node) };
        if (!group.name || bones.has(group.uuid) || names.has(group.name)) {
            throw new Error("缺少或重复骨名/UUID；请先在 Blockbench 修复分组");
        }
        bones.set(group.uuid, group.name);
        names.add(group.name);
        group.children = (node.children || []).map(inline);
        return group;
    }
    if (model.outliner.some(node => typeof node === "string")) throw new Error("根 cube 需要先分 group");
    model.outliner = model.outliner.map(inline);
    if (assigned.size !== model.elements.length) throw new Error("有 cube 未挂进骨架，导出会丢失几何");
    for (const element of model.elements) {
        if (element.type !== "cube") {
            throw new Error(`离线导出不支持 ${element.type}，请使用 Blockbench 官方导出`);
        }
        if (element.export === false || element.visibility === false) throw new Error("隐藏几何需要先明确导出层");
        if (element.to.some((value, axis) => value < element.from[axis])) {
            throw new Error(`负向方块 ${element.name} 需要先在 Blockbench 整理 UV`);
        }
        for (const face of Object.values(element.faces || {})) {
            if (face.rotation) {
                throw new Error(`旋转 UV ${element.name} 需要 Blockbench 官方导出`);
            }
        }
    }
    return { model, bones };
}

function exportAnimations(source, bones, name) {
    const animations = {};
    for (const animation of source.animations || []) {
        const boneTracks = {};
        for (const [uuid, animator] of Object.entries(animation.animators || {})) {
            if (animator.type !== "bone" || !bones.has(uuid)) {
                throw new Error(`动画 ${animation.name} 包含未知骨或特效轨道`);
            }
            const channels = {};
            for (const keyframe of animator.keyframes || []) {
                const channel = keyframe.channel;
                if (!["rotation", "position", "scale"].includes(channel)
                    || keyframe.interpolation !== "linear"
                    || keyframe.data_points.length !== 1
                    || (keyframe.easing && keyframe.easing !== "linear")) {
                    throw new Error(`${animation.name}/${bones.get(uuid)} 含未烘焙关键帧，请用官方导出`);
                }
                const point = keyframe.data_points[0];
                const vector = [point.x, point.y, point.z].map(value => {
                    if (value === "" || value == null || !Number.isFinite(Number(value))) {
                        throw new Error("离线导出只接受有限数值，不解释 Molang");
                    }
                    return Number(value);
                });
                if (channel === "rotation" || channel === "position") vector[0] *= -1;
                if (channel === "rotation") vector[1] *= -1;
                const time = Number(keyframe.time);
                if (!Number.isFinite(time) || time < 0) throw new Error("非法关键帧时间");
                channels[channel] ??= {};
                if (Object.hasOwn(channels[channel], String(time))) {
                    throw new Error(`${animation.name} 同一通道存在重叠关键帧`);
                }
                channels[channel][String(time)] = vector;
            }
            boneTracks[bones.get(uuid)] = channels;
        }
        const key = `animation.bong.${name}.${animation.name.split(".").at(-1)}`;
        if (Object.hasOwn(animations, key)) throw new Error(`重复动画名 ${key}`);
        animations[key] = {
            loop: animation.loop === "loop" ? true : animation.loop === "hold" ? "hold_on_last_frame" : false,
            animation_length: animation.length,
            bones: boneTracks,
        };
    }
    return { format_version: "1.8.0", animations };
}

function convert(source, name) {
    const { model, bones } = normalizeModel(source);
    const geometry = JSON.parse(exportModel(model));
    geometry["minecraft:geometry"][0].description.identifier = `geometry.bong.${name}`;
    return { geometry, animation: exportAnimations(source, bones, name) };
}

function formatJson(document) {
    // 数值向量保留一行，骨树/时间轴仍缩进；避免每个三元组撑成五行生成物。
    return JSON.stringify(document, null, 2)
        .replace(/\[\s+(-?\d[\d.eE+, \n-]*)\s+\]/g, value => JSON.stringify(JSON.parse(value))) + "\n";
}

if (require.main === module) {
    const [input, directory, name] = process.argv.slice(2);
    if (!input || !directory || !/^[a-z0-9_]+$/.test(name || "")) {
        throw new Error("用法：node creature_codec.cjs <source.bbmodel> <out-dir> <name>");
    }
    const result = convert(JSON.parse(fs.readFileSync(input, "utf8")), name);
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, `${name}.geo.json`), formatJson(result.geometry));
    fs.writeFileSync(path.join(directory, `${name}.animation.json`), formatJson(result.animation));
}

module.exports = { convert };
