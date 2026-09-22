title: SwiftUI iOS 开发入门
filename: swift_ios_intro.md
tags: [ios, swift, swiftui, mobile, apple]
updated_ns: 1704067200000000000

# SwiftUI iOS 开发入门

## 工具

- Xcode 16+（Swift 6）
- iOS Simulator
- Instruments（性能）

## 基础

### View

```swift
struct ContentView: View {
    var body: some View {
        VStack {
            Text("Hello, World!")
                .font(.title)
            Button("Tap me") {
                print("Tapped")
            }
        }
        .padding()
    }
}
```

### State

```swift
@State private var count = 0
// 单 view 内状态

@StateObject private var model = ViewModel()
// view 生命周期内共享
```

## 导航

```swift
NavigationStack {
    List(items) { item in
        NavigationLink(value: item) {
            RowView(item: item)
        }
    }
    .navigationDestination(for: Item.self) { item in
        DetailView(item: item)
    }
}
```

## 数据

- SwiftData（持久化，新）
- Core Data（成熟）
- CloudKit（云同步）

## 异步

```swift
.task {
    let data = try await fetchData()
    self.items = data
}
```

## 性能

- 避免 view 频繁 rebuild
- LazyVStack/LazyHStack 大列表
- Reduce 重渲染（`Equatable` view）

## 发布

- TestFlight（beta）
- App Store Connect
- App Review Guidelines

## 资源

- Hacking with Swift（Paul Hudson）
- Swift by Sundell
- objc.io 高级

## 与 PartiSync

- M6 Tauri 桌面 + iOS 移动（远期）
- MCP server 在 iOS 上的客户端形态（远期）