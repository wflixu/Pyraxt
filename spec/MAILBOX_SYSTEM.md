# Mailbox 系统详解

## 1. 什么是 Mailbox？

**Mailbox（邮箱）** 是 Actor 模型中的核心组件，用于实现 Actor 之间的**异步消息传递**。

### 核心概念

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│   Actor A   │ ────▶│   Mailbox   │─────▶│   Actor B   │
│  (发送者)   │ send │  (消息队列) │ recv │  (接收者)   │
└─────────────┘      └─────────────┘      └─────────────┘
```

**Mailbox 本质**: 一个**消息队列**，位于 Actor 内部，用于接收和缓存发送给该 Actor 的消息。

---

## 2. Mailbox 的原理

### 2.1 基本架构

```
┌─────────────────────────────────────────────────────────┐
│                      Actor B                             │
│  ┌───────────────────────────────────────────────────┐  │
│  │                  Mailbox                          │  │
│  │  ┌───────┐  ┌───────┐  ┌───────┐  ┌───────┐     │  │
│  │  │Msg 1  │→ │Msg 2  │→ │Msg 3  │→ │Msg 4  │ → ...│  │
│  │  └───────┘  └───────┘  └───────┘  └───────┘     │  │
│  │     ▲                                        │    │  │
│  │     │                                        │    │  │
│  │     └─────────────── 队列 ───────────────────┘    │  │
│  └───────────────────────────────────────────────────┘  │
│                         │                                │
│                         ▼                                │
│              ┌─────────────────────┐                    │
│              │  Actor Context/Loop │                    │
│              │  (消息处理器)        │                    │
│              └─────────────────────┘                    │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │ send()
                         │
                ┌─────────────────┐
                │    Actor A      │
                └─────────────────┘
```

### 2.2 消息流转过程

```rust
// 1. Actor A 发送消息
addr_b.send(Message { data: "Hello" });

// 2. 消息进入 Actor B 的 Mailbox (入队)
//    Mailbox: [Message { data: "Hello" }]

// 3. Actor B 的消息循环处理
//    loop {
//        let msg = mailbox.recv().await;  // 出队
//        self.handle(msg);                // 处理
//    }
```

### 2.3 内部实现 (简化版)

```rust
use tokio::sync::mpsc;

/// Actor 地址 (用于发送消息)
pub struct Addr<A: Actor> {
    sender: mpsc::Sender<A::Message>,
}

impl<A: Actor> Addr<A> {
    /// 发送消息 (异步)
    pub async fn send(&self, msg: A::Message) -> Result<(), SendError> {
        self.sender.send(msg).await
    }

    /// 发送消息 (忽略结果)
    pub fn do_send(&self, msg: A::Message) {
        self.sender.try_send(msg).ok();
    }
}

/// Actor trait
pub trait Actor {
    type Message;  // 消息类型
    type Context;  // 上下文类型

    fn handle(&mut self, msg: Self::Message, ctx: &mut Self::Context);
}

/// Mailbox 内部结构
pub struct Mailbox<A: Actor> {
    receiver: mpsc::Receiver<A::Message>,
}

impl<A: Actor> Mailbox<A> {
    pub async fn recv(&mut self) -> Option<A::Message> {
        self.receiver.recv().await
    }
}
```

---

## 3. Mailbox 的核心功能

### 3.1 消息缓冲

**问题**: 如果 Actor 正在处理消息，新消息怎么办？

**答案**: Mailbox 作为缓冲区，临时存储消息。

```
Actor B 正在处理 Msg 1

Mailbox: [Msg 2, Msg 3, Msg 4, ...]
              ▲
              │
         新消息入队
```

**容量管理**:
```rust
// Actix 默认 Mailbox 容量
pub struct Context<A: Actor> {
    mailbox: Mailbox<A>,
    capacity: usize,  // 默认 16
}

// 容量满时行为
// - send(): 等待 (异步阻塞)
// - do_send(): 丢弃消息
```

### 3.2 异步消息传递

```rust
// 发送方 - 不等待 Actor 处理完成
addr.send(Msg).await;  // 只等待消息入队

// 接收方 - Actor 按顺序处理
async fn run(&mut self) {
    while let Some(msg) = self.mailbox.recv().await {
        self.handle(msg);  // 顺序处理
    }
}
```

### 3.3 消息排序

**FIFO (先进先出)** 保证:

```
发送顺序：Msg A → Msg B → Msg C
Mailbox:  [A, B, C]
处理顺序：A → B → C (严格顺序)
```

### 3.4 背压 (Backpressure)

当 Mailbox 满时:

```rust
// 场景：Mailbox 容量 = 10, 已有 10 条消息

// send() - 等待
addr.send(msg).await;  // 阻塞直到有空位

// do_send() - 丢弃
addr.do_send(msg);  // 返回 Err(Timeout), 消息丢失
```

---

## 4. Actix 中的 Mailbox 实现

### 4.1 Actix Actor 结构

```rust
use actix::{Actor, ActorContext, Handler, Message};

// 定义消息
#[derive(Message)]
#[rtype(result = "()")]
struct Ping {
    id: u32,
}

// 定义 Actor
struct MyActor {
    name: String,
}

// 实现 Actor trait
impl Actor for MyActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Actor started");
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        println!("Actor stopped");
    }
}

// 实现消息处理器
impl Handler<Ping> for MyActor {
    type Result = ();

    fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) {
        println!("Received ping: {}", msg.id);
    }
}

// 使用
let addr = MyActor { name: "test".to_string() }.start();
addr.send(Ping { id: 1 }).await;
```

### 4.2 Actix Mailbox 类型

Actix 提供两种 Mailbox:

| 类型 | 容量 | 行为 | 使用场景 |
|------|------|------|----------|
| **Unbounded** | 无限 | 永不阻塞 | 默认 |
| **Bounded** | 固定 | 满时阻塞 | 需要背压控制 |

```rust
// Unbounded (默认)
let addr = MyActor.start();

// Bounded
let addr = MyActor.start_with_context(|ctx| {
    ctx.set_mailbox_capacity(10);
});
```

### 4.3 消息类型

Actix 支持三种消息:

#### 4.3.1 普通消息 (Message)

```rust
#[derive(Message)]
#[rtype(result = "()")]
struct Ping;

impl Handler<Ping> for MyActor {
    type Result = ();
    fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) {
        // 同步处理
    }
}
```

#### 4.3.2 响应消息 (Message with Response)

```rust
#[derive(Message)]
#[rtype(result = "String")]
struct GetName;

impl Handler<GetName> for MyActor {
    type Result = String;
    fn handle(&mut self, msg: GetName, ctx: &mut Self::Context) -> Self::Result {
        self.name.clone()
    }
}

// 使用
let name = addr.send(GetName).await?;
```

#### 4.3.3 流式消息 (StreamHandler)

```rust
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyActor {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // 处理流式消息
    }
}
```

---

## 5. WebSocket 中的 Mailbox 应用

### 5.1 Actix WebSocket 架构

```
┌─────────────────────────────────────────────────────────┐
│              WebSocketConnector (Actor)                  │
│  ┌───────────────────────────────────────────────────┐  │
│  │                  Mailbox                          │  │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐    │  │
│  │  │ConnectMsg │→ │TextMsg    │→ │CloseMsg   │ → ...│  │
│  │  └───────────┘  └───────────┘  └───────────┘    │  │
│  └───────────────────────────────────────────────────┘  │
│                         │                                │
│                         ▼                                │
│    StreamHandler<Result<ws::Message, ProtocolError>>    │
└─────────────────────────────────────────────────────────┘
```

### 5.2 代码示例

```rust
use actix::{Actor, StreamHandler};
use actix_web_actors::ws;

pub struct WebSocketConnector {
    pub id: Uuid,
    pub registry: Addr<WebSocketRegistry>,
}

// Actor 实现
impl Actor for WebSocketConnector {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // 注册到 Registry
        self.registry.do_send(Connect { id: self.id });
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        // 从 Registry 注销
        self.registry.do_send(Disconnect { id: self.id });
    }
}

// WebSocket 消息处理
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketConnector {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => {
                // 自动响应 Pong
                ctx.pong(&msg);
            }
            ws::Message::Text(text) => {
                // 文本消息 → 进入 Mailbox → 按顺序处理
                println!("Received: {}", text);
            }
            ws::Message::Close(reason) => {
                // 关闭连接
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}
```

### 5.3 Mailbox 在 WebSocket 中的作用

```
WebSocket 连接
      │
      ▼
┌─────────────────┐
│  TCP 数据到达   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  解析为 ws::Message  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Mailbox 入队    │ ← 如果 Actor 忙，消息在这里排队
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Actor 按顺序处理  │
└─────────────────┘
```

---

## 6. Mailbox vs Channel

| 特性 | Mailbox (Actor) | Channel (Stream) |
|------|-----------------|------------------|
| **抽象层次** | 高层 (Actor 内置) | 底层 (需要手动管理) |
| **消息队列** | 自动管理 | 需要显式创建 |
| **生命周期** | Actor 管理 | 手动管理 |
| **背压** | 自动 (容量限制) | 需要手动实现 |
| **消息排序** | 保证 FIFO | 取决于使用方式 |
| **错误处理** | Actor 监督 | 需要手动传播 |
| **使用场景** | Actor 模型 | Stream/async 模式 |

### 代码对比

**Mailbox (Actix)**:
```rust
// 自动创建 Mailbox
let addr = MyActor.start();

// 直接发送
addr.send(Msg).await;

// Actor 内部自动处理
impl StreamHandler<Msg> for MyActor {
    fn handle(&mut self, msg: Msg, ctx: &mut Self::Context) {
        // 直接处理
    }
}
```

**Channel (Axum/Tokio)**:
```rust
// 手动创建 channel
let (tx, mut rx) = mpsc::channel(32);

// 发送
tx.send(msg).await?;

// 接收 (需要手动 spawn task)
tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
        // 手动处理
    }
});
```

---

## 7. Mailbox 的优缺点

### ✅ 优点

| 优点 | 说明 |
|------|------|
| **封装性好** | Mailbox 是 Actor 的一部分，开发者无需关心内部实现 |
| **自动背压** | 容量满时自动阻塞发送方 |
| **消息排序** | 严格保证 FIFO 顺序 |
| **生命周期管理** | Actor 停止时自动清理 Mailbox |
| **简化代码** | 无需手动管理 channel |

### ❌ 缺点

| 缺点 | 说明 |
|------|------|
| **灵活性差** | 只能使用 Actor 模式，难以与其他模式组合 |
| **性能开销** | 额外的消息复制和队列管理 |
| **调试困难** | 消息异步传递，问题难以追踪 |
| **学习曲线** | 需要理解 Actor、Context、Addr 等概念 |
| **生态封闭** | 难以与标准 Stream/Channel 互操作 |

---

## 8. 实际案例：Axon WebSocket

### 8.1 当前实现 (Actix Mailbox)

```rust
// src/websockets/mod.rs

pub struct WebSocketConnector {
    pub id: Uuid,
    pub registry_addr: Addr<WebSocketRegistry>,  // ← Actor 地址
}

impl Actor for WebSocketConnector {
    type Context = ws::WebsocketContext<Self>;
    // Mailbox 自动创建
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketConnector {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // 消息通过 Mailbox 按顺序到达
        match msg {
            ws::Message::Text(text) => {
                // 执行 Python handler
                execute_ws_function(&text, self.params);
            }
            ws::Message::Close(_) => {
                // 通知 Registry
                self.registry_addr.do_send(Disconnect { id: self.id });
                ctx.stop();
            }
        }
    }
}
```

### 8.2 迁移后实现 (Axum Channel)

```rust
// src/websockets/mod.rs (Axum)

pub async fn handle_socket(socket: WebSocket, params: QueryParams) {
    // 手动创建 channel (替代 Mailbox)
    let (tx, mut rx) = mpsc::channel(32);

    let (mut sender, mut receiver) = socket.split();

    // 发送任务 (替代 Mailbox 出队)
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            sender.send(msg).await.ok();
        }
    });

    // 接收任务 (替代 StreamHandler)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    execute_ws_function(&text, &params).await;
                }
                Message::Close(_) => break,
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

---

## 9. 总结

### Mailbox 核心要点

| 问题 | 答案 |
|------|------|
| **是什么** | Actor 内部的消息队列 |
| **为什么** | 实现异步、有序的消息传递 |
| **怎么用** | 通过 `Addr::send()` 发送，Actor 自动处理 |
| **优点** | 封装好、自动背压、FIFO 排序 |
| **缺点** | 灵活性差、性能开销、调试困难 |
| **替代方案** | Channel (tokio::sync::mpsc) |

### 在 Axon 迁移中的意义

- **Actix → Axum 迁移** 本质上是 **Mailbox → Channel** 的迁移
- 需要手动管理原本 Mailbox 自动处理的事情
- 但获得更好的灵活性和生态兼容性

---

**参考资源**:
- [Actix Actor Documentation](https://actix.rs/docs/actor/)
- [Mailbox Pattern](https://www.enterpriseintegrationpatterns.com/patterns/messaging/Mailbox.html)
- [tokio::sync::mpsc](https://docs.rs/tokio/latest/tokio/sync/mpsc/)
