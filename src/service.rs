use std::time::Duration;

use log::{debug, error, info, warn};
use subprocess::{ExitStatus, Popen};

#[derive(Debug)]
pub struct TestServiceProcess {
    child: Option<Popen>,
    service_name: &'static str,
    test_name: String,
}

impl TestServiceProcess {
    pub fn new(service_name: &'static str, test_name: impl Into<String>, child: Option<Popen>) -> Self {
        Self {
            child,
            service_name,
            test_name: test_name.into(),
        }
    }

    pub fn test_name(&self) -> &str {
        &self.test_name
    }

    pub fn set_child(&mut self, child: Popen) {
        self.child.replace(child);
    }

    pub fn wait_for_finish(&mut self) -> ExitStatus {
        self.child
            .as_mut()
            .expect("Child subprocess does not exist")
            .wait_timeout(Duration::from_secs(10))
            .unwrap_or_else(|err| panic!("Wait finish {} service error: {:?}", self.service_name, err))
            .unwrap_or_else(|| panic!("Wait finish {} service timeout", self.service_name))
    }

    pub fn kill(&mut self) {
        self.child
            .as_mut()
            .expect("Child subprocess does not exist")
            .kill()
            .unwrap_or_else(|err| panic!("Kill {} service error: {:?}", self.service_name, err));

        self.wait_for_finish();

        debug!("{} service process for {:?} killed", self.service_name, self.test_name);
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for TestServiceProcess {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if let Some(pid) = child.pid() {
                child.terminate().ok();
                debug!(
                    "{} service process for test {:?} stopping (PID={})",
                    self.service_name, self.test_name, pid
                );

                let mut attempts = 10;
                // wait till process terminates
                let exit_code = loop {
                    let timeout = Duration::from_secs(2);
                    match child.wait_timeout(timeout) {
                        Err(err) => {
                            error!("Unable to stop process {}: {:?}", pid, err);
                        },
                        Ok(None) => {
                            warn!("Process {} did not stop for {:?}. Repeat SIGTERM.", pid, timeout);
                            child.terminate().ok();
                        },
                        Ok(Some(exit_code)) => {
                            break exit_code;
                        },
                    }

                    attempts -= 1;
                    if attempts == 0 {
                        self.kill();
                        panic!("Wait too long for the child process (PID={}) to terminate", pid);
                    }
                };

                debug!(
                    "{} service process for test {:?} stopped with {:?} (PID={})",
                    self.service_name, self.test_name, exit_code, pid
                );
            } else {
                info!("No need to terminate the process: already stopped");
            }
        }
    }
}
