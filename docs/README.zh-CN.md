<div align="center">

# Culebra

**/koo-LEH-brah/**

**面向 LLVM 自托管语言的编译器诊断工具。**

*ABI。IR。二进制。Bootstrap。一个二进制文件捕获调试器无法发现的问题。*

诞生于 [Mapanare](https://github.com/Mapanare-Research/Mapanare) 的 bootstrap 过程，在那里每个 bug 都是一个没有安全网的谜题。Culebra 内置 Nuclei 风格的模板引擎，让你经历过的每个编译器 bug 都成为别人不必再调试的模式。

[English](../README.md) | [Espanol](README.es.md) | 中文版 | [Portugues](README.pt.md)

<br>

![Rust](https://img.shields.io/badge/Rust-2021_版本-dea584?style=for-the-badge&logo=rust&logoColor=white)
![LLVM](https://img.shields.io/badge/LLVM-IR_分析-262D3A?style=for-the-badge&logo=llvm&logoColor=white)
![平台](https://img.shields.io/badge/Linux%20%7C%20macOS%20%7C%20Windows-grey?style=for-the-badge)

[![许可证](https://img.shields.io/badge/许可证-MIT-green.svg?style=flat-square)](../LICENSE)
[![版本](https://img.shields.io/badge/版本-0.1.0-blue.svg?style=flat-square)](../Cargo.toml)
[![模板](https://img.shields.io/badge/模板-17_内置-orange.svg?style=flat-square)]()
[![GitHub Stars](https://img.shields.io/github/stars/Mapanare-Research/Culebra?style=flat-square&color=f5c542)](https://github.com/Mapanare-Research/Culebra/stargazers)

<br>

[为什么选择 Culebra？](#为什么选择-culebra) · [安装](#安装) · [快速开始](#快速开始) · [模板引擎](#模板引擎) · [所有命令](#所有命令) · [内置模板](#内置模板) · [配置](#配置-culebratoml) · [架构](#架构) · [完整文档](../docs.md) · [贡献](#贡献)

</div>

---

## 为什么选择 Culebra？

大多数语言都是在成熟编译器之上实现自托管的：

- **Rust** 最初用 OCaml 编写，大约一年后实现自托管。
- **Go** 在 v1.5 之前用 C 编写，然后使用自动 C 到 Go 翻译器。
- **C++** 通过 Cfront 实现自托管，将 C++ 翻译为 C。

[Mapanare](https://github.com/Mapanare-Research/Mapanare) 没有这个条件。它是一个面向 AI 的编译语言，生成 LLVM IR，从零构建自己的后端：词法分析器、AST、类型推断、LLVM IR 发射。Bootstrap 编译器（Stage 0）用 Python 编写，但底层没有成熟的编译器作为后盾。

这意味着每一个 ABI 不匹配、每一个字符串字节计数错误、每一个 IR 和 C 之间的结构体布局差异、每一个 bootstrap 阶段回归都会直接命中，没有安全网。

**Culebra 就是安全网。**

它的存在是因为 Mapanare 需要它来度过自己的 bootstrap。事实证明，每个生成 LLVM 的编译器项目都需要同样的东西，但之前没有人将它打包。

我们不仅构建了一个 linter。我们构建了一个模式引擎。我们经历的每个编译器 bug 都变成了一个模板，这样其他人就不必再经历了。

> 名字的由来：*Mapanare* 是委内瑞拉的一种蝮蛇。*Culebra* 是普通的蛇。同一家族，不同的角色。Mapanare 是语言，Culebra 是任何编译器开发者都可以使用的实用工具。

---

## 安装

### Linux / macOS

```bash
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### Windows

```powershell
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### 从源码构建

```bash
git clone https://github.com/Mapanare-Research/Culebra.git
cd Culebra
cargo build --release
# 二进制文件在 target/release/culebra
```

验证：

```bash
culebra --version
```

---

## 快速开始

你刚从编译器输出了 `stage2.ll`，运行时出了问题：

```bash
# 1. 扫描所有已知 bug 模式
culebra scan stage2.ll

# 2. 只关注关键 ABI bug
culebra scan stage2.ll --tags abi --severity critical

# 3. 自动修复可修复的问题
culebra scan stage2.ll --autofix --dry-run   # 预览
culebra scan stage2.ll --autofix             # 应用

# 4. IR 是否有效？
culebra check stage2.ll

# 5. 字符串常量是否正确？
culebra strings stage2.ll

# 6. 有已知病理吗？
culebra audit stage2.ll

# 7. stage1 和 stage2 之间有什么变化？
culebra diff stage1.ll stage2.ll

# 8. 深入检查某个函数
culebra extract stage2.ll my_broken_function

# 9. 对照 C 运行时验证结构体布局
culebra abi stage2.ll --header runtime/mapanare_core.c

# 10. 检查编译后二进制的 .rodata
culebra binary ./my_compiler --ir stage2.ll --find "hello world"

# 11. 运行完整管道
culebra pipeline
```

---

## Culebra 捕获的真实 bug

来自 Mapanare bootstrap 的真实 bug。每一个都浪费了数小时的调试时间。

### 未对齐的字符串常量（bootstrap 杀手）

没有 `align 2` 的字符串常量落在奇数地址。指针标记将指针偏移 -1 字节。所有字符串比较静默失败。词法分析器产生 0 个 token。编译器输出空 IR。没有崩溃，没有错误。

```bash
$ culebra scan stage2.ll --id unaligned-string-constant
CRITICAL [unaligned-string-constant] 字符串常量缺少对齐 -- stage2.ll:47
  @.str.0 是一个 6 字节的字符串常量，没有对齐。
  fix: 在所有 [N x i8] 常量声明中添加 ', align 2'
```

### 列表推送无回写（别名分析陷阱）

通过 GEP 直接将数据推送到结构体字段中的列表。LLVM 缓存了推送前的结构体状态。变更丢失。Stage 1 正常，Stage 2 累积 0 行。

```bash
$ culebra scan stage2.ll --id direct-push-no-writeback
HIGH [direct-push-no-writeback] 列表推送无临时回写 -- stage2.ll:142 (在 emit_line 中)
  结构体字段 2 的列表推送直接通过 GEP，没有临时 alloca + 回写。
  LLVM 可能在 -O1+ 优化掉该变更。
```

---

## 模板引擎

Culebra 内置 Nuclei 风格的模式引擎。Bug 模式是 YAML 模板。Rust 二进制是引擎。模板是知识库。

### 扫描

```bash
# 运行所有模板
culebra scan file.ll

# 按标签、严重性或特定模板过滤
culebra scan file.ll --tags abi,string
culebra scan file.ll --severity critical,high
culebra scan file.ll --id unaligned-string-constant

# 跨文件 ABI 检查
culebra scan file.ll --header runtime.c

# 自动修复
culebra scan file.ll --autofix --dry-run
culebra scan file.ll --autofix

# 输出格式
culebra scan file.ll --format json
culebra scan file.ll --format sarif     # GitHub Code Scanning
culebra scan file.ll --format markdown  # CI 报告
```

### 浏览模板

```bash
culebra templates list
culebra templates list --tags abi
culebra templates show unaligned-string-constant
```

### 运行工作流

工作流将模板与停止条件链接在一起：

```bash
culebra workflow bootstrap-health-check \
  --input stage1_output=stage1.ll

culebra workflow pre-commit \
  --input ir_file=main.ll
```

### 编写自定义模板

模板是 `culebra-templates/` 中的 YAML 文件：

```yaml
id: my-check
info:
  name: 我的自定义检查
  severity: high
  author: yourname
  description: 检测特定的 bug 模式。
  tags:
    - ir
    - custom

scope:
  file_type: llvm-ir
  section: functions

match:
  matchers:
    - type: regex
      name: pattern_name
      pattern:
        - '某些正则表达式模式'
  condition: or

remediation:
  suggestion: "如何修复此问题"
```

任何构建面向 LLVM 的语言的人都可以贡献自己的 bug 模板。引擎不变，知识库增长。

查看 [docs.md](../docs.md) 获取完整规范。

---

## 内置模板

17 个模板分为 4 个类别，每个都来自 Mapanare 的真实 bug。

| 类别 | ID | 严重性 | 检测内容 |
|---|---|---|---|
| **ABI** | `unaligned-string-constant` | 关键 | 奇数地址的字符串常量破坏指针标记 |
| **ABI** | `struct-layout-mismatch` | 关键 | IR 结构体与 C 头文件字段数/类型不一致 |
| **ABI** | `direct-push-no-writeback` | 高 | 通过 GEP 推送列表但无临时 alloca 回写 |
| **ABI** | `sret-input-output-alias` | 高 | sret 指针与输入别名导致计算中数据损坏 |
| **ABI** | `tagged-pointer-odd-address` | 高 | 奇数大小常量无对齐破坏指针标记 |
| **ABI** | `missing-byval-large-struct` | 中 | 大结构体作为裸 ptr 传递但无 byval |
| **IR** | `empty-switch-body` | 关键 | 0 个 case 的 switch -- match 分支未生成 |
| **IR** | `ret-type-mismatch` | 关键 | 返回类型与函数签名不匹配 |
| **IR** | `byte-count-mismatch` | 高 | `[N x i8]` 声明大小与实际内容不同 |
| **IR** | `phi-predecessor-mismatch` | 高 | PHI 节点引用不存在的前驱块 |
| **IR** | `raw-control-byte-in-constant` | 中 | c"..." 中的原始控制字节破坏基于行的工具 |
| **IR** | `unreachable-after-branch` | 中 | 终止符后的指令（死代码）|
| **二进制** | `missing-symbol` | 关键 | 运行时符号在符号表中缺失 |
| **二进制** | `odd-address-rodata` | 高 | .rodata 节中奇数地址的字符串 |
| **Bootstrap** | `function-count-drop` | 关键 | Stage N+1 的函数数少于 Stage N |
| **Bootstrap** | `stage-output-divergence` | 高 | 阶段输出不收敛到不动点 |
| **Bootstrap** | `fixed-point-delta` | 高 | 编译器输出在 N 次迭代后不稳定 |

---

## 所有命令

| 命令 | 功能 |
|---|---|
| `culebra scan file.ll` | 使用 YAML 模板扫描 IR。`--tags`、`--severity`、`--id`、`--format`、`--autofix`。|
| `culebra templates list` | 列出所有可用模板。|
| `culebra templates show <id>` | 显示模板详细信息。|
| `culebra workflow <id>` | 运行多步骤扫描工作流。|
| `culebra strings file.ll` | 验证 `[N x i8] c"..."` 字节计数。|
| `culebra audit file.ll` | 检测 IR 病理：空 switch、ret 不匹配、缺少 `%`。|
| `culebra check file.ll` | 使用 `llvm-as` 验证 IR。|
| `culebra phi-check file.ll` | 验证变换脚本保持 IR 结构。|
| `culebra diff a.ll b.ll` | 按函数结构比较，寄存器规范化。|
| `culebra extract file.ll fn` | 从大型 IR 文件中提取单个函数。|
| `culebra table file.ll` | 按函数指标表。|
| `culebra abi file.ll` | 检测 sret/byref 误用，结构体布局验证。|
| `culebra binary ./binary` | ELF/PE 检查，.rodata 分析，IR 交叉引用。|
| `culebra run compiler source` | 编译、运行、检查预期输出。|
| `culebra test` | 运行 `culebra.toml` 中的所有 `[[tests]]`。|
| `culebra watch` | 监视文件，更改时重新运行命令。|
| `culebra pipeline` | 通过 `culebra.toml` 运行完整阶段管道。|
| `culebra fixedpoint compiler source` | 检测自托管编译器的不动点收敛。|
| `culebra status` | 显示自托管进度。|
| `culebra init` | 生成 `culebra.toml` 模板。|

---

## 架构

```
                        culebra scan file.ll --tags abi
                                    |
                    +---------------+---------------+
                    |                               |
              模板加载器                          IR 解析器
          (culebra-templates/)               (ir.rs -> IRModule)
                    |                               |
                    +----------- 引擎 -------------+
                                    |
                    +---------------+---------------+
                    |               |               |
             正则匹配器         序列匹配器        交叉引用匹配器
            (单行)           (多行，带           (IR vs C 头文件)
                             捕获和缺失检查)
                    |               |               |
                    +---------- 发现结果 ----------+
                                    |
                    +---------------+---------------+
                    |               |               |
                  文本            JSON            SARIF
```

---

## 适用于

- 任何构建面向 LLVM IR 的语言的人
- 任何自托管编译器的人
- 任何调试 ABI 和调用约定问题的人
- 任何运行多阶段 bootstrap 的人
- 任何想将编译器 bug 转化为可重用检测模板的人

---

## 贡献

欢迎贡献。两种贡献方式：

1. **代码** -- Rust 引擎改进、新匹配器类型、输出格式
2. **模板** -- 为你遇到的编译器 bug 添加 YAML 模板

---

## 许可证

MIT 许可证 -- 详见 [LICENSE](../LICENSE)。

---

<div align="center">

**Culebra** -- 你的编译器需要的安全网。

[完整文档](../docs.md) · [报告 Bug](https://github.com/Mapanare-Research/Culebra/issues) · [Mapanare](https://github.com/Mapanare-Research/Mapanare)

由 [Juan Denis](https://juandenis.com) 用心制作

</div>
