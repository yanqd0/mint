//! CLI 入口分发：project/db 路径解析、多 db 迁移、命令路由。

use std::path::{Path, PathBuf};

use clap::CommandFactory;
use rusqlite::Connection;

use crate::cli::label::{cmd_label_list, cmd_label_set};
use crate::cli::{Cli, Commands, LabelCmd, ProjectCmd, SyncCmd};
use crate::cli::{delete, doctor, export, help_llm, import, issue, milestone, plan, project, sync};
use crate::error::Error;

use super::sync::hint::detect_unmerged_machines;

impl Cli {
    /// 执行命令分发。
    pub fn run(&self) -> Result<(), Error> {
        // `--help-llm` 是纯输出：先于 project/db 解析返回，保证零副作用（不建库、不迁移）。
        if self.help_llm {
            return help_llm::cmd_help_llm();
        }
        // 无子命令：保持 clap 用法错误语义（退出码 2），并指路 `--help-llm`（#466）。
        let Some(command) = self.command.as_ref() else {
            let mut cmd = Cli::command();
            cmd.error(
                clap::error::ErrorKind::MissingSubcommand,
                "no command given: run `mint --help-llm` for the full reference",
            )
            .exit()
        };
        let cwd = std::env::current_dir()?;
        // project 检测：纯函数（detect_name），先于 open（多 db 按 project 定位路径）。
        let project = self.resolve_project(&cwd)?;
        // 一次性迁移：旧单一 db → 多项目 db（仅缺省路径；显式 --db 不迁移）。
        self.maybe_split_legacy()?;
        // 不需要当前项目 db 的命令（project create/list、sync --all）在任意目录运行
        // 不应物化假项目（open+ensure 会新建 projects/<dirname>/<machine>.db 并注册行，#399）。
        let needs_conn = match command {
            Commands::Project(p) => {
                !matches!(p.command, ProjectCmd::Create(_) | ProjectCmd::List(_))
            }
            Commands::Sync(s) => {
                !(matches!(&s.command, SyncCmd::Push(p) if p.all)
                    || matches!(&s.command, SyncCmd::Pull(p) if p.all)
                    || matches!(&s.command, SyncCmd::Merge(m) if m.all))
            }
            _ => true,
        };
        let mut conn = if needs_conn {
            let path = self.db_path(&project);
            let c = crate::db::open(&path)?;
            // 当前项目 db 内确保 project 行（每 db 单行本项目）。
            crate::project::ensure(&c, &project, &cwd)?;
            c
        } else {
            Connection::open_in_memory()?
        };

        // 无感多 db（#428）：读命令检测未合并的其他机器数据，提示 sync pull 聚合。
        // --db 单文件模式无多机语义，跳过（#437）；项目模式才检测，降频热路径开销（#438）。
        if needs_conn
            && self.db.is_none()
            && matches!(
                command,
                Commands::List(_) | Commands::Show(_) | Commands::Search(_)
            )
        {
            let others = detect_unmerged_machines(&conn, &self.data_dir(), &project);
            if !others.is_empty() {
                eprintln!(
                    "mint: hint: found unmerged data from machine(s): {}; run `mint sync pull` to view the full picture",
                    others.join(", ")
                );
            }
        }

        match command {
            Commands::Issue(i) => issue::dispatch(&mut conn, &cwd, &project, &i.command),
            Commands::List(l) => issue::list::cmd_list(&conn, &project, l),
            Commands::Show(s) => issue::list::cmd_show(&conn, &project, s),
            Commands::Search(s) => issue::list::cmd_search(&conn, &project, s),
            Commands::Doctor(d) => doctor::cmd_doctor(&conn, d),
            Commands::Label(t) => match &t.command {
                LabelCmd::List(l) => cmd_label_list(&conn, l),
                LabelCmd::Set(s) => cmd_label_set(&conn, s),
            },
            Commands::Project(p) => project::dispatch(&conn, &self.data_dir(), &p.command),
            Commands::Milestone(r) => milestone::dispatch(&conn, &project, &r.command),
            Commands::Plan(p) => plan::dispatch(&conn, &project, &p.command),
            Commands::Delete(d) => delete::dispatch(&conn, &self.data_dir(), &d.command),
            #[cfg(feature = "tui")]
            Commands::Tui => crate::tui::run_dashboard(&conn, &project),
            Commands::Export(a) => export::cmd_export(&conn, a),
            Commands::Import(a) => import::cmd_import(&mut conn, a),
            Commands::Sync(s) => sync::cmd_sync(&mut conn, &self.data_dir(), s),
        }
    }

    /// 解析当前 project（detect_name 纯函数；ensure 在 open 后由 run 调用）。
    fn resolve_project(&self, cwd: &std::path::Path) -> Result<String, Error> {
        let name = self
            .project
            .clone()
            .unwrap_or_else(|| crate::project::detect_name(cwd, None));
        // 名字将拼入 projects/<name>/<machine>.db 路径：校验拒绝 .. / 分隔符（#393）。
        crate::project::validate_project_name(&name)?;
        Ok(name)
    }

    /// 数据库路径：--db/MINT_DB_PATH 显式单文件；缺省 <data>/projects/<project>/<machine_id>.db
    /// （db 名含 machine 信息，多机多 db 同步简洁：项目目录下每机器一个 db 文件）。
    fn db_path(&self, project: &str) -> PathBuf {
        if let Some(p) = &self.db {
            return p.clone();
        }
        self.data_dir()
            .join("projects")
            .join(project)
            .join(format!("{}.db", crate::db::machine_id()))
    }

    /// 数据目录：--db 显式时为其父目录（多项目 db 的根，测试隔离也走这里）；
    /// 缺省 $XDG_DATA_HOME/mint（或 HOME/.local/share/mint；均缺省 "."）。
    fn data_dir(&self) -> PathBuf {
        if let Some(p) = &self.db {
            return p.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        }
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| PathBuf::from("."))
            })
            .join("mint")
    }

    /// 一次性迁移：旧单一 db → 多项目 db（仅缺省路径；显式 --db 由用户管理，不迁移）。
    fn maybe_split_legacy(&self) -> Result<(), Error> {
        if self.db.is_some() {
            return Ok(());
        }
        crate::db::migrate_split::maybe_split(&self.data_dir())
    }
}

// ── 共享 helpers（plan/milestone 共用）───────────────────────────────
