# 008-SECURITY

## 1. 本文档目的

本文档定义 amagi 的认证、授权、vault 访问控制、step-up auth 与 WebAuthn 相关设计。

它覆盖：

- OIDC 登录
- base session
- step-up auth
- vault unlock
- WebAuthn / passkey
- 基本审计要求

相关文档：

- 架构：`001-ARCHITECTURE.md`
- 领域模型：`002-DOMAIN-MODEL.md`
- API：`004-API.md`

---

## 2. 安全目标

### 2.1 目标

- 使用自定义 OIDC 作为主登录
- 支持基于 session 的 Dashboard 访问
- 支持浏览器扩展受控访问
- 支持 vault 的二次解锁
- 支持 WebAuthn / passkey 作为强认证手段
- 审计关键安全事件

### 2.2 非目标（首阶段）

- 完整端到端加密收藏系统
- 隐身浏览器端零明文缓存架构
- 企业级多租户复杂 IAM

---

## 3. 认证模型

### 3.1 Base Login

用户通过 OIDC 登录，获得基础会话。

基础会话可访问：

- normal libraries
- devices
- profiles
- search（不含 vault）
- 基础同步管理页

基础会话不自动授予 vault 访问权。

### 3.2 Step-up Auth

当用户访问高敏感操作时，需要更强认证上下文。
典型场景：

- 解锁 vault
- 查看 vault 搜索结果
- 执行敏感安全设置修改

step-up 可通过：

- 重新完成更高 ACR 的 OIDC 认证
- WebAuthn assertion

### 3.3 Unlock Session

step-up 成功后，系统发放短期 unlock session。
unlock session 作用域至少包括：

- `user_id`
- `library_id`
- `expires_at`

unlock session 仅影响 vault 可见性，不应替代普通登录 session。

---

## 4. OIDC 设计

### 4.1 角色

amagi 作为 OIDC Relying Party。

### 4.2 登录流程

推荐：

- Authorization Code Flow
- PKCE

### 4.3 会话绑定

服务端维护自身 session，不直接把第三方 token 当内部权限载体的唯一来源。

### 4.4 需要记录的信息

- `sub`
- `email`
- `name`
- 可选 `acr`
- 可选 `amr`

### 4.5 会话升级

若 OIDC Provider 支持更高认证上下文，可在 vault unlock 时触发 step-up 登录。

---

## 5. WebAuthn / Passkey

### 5.1 用途

WebAuthn 主要用于：

- vault 二次解锁
- 高敏感设置确认

### 5.2 注册

用户在已登录状态下可注册 passkey。

### 5.3 断言

vault unlock 时可要求 WebAuthn assertion。

### 5.4 设备记忆

首阶段建议保守：

- 不默认“记住设备长期免二次解锁”
- 若要支持，也应有较短 TTL 与显式风险说明

---

## 6. Vault 访问控制

### 6.1 默认不可见

vault 默认不出现在：

- 普通 tree 浏览
- 普通搜索结果
- 普通 sync feed
- 普通扩展本地缓存

### 6.2 访问条件

访问某 vault library 至少要求：

- 用户已登录
- 对应 library 的 unlock session 有效

### 6.3 unlock TTL

建议可配置，例如：

- 默认 5~30 分钟
- 过期后需重新 step-up

### 6.4 主动锁定

应支持用户主动 lock 当前 vault。

---

## 7. 授权模型

### 7.1 首阶段

首阶段可采用 owner-only 模型：

- 用户仅访问自己的 libraries / devices / profiles

### 7.2 后续扩展

若支持共享，可增加：

- viewer
- editor
- admin

但不应影响 vault 基础模型。

---

## 8. 扩展侧安全

### 8.1 浏览器扩展会话

扩展需要自己的受控会话，与 Dashboard session 关联但不完全等同。

### 8.2 本地存储最小化

不要在普通扩展本地状态中持久化 vault 内容。
不要长期保存高敏感 unlock 状态。

### 8.3 capability 报告

扩展注册时应报告能力，但服务端不能仅信任自报。
对于高风险动作仍需服务端 policy 限制。

---

## 9. 搜索与可见性

### 9.1 普通搜索

默认只搜索 normal libraries。

### 9.2 vault 搜索

只有在 unlock 有效时才能包含 vault 内容。

### 9.3 审计

应记录：

- 用户何时解锁了哪个 vault
- 解锁持续多久
- 是否主动锁定
- 是否解锁失败

---

## 10. 审计事件建议

至少记录：

- user_login
- user_logout
- step_up_started
- step_up_succeeded
- step_up_failed
- webauthn_registered
- webauthn_asserted
- vault_unlock_succeeded
- vault_unlock_failed
- vault_locked
- profile_changed
- sync_apply

---

## 11. 未来加密扩展

### 11.1 首阶段建议

先做服务端控制的 vault 可见性与二次解锁，不做端到端加密。

### 11.2 后续可演进方向

若未来需要更强隐私，可引入：

- vault 内容加密存储
- 用户侧密钥包装
- 本地解密渲染
- 有限制的搜索能力

但这将显著增加复杂度，不应影响首阶段推进。

---

## 12. 安全边界总结

必须坚持：

1. base session != vault access
2. unlock session 是短期且可撤销的
3. vault 不进入普通 sync feed
4. vault 不进入普通扩展缓存
5. 所有关键安全行为都有审计

---

## 13. 与其他文档关系

- API：`004-API.md`
- 同步：`005-SYNC.md`
- 浏览器适配：`006-BROWSER-ADAPTERS.md`
