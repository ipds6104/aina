use crate::core::usecases::ScheduledTickUseCase;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

pub struct SchedulerRunner {
    usecase: Arc<ScheduledTickUseCase>,
    interval_duration: Duration,
}

impl SchedulerRunner {
    pub fn new(usecase: Arc<ScheduledTickUseCase>, interval_seconds: u64) -> Self {
        Self {
            usecase,
            interval_duration: Duration::from_secs(interval_seconds),
        }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            info!(
                "Starting background scheduler loop (tick every {:?})",
                self.interval_duration
            );
            let mut ticker = interval(self.interval_duration);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = self.usecase.execute().await {
                    error!("Scheduler tick encountered error: {:?}", e);
                }
            }
        });
    }
}
