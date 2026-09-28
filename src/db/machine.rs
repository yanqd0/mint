//! 本机标识（machine_id）：生成、校验、持久化与 `machines` 表注册。
//!
//! db 名含 machine 信息，多机多 db；主机名/用户变化不影响既有 id（优先读持久化文件）。

use std::path::PathBuf;

use crate::db::{MACHINE_BACKFILL_UID, MACHINE_UPSERT, PLAN_BACKFILL_UID};
use crate::error::Error;

/// 本机 machine_id：MINT_MACHINE_ID env 优先，否则 hostname+user 的 FNV-1a 哈希（mach-<hex>）。
/// FNV-1a 稳定（不随工具链变化），适合持久身份键；改 hostname 会变
/// （接受；CI/容器/VM 克隆用 MINT_MACHINE_ID 显式固定）。env 值需为 [A-Za-z0-9_-]
/// 且不含 ':'（否则破坏 uid 格式），非法则忽略回退机器特征。
pub fn machine_id() -> String {
    if let Some(mid) = std::env::var("MINT_MACHINE_ID")
        .ok()
        .filter(|m| is_valid_machine_id(m.trim()))
    {
        return mid.trim().to_string();
    }
    // 持久化 machine.json：已存在则复用（hostname 变化不再重算，避免同项目 db 碎片）。
    let path = machine_info_path();
    if let Some(text) = std::fs::read_to_string(&path).ok()
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(id) = v.get("machine_id").and_then(serde_json::Value::as_str)
        && is_valid_machine_id(id)
    {
        // 读回同样校验字符集：非法值（文件被篡改/手工编辑）忽略回退重生成，防污染路径（#397）。
        return id.to_string();
    }
    // 首次：基于 hostname+user 生成，写 json（含可简单获取的机器信息，参考用），后续读文件。
    let hostname = whoami::fallible::hostname().unwrap_or_default();
    let username = whoami::username();
    let id = format!(
        "mach-{:08x}",
        fnv1a(&format!("{hostname}|{username}")) & 0xffff_ffff
    );
    persist_machine_info(&id, &hostname, &username);
    id
}

/// machine_id 字符集约束（env 与 machine.json 读回共用）：`[A-Za-z0-9_-]`、非空、不含 ':'。
/// machine_id 会拼入 db 文件名 / 快照文件名，非法字符可破坏路径或逃逸目录（#397）。
pub(super) fn is_valid_machine_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains(':')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// machine.json 路径（`$XDG_DATA_HOME/mint/` 下，projects/ 的父级，machine 级共享；
/// 零新依赖，serde_json 已有）。内容 = machine_id + 可简单获取的机器信息
/// （hostname/username/os/arch，仅首次记录，供参考）。
pub(super) fn machine_info_path() -> PathBuf {
    let dir = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".local/share"))
                .unwrap_or_else(|_| PathBuf::from("."))
        })
        .join("mint");
    dir.join("machine.json")
}

/// 首次生成 machine_id 时持久化：写 JSON（machine_id + 机器信息，供参考；仅首次）。
/// 权限与 db 一致（目录 0700 / 文件 0600），防本地其他用户读取 hostname/username（#397）；
/// 旧文件损坏时保留为 `.json.bak`（防覆盖致 machine_id 漂移、旧 db 变孤儿）。
pub(super) fn persist_machine_info(id: &str, hostname: &str, username: &str) {
    let path = machine_info_path();
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent);
        }
        #[cfg(not(unix))]
        let _ = std::fs::create_dir_all(parent);
    }
    // 旧文件存在但不可解析（损坏）：保留备份再写新值。
    if let Ok(existing) = std::fs::read_to_string(&path)
        && serde_json::from_str::<serde_json::Value>(&existing).is_err()
    {
        let _ = std::fs::rename(&path, path.with_extension("json.bak"));
    }
    let info = serde_json::json!({
        "machine_id": id,
        "hostname": hostname,
        "username": username,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });
    let content = serde_json::to_string_pretty(&info).unwrap_or_default();
    #[cfg(unix)]
    {
        // tmp + rename 原子写，落 0600（rename 保留目标权限，tmp 先设好）。
        use std::os::unix::fs::PermissionsExt;
        let tmp = path.with_extension("json.tmp");
        let _ = std::fs::write(&tmp, &content);
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        let _ = std::fs::rename(&tmp, &path);
    }
    #[cfg(not(unix))]
    let _ = std::fs::write(&path, &content);
}

/// FNV-1a 64 位哈希（稳定：不随 Rust 工具链/release 变化，作持久身份键用）。
pub(super) fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// 注册本机 machine 行（如实记录 hostname/user；已存在则更新反映当前）。
pub(super) fn register_machine(conn: &rusqlite::Connection) -> Result<(), Error> {
    let hostname = whoami::fallible::hostname().unwrap_or_default();
    conn.execute(
        MACHINE_UPSERT,
        rusqlite::params![machine_id(), hostname, whoami::username()],
    )?;
    // 回填存量 issue 的 uid（machine_id 已知后；跨机幂等键）
    conn.execute(MACHINE_BACKFILL_UID, [])?;
    // 回填存量 plan 的 uid（#498）
    conn.execute(PLAN_BACKFILL_UID, [machine_id()])?;
    Ok(())
}
