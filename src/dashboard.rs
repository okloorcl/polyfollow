use std::path::Path;

use anyhow::{Context, Result};

use crate::config::AppConfig;
use crate::storage::Storage;

pub fn render_dashboard(
    cfg: &AppConfig,
    storage: &Storage,
    out: &Path,
    limit: usize,
) -> Result<()> {
    let status = storage.status()?;
    let pnl = storage.pnl_summary()?;
    let orders = storage.recent_intents(limit)?;
    let logs = storage.recent_logs(limit)?;
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>PolyFollow Dashboard</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 0; background: #f7f7f4; color: #191919; }}
    header {{ padding: 28px 36px; background: #101820; color: white; }}
    main {{ padding: 28px 36px; display: grid; gap: 24px; }}
    section {{ background: white; border: 1px solid #ddd; border-radius: 8px; padding: 18px; }}
    .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; }}
    .metric {{ border: 1px solid #e3e3df; border-radius: 8px; padding: 14px; }}
    .metric span {{ display: block; color: #666; font-size: 12px; }}
    .metric strong {{ display: block; font-size: 22px; margin-top: 4px; }}
    table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
    th, td {{ border-bottom: 1px solid #ececea; padding: 8px; text-align: left; vertical-align: top; }}
    th {{ color: #555; font-weight: 600; }}
    code {{ font-size: 12px; }}
  </style>
</head>
<body>
  <header>
    <h1>PolyFollow Dashboard</h1>
    <p>Mode: {mode} · leaders: {leaders} · database: {db}</p>
  </header>
  <main>
    <section>
      <h2>Snapshot</h2>
      <div class="grid">
        {metrics}
      </div>
    </section>
    <section>
      <h2>Recent Copy Intents</h2>
      <table>
        <thead><tr><th>Time</th><th>Leader</th><th>Side</th><th>Notional</th><th>Verdict</th><th>Reasons</th></tr></thead>
        <tbody>{orders}</tbody>
      </table>
    </section>
    <section>
      <h2>Observed Leader Trades</h2>
      <table>
        <thead><tr><th>Time</th><th>Leader</th><th>Trade</th><th>Source</th><th>Status</th></tr></thead>
        <tbody>{logs}</tbody>
      </table>
    </section>
  </main>
</body>
</html>"#,
        mode = html_escape(&format!("{:?}", cfg.global.mode)),
        leaders = cfg.leaders.len(),
        db = html_escape(&status.db_path),
        metrics = metrics_html(&status, &pnl),
        orders = orders
            .iter()
            .map(|row| format!(
                "<tr><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(&row.created_at),
                html_escape(&short(&row.leader_address)),
                html_escape(&row.side),
                html_escape(&row.notional_usdc),
                html_escape(&row.verdict),
                html_escape(&row.reasons_json),
            ))
            .collect::<String>(),
        logs = logs
            .iter()
            .map(|row| format!(
                "<tr><td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                html_escape(&row.observed_at),
                html_escape(&short(&row.leader_address)),
                html_escape(&short(&row.trade_id)),
                html_escape(&row.source),
                html_escape(&row.status),
            ))
            .collect::<String>(),
    );
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(out, html).with_context(|| format!("failed to write {}", out.display()))?;
    Ok(())
}

fn metrics_html(
    status: &crate::storage::StorageStatus,
    pnl: &crate::storage::PnlSummary,
) -> String {
    [
        ("Processed trades", status.processed_trade_count.to_string()),
        ("Copy intents", status.copy_intent_count.to_string()),
        ("Open fills", pnl.open_paper_fills.to_string()),
        ("Closed fills", pnl.closed_paper_fills.to_string()),
        ("Open notional", pnl.open_notional_usdc.clone()),
        ("Realized PnL", pnl.realized_pnl_usdc.clone()),
    ]
    .into_iter()
    .map(|(label, value)| {
        format!(
            "<div class=\"metric\"><span>{label}</span><strong>{}</strong></div>",
            html_escape(&value)
        )
    })
    .collect()
}

fn short(value: &str) -> String {
    if value.len() <= 14 {
        return value.to_string();
    }
    format!("{}...{}", &value[..8], &value[value.len() - 6..])
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
