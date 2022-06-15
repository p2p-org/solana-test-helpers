use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration, thread,
};
use std::io::Read;

use log::{debug, info, warn};
use subprocess::{make_pipe, Exec, Redirection, Result as PopenResult};

use super::service::TestServiceProcess;
use std::net::SocketAddr;

fn clean_test_ledger_dir(test_name: &str) -> io::Result<PathBuf> {
    let dir = super::util::parent_exe_dir().join("ledger").join(test_name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[derive(Debug)]
pub struct TestValidatorService {
    process: TestServiceProcess,
    ledger_path: PathBuf,
    rpc_port: u16,
    faucet_port: u16,
}

#[derive(Clone)]
pub struct TestValidatorServiceBuilder {
    test_name: String,
    rpc_port: u16,
    faucet_port: u16,
    ledger_path: Option<PathBuf>,
}

impl Default for TestValidatorServiceBuilder {
    fn default() -> Self {
        Self::new("test-validator-service")
    }
}

impl TestValidatorServiceBuilder {
    pub fn new(test_name: impl Into<String>) -> Self {
        Self {
            test_name: test_name.into(),
            rpc_port: 8899,
            faucet_port: 9900,
            ledger_path: None,
        }
    }

    pub fn ledger_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.ledger_path = Some(path.into());
        self
    }

    pub fn faucet_port(mut self, port: u16) -> Self {
        self.faucet_port = port;
        self
    }

    pub fn rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = port;
        self
    }

    pub fn build(mut self) -> TestValidatorService {
        let ledger_path = self
            .ledger_path
            .take()
            .unwrap_or_else(|| clean_test_ledger_dir(&self.test_name).expect("Failed to initialize test ledger"));
        TestValidatorService {
            process: TestServiceProcess::new("Test validator", self.test_name, None),
            ledger_path,
            rpc_port: self.rpc_port,
            faucet_port: self.faucet_port,
        }
    }
}

impl TestValidatorService {
    const SERVICE_WAIT_TRIES: u32 = 50;
    const SERVICE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

    pub fn builder() -> TestValidatorServiceBuilder {
        TestValidatorServiceBuilder::default()
    }

    pub fn start(self) -> PopenResult<Self> {
        self.start_with_wait_tries(Self::SERVICE_WAIT_TRIES)
    }

    pub fn start_with_wait_tries(mut self, wait_tries: u32) -> PopenResult<Self> {
        if self.check_availability().is_ok() {
            return Ok(self);
        }

        let bin_path = PathBuf::from("solana-test-validator");

        info!("Starting test validator process for {}", self.process.test_name());

        let run_output = self.run(&bin_path)?;

        self.wait_for_availability(wait_tries, run_output)?;

        debug!("Test validator service started");
        Ok(self)
    }

    pub fn check_availability(&self) -> Result<(), std::io::Error> {
        std::net::TcpStream::connect_timeout(
            &SocketAddr::new([127, 0, 0, 1].into(), self.rpc_port),
            Self::SERVICE_WAIT_TIMEOUT,
        )
        .map(drop)
    }

    pub fn wait_for_availability(&self, wait_tries: u32, mut run_output: fs::File) -> Result<(), std::io::Error> {
        let mut tries = wait_tries;
        loop {
            match self.check_availability() {
                Ok(_) => break Ok(()),
                Err(err) if tries == 0 => {
                    warn!(
                        "failed to wait for test validator after {} retries: {:?}",
                        wait_tries,
                        err
                    );

                    let output_string = {
                        let mut run_output_string = String::new();
                        match run_output.read_to_string(&mut run_output_string) {
                            Ok(_) => run_output_string,
                            Err(err) => err.to_string(),
                        }
                    };
                    warn!("test validator output:\n{}", output_string);

                    break Err(err);
                },
                Err(_) => {
                    thread::sleep(Duration::from_millis(500));
                    tries -= 1;
                },
            }
        }
    }

    pub fn ledger(&self) -> &Path {
        &self.ledger_path
    }

    pub fn run(&mut self, bin_path: &Path) -> PopenResult<fs::File> {
        debug!("Starting process {:?}", bin_path);

        let (read, write) = make_pipe()?;
        let child = Exec::cmd(bin_path)
            .arg("--ledger")
            .arg(&self.ledger_path)
            .arg("--rpc-port")
            .arg(&self.rpc_port.to_string())
            .arg("--faucet-port")
            .arg(&self.faucet_port.to_string())
            .detached()
            .stdout(Redirection::Pipe)
            .stderr(Redirection::File(write))
            .popen()?;

        let pid = child.pid().expect("Failed to start test validator");
        debug!(
            "Started test validator process PID {:?} for test {}",
            pid,
            self.process.test_name()
        );

        self.process.set_child(child);
        Ok(read)
    }

    pub fn rpc_url(&self) -> String {
        format!("http://localhost:{}", self.rpc_port)
    }

    pub fn faucet_addr(&self) -> SocketAddr {
        SocketAddr::new([127, 0, 0, 1].into(), self.faucet_port)
    }
}
