# 文档索引

这里整理 `sublinkx-rs` 的部署、规划、兼容性和模板相关文档。

## 部署

- [Docker 部署](docker.md)：Docker Compose、数据映射、SQLite/MySQL、固定网段和反向代理说明。
- [Docker 英文部署说明](docker.en.md)：英文版 Docker 部署说明。

## 设计与规划

- [重构蓝图](plan.md)：Rust + Vue 3 重构目标、模块划分和系统设计。
- [客户端目标注册表](client-target-registry.md)：客户端 target、renderer family 和模板预设规划。

## 兼容性

- [客户端兼容矩阵](client-compatibility.md)：主流客户端家族的支持优先级。
- [协议 x 客户端矩阵](protocol-client-matrix.md)：协议能力和客户端导出目标的对应关系。

## 模板

- [Clash 分流模板说明](clash-routing-template.md)：Clash/Mihomo 分流模板结构和示例。
