# Axum Template 架构分析

## 架构层次

### 应用层 (app/)

- **dto/**: API数据传输对象 (请求/响应/提取器)
- **middleware/**: HTTP中间件 (认证/CORS/日志)
- **route/**: 路由处理器和业务逻辑
- **router.rs**: 路由配置和OpenAPI集成
- **serve.rs**: 服务器启动配置
- **state.rs**: 应用状态管理
- **init.rs**: 应用初始化引导

### 领域层 (domain/)

- **db/**: 数据库连接管理
- **model/**: 领域模型定义
- **repository/**: 数据访问层 (CRUD操作)
- **service/**: 业务逻辑服务
- **facade.rs**: 领域层统一接口

### 基础设施层

- **config/**: 配置管理
- **error/**: 错误处理
- **logging/**: 日志系统
- **util/**: 工具函数
- **cli/**: 命令行工具

## 核心组件职责

### AppBootstrap

- 加载配置和环境变量
- 初始化日志系统
- 建立数据库连接
- 构建领域层依赖

### Domain

- 封装所有领域逻辑
- 管理数据库连接池
- 提供仓库和服务实例

### AppState

- 持有应用全局状态
- 通过Deref提供领域层访问

## 数据流

```txt
HTTP请求 → 中间件 → 路由 → DTO验证 → 领域服务 → 仓库 → 数据库 → 响应
```

## 技术栈

- **Web框架**: Axum + Tokio
- **ORM**: SeaORM (SQLite/PostgreSQL)
- **认证**: axum-login + tower-sessions
- **文档**: utoipa + Swagger/Scalar
- **验证**: validator
- **序列化**: serde
