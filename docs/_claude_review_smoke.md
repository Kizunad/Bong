# claude-review smoke test

一次性测试 PR，用于验证 `@claude` 触发 + bypassPermissions 下对抗式子代理
(review-finder / review-skeptic) 能否真派生。验证完即关闭并删分支。

下面放一小段"可审"的内容给 finder 找茬：

```ts
// 故意留一个可疑点：除零未防护
function avg(xs: number[]): number {
  return xs.reduce((a, b) => a + b, 0) / xs.length; // xs 为空 -> NaN
}
```
