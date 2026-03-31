# Actix 到 Axum 迁移实施计划

## 1. 项目概述

**项目名称**: Axon Rust Runtime Actix → Axum 迁移

**目标**: 将 Axon 的 Rust Web 运行时从 Actix-web 迁移到 Axum

**预期收益**:
- ✅ 更好的 Tower 生态兼容性
- ✅ 代码量减少 30-40%
- ✅ 编译时间缩短
- ✅ 更符合 Rust 标准模式

**预期风险**:
- ⚠️ 性能可能略有下降（< 15%）
- ⚠️ WebSocket 需要完全重写
- ⚠️ 需要 8-12 周全职工作

---

## 2. 实施时间表

### 甘特图概览

```
Week    1   2   3   4   5   6   7   8   9   10  11  12  13  14
        ├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┤
阶段 1  ████████                                                               (基础准备)
阶段 2          ████████████████████                                           (类型迁移)
阶段 3                          ████████████████                               (路由迁移)
阶段 4                                          ████████████████████           (WebSocket)
阶段 5                                                          ██████████████  (Server)
阶段 6                                                                          ██████████████ (测试)
```

---

## 3. 详细任务清单

### 阶段 1: 基础设施准备 (Week 1-2)

#### 1.1 Cargo.toml 更新

**任务负责人**: TBD
**预计时间**: 2 天
**依赖**: 无

**检查清单**:
- [ ] 添加 Axum 相关依赖
- [ ] 添加 Tower 相关依赖
- [ ] 添加 tokio-tungstenite 依赖
- [ ] 配置 feature flags
- [ ] 验证 `cargo check` 通过

**Cargo.toml 变更**:
```toml
[dependencies]
# 新增 Axum 依赖
axum = { version = "0.7", features = ["ws", "multipart"] }
axum-core = "0.4"
tower = { version = "0.4", features = ["full"] }
tower-http = { version = "0.5", features = ["fs", "trace", "limit", "cors"] }
tokio-tungstenite = "0.21"
hyper = { version = "1.0", features = ["full"] }
hyper-util = { version = "0.1", features = ["full"] }
http-body-util = "0.1"

# 保留 Actix 依赖 (可选，用于回滚)
actix-web = { version = "4.4.2", optional = true }
actix-web-actors = { version = "4.3.0", optional = true }
actix-http = { version = "3.3.1", optional = true }
actix-files = { version = "0.6.2", optional = true }
actix-multipart = { version = "0.6.1", optional = true }

[features]
default = ["actix-runtime"]
actix-runtime = ["dep:actix-web", "dep:actix-web-actors", "dep:actix-files", "dep:actix-multipart"]
axum-runtime = ["dep:axum", "dep:axum-core", "dep:tower", "dep:tower-http", "dep:tokio-tungstenite", "dep:hyper", "dep:hyper-util", "dep:http-body-util"]
```

#### 1.2 目录结构创建

**任务负责人**: TBD
**预计时间**: 1 天

**新目录结构**:
```
src/
├── axum_adapter/           # 新增
│   ├── mod.rs
│   ├── server.rs          # Axum 服务器
│   ├── websocket/         # Axum WebSocket 实现
│   │   ├── mod.rs
│   │   └── registry.rs
│   └── handlers/          # Axum handlers
│       ├── mod.rs
│       └── http.rs
├── types/
│   ├── request.rs         # 添加 from_axum_request
│   ├── response.rs        # 添加 IntoResponse 实现
│   └── headers.rs         # 添加 HTTP 头部转换
├── routers/               # 基本不变
├── executors/             # 类型适配
└── shared_socket.rs       # 完全复用
```

#### 1.3 测试框架搭建

**任务负责人**: TBD
**预计时间**: 3 天

**检查清单**:
- [ ] 设置 Criterion 性能基准测试
- [ ] 配置 Axum Test Client
- [ ] 创建并行测试脚本
- [ ] 编写 CI 配置更新

---

### 阶段 2: 核心类型迁移 (Week 3-5)

#### 2.1 Response 类型适配

**任务负责人**: TBD
**预计时间**: 3 天
**文件**: `src/types/response.rs`

**检查清单**:
- [ ] 实现 `IntoResponse` trait
- [ ] 保留原有 `Responder` 实现 (用于 Actix 构建)
- [ ] 使用 `#[cfg(feature = "axum-runtime")]` 条件编译
- [ ] 编写单元测试

**代码模板**:
```rust
#[cfg(feature = "axum-runtime")]
impl IntoResponse for Response {
    fn into_response(self) -> axum::response::Response {
        let mut builder = http::Response::builder()
            .status(self.status_code.to_u16());

        for (key, value) in self.headers.iter() {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let body = axum::body::Body::from(self.body.to_string());
        builder.body(body).unwrap().into_response()
    }
}
```

#### 2.2 Request 类型适配

**任务负责人**: TBD
**预计时间**: 4 天
**文件**: `src/types/request.rs`

**检查清单**:
- [ ] 实现 `from_axum_request()` 方法
- [ ] 实现 HTTP Method 转换
- [ ] 实现 Body 提取逻辑
- [ ] 保留原有 `from_actix_request()` 方法
- [ ] 编写单元测试

#### 2.3 Headers 类型适配

**任务负责人**: TBD
**预计时间**: 2 天
**文件**: `src/types/headers.rs`

**检查清单**:
- [ ] 实现 `from_http_headers()` 方法
- [ ] 实现 `Into<HeaderMap>` trait
- [ ] 编写单元测试

#### 2.4 Executors 适配

**任务负责人**: TBD
**预计时间**: 3 天
**文件**: `src/executors/mod.rs`

**检查清单**:
- [ ] 更新 `execute_http_function()` 类型签名
- [ ] 更新 `execute_middleware_function()` 类型签名
- [ ] 更新 `execute_startup_handler()` 类型签名
- [ ] 编写集成测试

---

### 阶段 3: 路由系统迁移 (Week 6-7)

#### 3.1 Router 模块类型适配

**任务负责人**: TBD
**预计时间**: 3 天
**文件**: `src/routers/*.rs`

**检查清单**:
- [ ] 更新 `HttpRouter` 类型签名
- [ ] 更新 `ConstRouter` 类型签名
- [ ] 更新 `MiddlewareRouter` 类型签名
- [ ] 更新 `WebSocketRouter` 类型签名
- [ ] 验证条件编译通过

#### 3.2 Axum Router 构建

**任务负责人**: TBD
**预计时间**: 4 天
**文件**: `src/axum_adapter/server.rs`

**检查清单**:
- [ ] 实现 `make_router()` 函数
- [ ] 集成静态文件服务 (tower-http)
- [ ] 集成路由 handlers
- [ ] 添加 Tower 中间件 (Trace, Limit, Timeout)
- [ ] 编写路由测试

**代码模板**:
```rust
fn make_router(state: RouterState) -> Router {
    let mut app = Router::new()
        .route("/*path", web::route().to(index_handler))
        .with_state(state);

    // 静态文件
    for dir in state.directories.iter() {
        app = app.nest_service(
            &dir.route,
            ServeDir::new(&dir.directory_path)
                .append_index_html_on_directories(true),
        );
    }

    // 中间件
    app = app
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(60)))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024));

    app
}
```

#### 3.3 静态文件服务迁移

**任务负责人**: TBD
**预计时间**: 2 天

**检查清单**:
- [ ] 迁移 `actix-files` → `tower-http::ServeDir`
- [ ] 测试目录浏览
- [ ] 测试 index 文件重定向
- [ ] 测试 mime 类型

---

### 阶段 4: WebSocket 迁移 (Week 8-10)

#### 4.1 ConnectionRegistry 设计

**任务负责人**: TBD
**预计时间**: 3 天
**文件**: `src/websockets/registry.rs`

**检查清单**:
- [ ] 实现 `ConnectionRegistry` struct
- [ ] 实现 `register()` 方法
- [ ] 实现 `unregister()` 方法
- [ ] 实现 `send_to()` 方法
- [ ] 实现 `broadcast()` 方法
- [ ] 使用 `Arc<Mutex<>>` 包装
- [ ] 编写单元测试

**代码模板**:
```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use axum::extract::ws::Message;

pub type SharedRegistry = Arc<Mutex<ConnectionRegistry>>;

pub struct ConnectionRegistry {
    connections: HashMap<Uuid, mpsc::Sender<Message>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self { connections: HashMap::new() }
    }

    pub async fn register(&mut self, id: Uuid, tx: mpsc::Sender<Message>) {
        self.connections.insert(id, tx);
    }

    pub async fn unregister(&mut self, id: Uuid) {
        self.connections.remove(&id);
    }

    pub async fn send_to(&self, id: Uuid, msg: Message) -> Result<(), SendError> {
        if let Some(tx) = self.connections.get(&id) {
            tx.send(msg).await.map_err(|_| SendError)?;
        }
        Ok(())
    }

    pub async fn broadcast(&self, msg: Message) {
        let futures: Vec<_> = self.connections.values()
            .map(|tx| tx.send(msg.clone()))
            .collect();
        futures::future::join_all(futures).await;
    }
}
```

#### 4.2 WebSocket Handler 实现

**任务负责人**: TBD
**预计时间**: 5 天
**文件**: `src/websockets/mod.rs`

**检查清单**:
- [ ] 实现 `websocket_handler()` 函数
- [ ] 实现 `handle_socket()` 函数
- [ ] 实现消息发送任务
- [ ] 实现消息接收任务
- [ ] 实现连接生命周期管理
- [ ] 集成 Python WebSocket handlers
- [ ] 编写集成测试

**代码模板**:
```rust
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    params: QueryParams,
    State(registry): State<SharedRegistry>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_socket(socket, params, registry).await;
    })
}

async fn handle_socket(
    socket: WebSocket,
    params: QueryParams,
    registry: SharedRegistry,
) {
    let id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel(32);

    // 注册
    registry.lock().await.register(id, tx).await;

    let (mut sender, mut receiver) = socket.split();

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // 接收任务
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // 执行 Python handler
                    execute_ws_function(&text, &params).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // 清理
    registry.lock().await.unregister(id).await;
}
```

#### 4.3 WebSocket Router 集成

**任务负责人**: TBD
**预计时间**: 2 天
**文件**: `src/routers/web_socket_router.rs`

**检查清单**:
- [ ] 更新 WebSocket 路由注册逻辑
- [ ] 集成到 Axum Router
- [ ] 测试 WebSocket 连接
- [ ] 测试消息收发

---

### 阶段 5: Server 整合 (Week 11-12)

#### 5.1 Server 模块重写

**任务负责人**: TBD
**预计时间**: 4 天
**文件**: `src/server.rs`

**检查清单**:
- [ ] 实现 `start()` 方法的 Axum 版本
- [ ] 实现 Tokio runtime 初始化
- [ ] 集成 `make_router()`
- [ ] 集成静态文件服务
- [ ] 集成 WebSocket handlers
- [ ] 共享 Socket 适配
- [ ] 编写端到端测试

**代码模板**:
```rust
#[cfg(feature = "axum-runtime")]
pub fn start(&mut self, py: Python, socket: &PyCell<SocketHeld>, workers: usize) -> PyResult<()> {
    pyo3_log::init();

    if STARTED.compare_exchange(false, true, SeqCst, Relaxed).is_err() {
        debug!("Axon is already running...");
        return Ok(());
    }

    let raw_socket = socket.try_borrow_mut()?.get_socket();

    // 创建 Tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    // 构建状态
    let state = RouterState {
        router: self.router.clone(),
        const_router: self.const_router.clone(),
        middleware_router: self.middleware_router.clone(),
        websocket_router: self.websocket_router.clone(),
        registry: Arc::new(Mutex::new(ConnectionRegistry::new())),
    };

    // 构建 Router
    let app = make_router(state);

    // 创建监听器
    let listener = tokio::net::TcpListener::from_std(raw_socket)
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    // 启动服务
    runtime.block_on(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(())
}
```

#### 5.2 条件编译整合

**任务负责人**: TBD
**预计时间**: 2 天

**检查清单**:
- [ ] 所有 `use actix_web` 添加条件编译
- [ ] 所有 `use axum` 添加条件编译
- [ ] 验证 `cargo build --features actix-runtime` 通过
- [ ] 验证 `cargo build --features axum-runtime --no-default-features` 通过

#### 5.3 Python 层验证

**任务负责人**: TBD
**预计时间**: 2 天

**检查清单**:
- [ ] 验证 `maturin develop` 构建
- [ ] 验证 Python import
- [ ] 验证基础 HTTP 请求
- [ ] 验证中间件
- [ ] 验证 WebSocket

---

### 阶段 6: 测试验证 (Week 13-14)

#### 6.1 单元测试

**任务负责人**: TBD
**预计时间**: 3 天

**检查清单**:
- [ ] 运行 `cargo test --features axum-runtime`
- [ ] 确保单元测试覆盖率 > 80%
- [ ] 修复所有测试失败
- [ ] 生成覆盖率报告

#### 6.2 集成测试

**任务负责人**: TBD
**预计时间**: 4 天

**检查清单**:
- [ ] 运行所有 Python 集成测试
- [ ] 验证 HTTP 所有方法
- [ ] 验证中间件链
- [ ] 验证 WebSocket 功能
- [ ] 验证静态文件服务
- [ ] 验证 Multipart 表单

#### 6.3 性能基准

**任务负责人**: TBD
**预计时间**: 3 天

**检查清单**:
- [ ] 运行 Criterion 基准测试
- [ ] 对比 Actix vs Axum 性能
- [ ] 生成性能报告
- [ ] 分析内存使用
- [ ] 分析 CPU 使用

**基准测试脚本**:
```bash
# Actix 基准
cargo bench --features actix-runtime

# Axum 基准
cargo bench --features axum-runtime --no-default-features

# 对比报告
cargo bench --features axum-runtime -- --save-baseline axum
cargo bench --features actix-runtime -- --baseline axum
```

#### 6.4 文档更新

**任务负责人**: TBD
**预计时间**: 2 天

**检查清单**:
- [ ] 更新 README.md
- [ ] 更新架构文档
- [ ] 编写迁移指南
- [ ] 更新 CHANGELOG.md
- [ ] 编写发布说明

---

## 4. 交付物清单

### 代码交付物

| 文件 | 状态 | 说明 |
|------|------|------|
| `Cargo.toml` | 修改 | 添加 Axum 依赖和 feature flags |
| `src/axum_adapter/mod.rs` | 新增 | Axum 适配器入口 |
| `src/axum_adapter/server.rs` | 新增 | Axum 服务器实现 |
| `src/axum_adapter/handlers/mod.rs` | 新增 | HTTP handlers |
| `src/axum_adapter/handlers/http.rs` | 新增 | HTTP handler 实现 |
| `src/websockets/mod.rs` | 重写 | WebSocket Stream 实现 |
| `src/websockets/registry.rs` | 重写 | ConnectionRegistry 实现 |
| `src/types/request.rs` | 修改 | 添加 `from_axum_request()` |
| `src/types/response.rs` | 修改 | 添加 `IntoResponse` 实现 |
| `src/types/headers.rs` | 修改 | 添加 HTTP 头部转换 |
| `src/executors/mod.rs` | 修改 | 类型签名适配 |
| `src/server.rs` | 修改 | 添加 Axum `start()` 方法 |

### 文档交付物

| 文件 | 状态 | 说明 |
|------|------|------|
| `spec/actix-to-axum-migration.md` | 已完成 | 迁移设计文档 |
| `spec/IMPLEMENTATION_PLAN.md` | 进行中 | 实施计划（本文档） |
| `docs/en/axum-runtime.md` | 待创建 | Axum 运行时文档 |
| `docs/zh/axum-runtime.md` | 待创建 | Axum 运行时中文文档 |
| `MIGRATION_GUIDE.md` | 待创建 | 迁移指南 |

### 测试交付物

| 文件 | 状态 | 说明 |
|------|------|------|
| `.github/workflows/axum-tests.yml` | 待创建 | Axum CI 配置 |
| `scripts/benchmark-axum.sh` | 待创建 | 性能基准脚本 |
| `integration_tests/test_axum_*.py` | 待创建 | Axum 集成测试 |

---

## 5. 风险管理

### 5.1 技术风险

| 风险 | 概率 | 影响 | 缓解措施 | 负责人 |
|------|------|------|----------|--------|
| WebSocket 功能不完整 | 中 | 高 | 优先实现核心功能，渐进式完善 | TBD |
| 性能回归超过 20% | 低 | 高 | 持续基准测试，保留 Actix fallback | TBD |
| Python 层不兼容 | 低 | 高 | 保持 PyO3 FFI 边界不变 | TBD |
| 多进程支持问题 | 中 | 中 | 早期测试共享 Socket | TBD |

### 5.2 进度风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 迁移时间超期 | 中 | 中 | 分阶段交付，优先 P0 任务 |
| 测试覆盖不足 | 中 | 中 | 优先保证核心路径覆盖 |
| 文档更新滞后 | 高 | 低 | 代码完成后立即更新 |

### 5.3 回滚计划

如果迁移遇到问题，按以下步骤回滚：

1. **切换 feature flag**:
   ```bash
   cargo build --features actix-runtime --no-default-features
   ```

2. **恢复代码**:
   ```bash
   git checkout <previous-commit>
   ```

3. **发布说明**:
   - 说明回滚原因
   - 计划重新迁移时间

---

## 6. 沟通计划

### 6.1 状态更新

| 频率 | 内容 | 渠道 |
|------|------|------|
| 每周 | 进度报告 | GitHub Issue |
| 每阶段 | 阶段总结 | GitHub Discussion |
| 完成 | 发布公告 | README, CHANGELOG |

### 6.2 问题跟踪

- 使用 GitHub Issues 跟踪任务
- 标签：`axum-migration`, `in-progress`, `blocked`

### 6.3 决策记录

- 使用 `docs/adr/` 记录架构决策
- 格式：ADR-XXXX-标题.md

---

## 7. 资源需求

### 7.1 人力资源

| 角色 | 人数 | 时间 |
|------|------|------|
| Rust 开发者 | 1-2 | 8-12 周全职 |
| Python 开发者 | 1 | 2-3 周兼职（测试） |
| 文档撰写 | 1 | 1 周兼职 |

### 7.2 基础设施

| 资源 | 用途 |
|------|------|
| CI Runner | 并行测试 |
| 性能测试服务器 | 基准对比 |
| 文档托管 | GitHub Pages |

---

## 8. 成功标准

### 8.1 功能完整

- [ ] 所有 HTTP 方法支持
- [ ] 中间件系统正常
- [ ] WebSocket 完全支持
- [ ] 静态文件服务正常
- [ ] Multipart 支持
- [ ] 多进程支持

### 8.2 性能达标

- [ ] 吞吐量 >= Actix 的 90%
- [ ] P99 延迟增加 <= 20%
- [ ] 内存使用增加 <= 10%
- [ ] 编译时间减少 >= 20%

### 8.3 质量保证

- [ ] 单元测试覆盖率 > 80%
- [ ] 所有集成测试通过
- [ ] 无内存泄漏
- [ ] 文档完整

---

## 9. 附录

### 9.1 构建命令速查

```bash
# Actix 构建 (默认)
cargo build

# Axum 构建
cargo build --features axum-runtime --no-default-features

# Actix 测试
cargo test --features actix-runtime

# Axum 测试
cargo test --features axum-runtime --no-default-features

# Actix 基准
cargo bench --features actix-runtime

# Axum 基准
cargo bench --features axum-runtime --no-default-features

# 格式化
cargo fmt

# Clippy 检查
cargo clippy --features axum-runtime --no-default-features
```

### 9.2 Python 构建命令

```bash
# Actix 开发构建
maturin develop

# Axum 开发构建
maturin develop --features axum-runtime --no-default-features

# Actix 发布构建
maturin build --release

# Axum 发布构建
maturin build --release --features axum-runtime --no-default-features
```

### 9.3 关键代码位置索引

| 功能 | Actix 实现 | Axum 实现 |
|------|-----------|-----------|
| HTTP Server | `src/server.rs:128-228` | `src/axum_adapter/server.rs` |
| WebSocket | `src/websockets/mod.rs` | `src/websockets/mod.rs` (重写) |
| Registry | `src/websockets/registry.rs` | `src/websockets/registry.rs` (重写) |
| Request | `src/types/request.rs` | `src/types/request.rs` (添加) |
| Response | `src/types/response.rs` | `src/types/response.rs` (添加) |
| Router | `src/routers/` | `src/routers/` (适配) |

---

**文档版本**: 1.0
**创建日期**: 2026-03-29
**最后更新**: 2026-03-29
**维护者**: Axon Core Team
