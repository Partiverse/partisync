title: GraphQL 入门
filename: graphql_introduction.md
tags: [graphql, api, rest, typescript, server]
updated_ns: 1704067200000000000

# GraphQL 入门

## 是什么

GraphQL = Facebook 2015 开源的 API 查询语言 + 运行时。

## vs REST

| 维度 | REST | GraphQL |
|---|---|---|
| 端点 | 多个 | 单个 |
| 数据 | 固定结构 | 客户端指定 |
| 文档 | OpenAPI | Schema 自描述 |
| 缓存 | HTTP 缓存友好 | 难（per-query） |

## Schema 定义

```graphql
type User {
  id: ID!
  name: String!
  posts: [Post!]!
}

type Post {
  id: ID!
  title: String!
  author: User!
}

type Query {
  user(id: ID!): User
  posts(limit: Int!): [Post!]!
}
```

## 查询

```graphql
query {
  user(id: "123") {
    name
    posts {
      title
      author {
        name
      }
    }
  }
}
```

## PartiSync MCP vs GraphQL

两者目标不同：
- MCP：工具调用（动作）
- GraphQL：数据查询（被动）

MCP 更适合 Agent（tool calling 模型原生）。
GraphQL 更适合前端（按需取数）。

## 服务端

- Apollo Server（Node）
- Hasura（自动 GraphQL）
- GraphQL Yoga

## 客户端

- Apollo Client（React）
- urql
- Relay（FB 出品）

## N+1 问题

```graphql
posts { author { name } }
```

→ 后端对每 post 查 author

解决：DataLoader 批量 / JOIN

## 持久查询（Persisted Queries）

- 客户端预注册 query hash
- 服务端只接受白名单
- 减少网络攻击面

## Federation

多服务合并成单一 GraphQL schema：
- Apollo Federation
- 各服务自管 schema
- Gateway 合并

## PartiSync 现状

- 用 MCP 而非 GraphQL
- 原因：MCP 专为 AI Agent 设计
- GraphQL 适合前端，PartiSync 客户端是 AI