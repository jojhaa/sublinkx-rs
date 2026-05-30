# sublinkx-rs Rewrite Plan

## 1. Goal

Rebuild the old `sublinkX-2.1` system into a maintainable, secure, multi-protocol subscription platform.

Core goals:

- Support many node protocols without repeated large refactors
- Separate protocol parsing, storage, rendering, and frontend editing
- Replace weak security design with explicit auth and token boundaries
- Keep the product focus: node management, subscription management, template management, and client export

## 2. High-Level Architecture

```text
sublinkx-rs/
  backend/
  frontend/
  docs/
```

Backend stack:

- Rust
- Axum
- SQLx
- SQLite first, PostgreSQL-ready
- Serde
- JsonWebToken
- Argon2
- Reqwest
- Tracing

Frontend stack:

- Vue 3
- Vite
- TypeScript
- Pinia
- Vue Router
- Element Plus

## 3. Key Differences From The Legacy Project

Legacy project:

- Function-first structure
- Link-string-driven node model
- Protocol support added with manual branching
- Plaintext password verification
- Predictable subscription token design

New project:

- Domain-first structure
- Structured node model with protocol metadata
- Protocol registry architecture
- Argon2 password hashes
- Random subscription tokens
- Safer remote fetch and template handling

## 4. Folder Plan

### 4.1 Backend

```text
backend/
  src/
    main.rs
    app.rs
    state.rs
    api/
    config/
    db/
    domain/
    dto/
    errors/
    middleware/
    protocols/
    repository/
    services/
    utils/
  migrations/
```

### 4.2 Frontend

```text
frontend/
  src/
    api/
    components/
    layouts/
    router/
    store/
    styles/
    types/
    utils/
    views/
```

## 5. Data Model Principles

Node data must use:

- Stable common fields
- `protocol` type
- `settings_json` for protocol-specific fields
- `raw_link` for compatibility and traceability
- `fingerprint` for deduplication
- `source_type` and `source_ref` for import origin

Subscriptions must use:

- Random token
- Explicit node ordering table
- Optional client default and template binding

Templates should be:

- Stored in database first
- Written to files only if a clear requirement appears later

## 6. Protocol Expansion Strategy

The backend must use a protocol registry model.

Each protocol module should provide:

- Link parser
- Structured validator
- Capability declaration
- Render support per client type

Planned protocol module layout:

```text
backend/src/protocols/
  mod.rs
  traits.rs
  registry.rs
  ss.rs
  ssr.rs
  vmess.rs
  vless.rs
  trojan.rs
  hysteria.rs
  hysteria2.rs
  tuic.rs
  wireguard.rs
```

Design rule:

- Adding one protocol should not require editing unrelated business logic

## 6.1 Client Compatibility Strategy

The new project must target current mainstream clients, not only the legacy output names.

Primary target families:

- Mihomo / Clash-family clients
- v2rayN / Xray-family clients
- Surge-family clients
- sing-box-family clients

Detailed targets and compatibility rules are documented in:

- [client-compatibility.md](D:\Desktop\tool\sublinkx-rs\docs\client-compatibility.md)
- [client-target-registry.md](D:\Desktop\tool\sublinkx-rs\docs\client-target-registry.md)

## 7. Security Baseline

Must-have changes:

- Passwords stored with Argon2
- Admin auth by JWT
- Public subscription access token separated from admin auth
- Subscription token must be random and rotatable
- Remote fetch restricted by timeout, size, scheme, and target validation
- Template operations restricted to trusted boundaries

## 8. Phased Development Plan

### Phase 0: Planning

Deliverables:

- Project skeleton
- Rewrite blueprint
- Data model design
- Protocol expansion rules

### Phase 1: Backend Foundation

Deliverables:

- Rust workspace init
- Axum app bootstrap
- Config loading
- Tracing and error middleware
- SQLx setup and first migration
- User model and admin bootstrap

### Phase 2: Authentication And Admin Base

Deliverables:

- Login API
- JWT middleware
- Current user API
- Change password API

### Phase 3: Node Domain

Deliverables:

- Node group CRUD
- Node CRUD
- Import raw links
- Parse into structured protocol settings
- Dedup with fingerprint

### Phase 4: Subscription Domain

Deliverables:

- Subscription CRUD
- Bind nodes with sort order
- Rotate subscription token
- Enable or disable subscriptions

### Phase 5: Render And Export

Deliverables:

- Unified canonical node pipeline
- Mihomo renderer
- Surge renderer
- Xray URI bundle renderer
- sing-box outbound renderer
- Client-family compatibility filtering

### Phase 6: Frontend Admin

Deliverables:

- Login page
- Dashboard
- Node management
- Subscription management
- Template management
- Settings page

### Phase 7: Hardening

Deliverables:

- Access logs
- Remote fetch guardrails
- Test coverage for protocol parsing
- Docker deployment files

## 9. MVP Scope

First usable version should include only:

- Admin login
- Node CRUD
- Subscription CRUD
- Node ordering
- Mihomo / Surge / Xray export
- Random token subscription access

Not in first milestone:

- Multi-role permission system
- Advanced dashboards
- Complex template inheritance
- Background sync scheduler

## 10. Immediate Next Steps

Recommended implementation order:

1. Initialize Rust backend workspace
2. Write first SQLx migrations
3. Define core Rust domain types for node and subscription
4. Bootstrap Vue 3 frontend
5. Connect auth flow before protocol-heavy work

## 11. Non-Negotiable Design Rules

- Do not couple protocol parsing to controller handlers
- Do not store plaintext passwords
- Do not use predictable subscription tokens
- Do not let protocol-specific fields explode into one giant table schema
- Do not mix admin auth with public subscription access
