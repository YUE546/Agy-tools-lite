use std::process::{Command, Output};
use std::time::Duration;

pub fn output_with_timeout(command: Command, timeout: Duration) -> Result<Output, String> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(async move {
            let mut command = tokio::process::Command::from(command);
            command.kill_on_drop(true);
            tokio::time::timeout(timeout, command.output())
                .await
                .map_err(|_| "Executable version check timed out.".to_string())?
                .map_err(|error| error.to_string())
        })
    })
    .join()
    .map_err(|_| "Executable version check worker failed.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hung_version_command_is_bounded() {
        let mut command = Command::new("sleep");
        command.arg("10");
        let start = std::time::Instant::now();
        assert!(output_with_timeout(command, Duration::from_millis(50))
            .unwrap_err()
            .contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
