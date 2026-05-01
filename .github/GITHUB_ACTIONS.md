# GitHub Actions 构建指南

本项目已配置GitHub Actions自动化构建和发布流程。

## 工作流说明

### 1. CI 构建 (`.github/workflows/ci.yml`)

**触发条件**：
- Push到 `main`, `master`, `develop` 分支
- Pull Request到 `main`, `master`, `develop` 分支

**执行任务**：
- ✅ 代码检查 (cargo check)
- ✅ 单元测试 (cargo test)
- ✅ 代码格式检查 (cargo fmt)
- ✅ Clippy检查 (cargo clippy)
- ✅ 构建x86_64版本
- ✅ 构建Web前端
- ✅ 构建Docker镜像 (仅main分支)

**输出物**：
- `one-kvm-x86_64` - x86_64架构可执行文件
- `one-kvm-web` - Web前端dist目录

### 2. 交叉编译 (`.github/workflows/cross-compile.yml`)

**触发条件**：
- Push到 `main`, `master`, `develop` 分支（修改了src/、Cargo.toml或build/目录）
- Pull Request到 `main`, `master`, `develop` 分支
- 手动触发 (workflow_dispatch)

**执行任务**：
- ✅ 构建ARM64 (aarch64) 版本
- ✅ 构建ARMv7版本

**输出物**：
- `one-kvm-aarch64` - ARM64架构可执行文件
- `one-kvm-armv7` - ARMv7架构可执行文件

### 3. 发布构建 (`.github/workflows/release.yml`)

**触发条件**：
- 创建GitHub Release
- 手动触发 (workflow_dispatch)

**执行任务**：
- ✅ 构建多平台版本 (x86_64, ARM64, ARMv7)
- ✅ 生成SHA256校验和
- ✅ 上传到Release中

**输出物**：
- `one-kvm-x86_64-linux` - x86_64二进制
- `one-kvm-aarch64-linux` - ARM64二进制
- `one-kvm-armv7-linux` - ARMv7二进制
- `SHA256SUMS` - 校验和文件

## 快速开始

### 第一次设置

1. **提交工作流文件**：
```bash
git add .github/workflows/
git commit -m "Add GitHub Actions CI/CD workflows"
git push origin main
```

2. **检查工作流**：
   - 访问 https://github.com/你的用户名/One-KVM/actions
   - 查看工作流运行情况

### 创建发布版本

1. **创建版本标签**：
```bash
git tag v0.1.10
git push origin v0.1.10
```

2. **在GitHub上创建Release**：
   - 访问 https://github.com/你的用户名/One-KVM/releases
   - 点击 "Create a new release"
   - 选择你的标签
   - 添加发布说明
   - 点击 "Publish release"

3. **GitHub Actions自动**：
   - 构建所有平台版本
   - 上传到Release

### 手动触发工作流

在GitHub Actions页面，选择工作流后点击"Run workflow"按钮即可手动触发。

## 故障排查

### 构建失败

1. **查看日志**：
   - 点击具体的工作流运行
   - 查看失败的步骤日志

2. **常见问题**：
   - **依赖问题**：检查是否有系统依赖缺失
   - **编译错误**：检查代码是否有语法错误
   - **超时**：大型编译可能需要更长时间，可设置更长的超时时间

### 修改工作流

编辑`.github/workflows/`中的YAML文件后，push到仓库即可自动应用新的工作流配置。

## Secrets 配置（Docker Registry发布）

如果要自动发布Docker镜像到Docker Hub或GitHub Container Registry，需要配置Secrets：

### 发布到Docker Hub

1. 访问 https://github.com/你的用户名/One-KVM/settings/secrets
2. 点击 "New repository secret"
3. 添加：
   - `DOCKER_USERNAME` - Docker Hub用户名
   - `DOCKER_PASSWORD` - Docker Hub token

### 发布到GitHub Container Registry

1. 创建 [Personal Access Token](https://github.com/settings/tokens)
2. 添加Secret：
   - `GHCR_PAT` - GitHub Container Registry token

## 性能优化

工作流已配置以下优化：

- **缓存**：使用Rust缓存加速构建（`Swatinem/rust-cache`）
- **并行构建**：多个平台同时构建
- **选择性触发**：仅在相关文件修改时触发交叉编译

## 许可证

遵循项目的 GPL-2.0 许可证

---

更多信息：[GitHub Actions 文档](https://docs.github.com/en/actions)
