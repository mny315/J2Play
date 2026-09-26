use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct PrivateBus {
    child: Child,
    pub address: String,
}

impl PrivateBus {
    pub fn start(directory: &Path) -> Self {
        let socket = directory.join("bus");
        let address = format!("unix:path={}", socket.display());
        let child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nosyslog", "--address", &address])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon must be available for this opt-in fixture");
        let bus = Self { child, address };
        let deadline = Instant::now() + Duration::from_secs(2);
        while !socket.exists() {
            assert!(Instant::now() < deadline, "private bus startup deadline");
            std::thread::sleep(Duration::from_millis(5));
        }
        bus
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
