use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use tokio::time::MissedTickBehavior;

use crate::{app::AppContext, config::ConfigData, error::Result, infra::signal::draining};

type TaskFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

struct Task {
    name: &'static str,
    interval: Duration,
    /// 每轮被调用一次。
    run: fn(AppContext) -> TaskFuture,
}

fn tasks(cfg: &ConfigData) -> Vec<Task> {
    vec![Task {
        name: "files.gc",
        interval: cfg.storage.gc_interval(),
        run: files_gc,
    }]
}

/// 每个任务一条 tokio 任务并等待；调用方负责 abort。
pub async fn run(ctx: AppContext) {
    let running: Vec<_> = tasks(ctx.cfg())
        .into_iter()
        .map(|task| tokio::spawn(run_task(ctx.clone(), task)))
        .collect();

    for task in running {
        let _ = task.await;
    }
}

async fn run_task(ctx: AppContext, task: Task) {
    let mut ticker = tokio::time::interval(task.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // 吃掉 tokio 立即触发的首 tick：首轮执行落在一个完整周期之后。
    ticker.tick().await;

    let mut stopping = ctx.shutdown();
    loop {
        tokio::select! {
            _ = draining(&mut stopping) => break,
            _ = ticker.tick() => {
                let started = Instant::now();
                match (task.run)(ctx.clone()).await {
                    Ok(()) => tracing::info!(
                        task = task.name,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "scheduled task finished"
                    ),
                    Err(e) => tracing::warn!(task = task.name, error = %e, "scheduled task failed"),
                }
            }
        }
    }
}

fn files_gc(ctx: AppContext) -> TaskFuture {
    Box::pin(async move {
        let storage = ctx.cfg().storage.clone();

        let swept = ctx
            .srv()
            .file
            .sweep_unattached(storage.unattached_ttl())
            .await?;
        let report = ctx.srv().file.reconcile(storage.orphan_grace()).await?;

        tracing::info!(
            swept,
            orphans_deleted = report.orphans_deleted,
            missing_blobs = report.missing_blobs,
            size_mismatches = report.size_mismatches,
            "file gc finished"
        );
        Ok(())
    })
}
