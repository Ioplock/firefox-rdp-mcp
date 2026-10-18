//! firefox-rdp-mcp: MCP (stdio) server driving a shared Firefox via `ff-rdp`.
//!
//! Page tools use `ff-rdp --format json <subcommand>`; fixed browser-chrome
//! operations use the same DevTools RDP endpoint directly
//! (default 127.0.0.1:6000, the `agent-firefox` instance) and returns the
//! structured JSON envelope. Configure with:
//!   FF_RDP_BIN     path to ff-rdp (default: ff-rdp on PATH)
//!   FF_RDP_HOST    RDP host (default: 127.0.0.1, keep loopback-only)
//!   FF_RDP_PORT    RDP port (default: 6000)
//!   FF_RDP_TIMEOUT_MS per-command timeout in milliseconds (default: 8000)

mod browser;
mod compact;
mod error;
mod ff_rdp;
mod tools;

use rmcp::{tool_handler, transport::stdio, ServiceExt};

use crate::{ff_rdp::Backend, tools::FirefoxRdp};

#[tool_handler(name = "firefox-rdp-mcp", version = "0.2.0")]
impl rmcp::ServerHandler for FirefoxRdp {
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        let timeout_ms = request
            .arguments
            .as_ref()
            .and_then(|a| a.get("timeout_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(25_000)
            .clamp(100, 120_000);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        let name = request.name.to_string();
        let started = tokio::time::Instant::now();
        let cancel = context.ct.clone();
        let work = async {
            let _guard = self.operation_lock.lock().await;
            let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
            Self::tool_router().call(tcc).await
        };
        let result = ff_rdp::DEADLINE.scope(deadline, async {
            tokio::select! {
                r = tokio::time::timeout_at(deadline, work) => r.unwrap_or_else(|_| Err(error::internal(
                    "tool deadline exceeded; action may already have occurred — inspect before retrying"))),
                _ = cancel.cancelled() => Err(error::internal("tool cancelled; inspect page before retrying an action")),
            }
        }).await;
        tracing::info!(
            tool = name,
            elapsed_ms = started.elapsed().as_millis() as u64,
            success = result.is_ok(),
            "tool finished"
        );
        result
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let backend = Backend::from_env();
    tracing::info!(
        "firefox-rdp-mcp starting: ff-rdp={} rdp={}:{}",
        backend.exe,
        backend.host,
        backend.port
    );

    let service = FirefoxRdp::new(backend)
        .serve(stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("serve error: {e:?}");
        })?;
    service.waiting().await?;
    Ok(())
}
