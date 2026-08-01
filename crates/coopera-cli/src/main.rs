//! coopera — team context harness engine.
//!
//! Internal engine: normally invoked by editor/agent hooks, not by developers
//! (product principle: zero explicit user commands). All output is English.

mod cmd_distill;
mod cmd_hook;
mod cmd_init;
mod cmd_wiki;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "coopera",
    version,
    about = "Team context harness engine (invoked by hooks)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install the harness into the current git repository (F1).
    Init,
    /// Hook entrypoints — read hook JSON on stdin, emit hook JSON on stdout.
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
    /// Distill a session transcript into a digest + wiki diff (F3).
    Distill {
        /// Path to the session transcript to distill.
        #[arg(long)]
        transcript: Option<String>,
        /// Session id for attribution (defaults to the transcript file stem).
        #[arg(long)]
        session: Option<String>,
        /// Tool that hosted the session (selects the distiller agent —
        /// decision 003). Defaults to claude-code.
        #[arg(long)]
        tool: Option<String>,
        /// Retroactively distill sessions left in the retro queue.
        #[arg(long)]
        retro: bool,
    },
    /// Wiki maintenance commands.
    Wiki {
        #[command(subcommand)]
        cmd: WikiCmd,
    },
    /// Print harness status for the current repository.
    Status,
}

#[derive(Subcommand)]
enum HookEvent {
    /// SessionStart: presence fetch/publish + team-context injection (F2/F5/F7).
    SessionStart,
    /// UserPromptSubmit: intent refresh + trigger-matched injection (F6).
    UserPromptSubmit,
    /// SessionEnd: presence cleanup + background distillation (F3/F5).
    SessionEnd,
    /// PreInvocation (Antigravity): presence + first-call injection (F8b).
    PreInvocation,
    /// Stop (Antigravity): capture marker + presence heartbeat (F8b).
    Stop,
}

#[derive(Subcommand)]
enum WikiCmd {
    /// Validate all wiki pages against the schema (F4).
    Lint,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.cmd {
        Cmd::Init => cmd_init::run(),
        Cmd::Hook { event } => match event {
            HookEvent::SessionStart => cmd_hook::session_start(),
            HookEvent::UserPromptSubmit => cmd_hook::user_prompt_submit(),
            HookEvent::SessionEnd => cmd_hook::session_end(),
            HookEvent::PreInvocation => cmd_hook::pre_invocation(),
            HookEvent::Stop => cmd_hook::stop(),
        },
        Cmd::Distill {
            transcript,
            session,
            tool,
            retro,
        } => cmd_distill::run(
            transcript.as_deref(),
            session.as_deref(),
            tool.as_deref(),
            retro,
        ),
        Cmd::Wiki { cmd } => match cmd {
            WikiCmd::Lint => cmd_wiki::lint(),
        },
        Cmd::Status => cmd_init::status(),
    };
    std::process::exit(code);
}
